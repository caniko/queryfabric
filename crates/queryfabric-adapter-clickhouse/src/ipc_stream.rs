//! Decode a chunked Arrow IPC byte stream into record batches.

use std::pin::Pin;

use arrow::buffer::Buffer;
use arrow::error::ArrowError;
use arrow::record_batch::RecordBatch;
use arrow_ipc::reader::StreamDecoder;
use futures::stream::{self, Stream, StreamExt};

/// Decode an Arrow IPC stream from a byte stream chunked at arbitrary
/// boundaries.
///
/// The decoder buffers partial messages internally, so callers can split the
/// bytes anywhere. End of input calls [`StreamDecoder::finish`], so a truncated
/// stream is an error and an empty input yields no batches and no error.
pub fn decode_arrow_ipc<S, B, E>(bytes: S) -> impl Stream<Item = Result<RecordBatch, E>>
where
    S: Stream<Item = Result<B, E>>,
    B: Into<Buffer>,
    E: From<ArrowError>,
{
    struct State<S> {
        // Pinned so `unfold` (which owns an unpinned state) can poll it without
        // imposing `Unpin` on the caller's stream type.
        bytes: Pin<Box<S>>,
        decoder: StreamDecoder,
        buffer: Buffer,
    }

    // `None` ends the stream after an error or the end of input.
    stream::unfold(
        Some(State {
            bytes: Box::pin(bytes),
            decoder: StreamDecoder::new(),
            buffer: Buffer::default(),
        }),
        |state| async move {
            let mut state = state?;
            loop {
                if !state.buffer.is_empty() {
                    match state.decoder.decode(&mut state.buffer) {
                        Ok(Some(batch)) => return Some((Ok(batch), Some(state))),
                        Ok(None) => {}
                        Err(error) => return Some((Err(E::from(error)), None)),
                    }
                }
                match state.bytes.next().await {
                    Some(Ok(chunk)) => state.buffer = chunk.into(),
                    Some(Err(error)) => return Some((Err(error), None)),
                    None => {
                        return match state.decoder.finish() {
                            Ok(()) => None,
                            Err(error) => Some((Err(E::from(error)), None)),
                        };
                    }
                }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{ArrayRef, Int32Array};
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow_ipc::writer::StreamWriter;
    use futures::TryStreamExt;

    use super::*;

    fn test_batches() -> (Arc<Schema>, Vec<RecordBatch>) {
        let schema = Arc::new(Schema::new(vec![Field::new("x", DataType::Int32, false)]));
        let batch = |values: &[i32]| {
            let column: ArrayRef = Arc::new(Int32Array::from(values.to_vec()));
            RecordBatch::try_new(schema.clone(), vec![column]).unwrap()
        };
        let batches = vec![batch(&[1, 2, 3]), batch(&[4])];
        (schema, batches)
    }

    fn encode(schema: &Schema, batches: &[RecordBatch]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut writer = StreamWriter::try_new(&mut bytes, schema).unwrap();
        for batch in batches {
            writer.write(batch).unwrap();
        }
        writer.finish().unwrap();
        bytes
    }

    #[tokio::test]
    async fn decodes_batches_split_into_seven_byte_chunks() {
        let (schema, batches) = test_batches();
        let chunks: Vec<Result<Buffer, ArrowError>> = encode(&schema, &batches)
            .chunks(7)
            .map(|chunk| Ok(Buffer::from(chunk.to_vec())))
            .collect();

        let decoded: Vec<RecordBatch> = decode_arrow_ipc(stream::iter(chunks))
            .try_collect()
            .await
            .expect("decode");

        assert_eq!(decoded, batches);
    }

    #[tokio::test]
    async fn empty_input_yields_no_batches_and_no_error() {
        let decoded: Vec<RecordBatch> =
            decode_arrow_ipc(stream::empty::<Result<Buffer, ArrowError>>())
                .try_collect()
                .await
                .expect("decode");

        assert!(decoded.is_empty());
    }
}

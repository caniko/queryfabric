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
///
/// A stream that carries only a schema message yields one zero-row batch with
/// that schema, because the schema is the only thing such a stream says about
/// a result and dropping it loses the column types. ClickHouse answers a query
/// that matches no rows this way.
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
        emitted: bool,
    }

    // `None` ends the stream after an error or the end of input.
    stream::unfold(
        Some(State {
            bytes: Box::pin(bytes),
            decoder: StreamDecoder::new(),
            buffer: Buffer::default(),
            emitted: false,
        }),
        |state| async move {
            let mut state = state?;
            loop {
                if !state.buffer.is_empty() {
                    match state.decoder.decode(&mut state.buffer) {
                        Ok(Some(batch)) => {
                            state.emitted = true;
                            return Some((Ok(batch), Some(state)));
                        }
                        Ok(None) => {}
                        Err(error) => return Some((Err(E::from(error)), None)),
                    }
                }
                match state.bytes.next().await {
                    Some(Ok(chunk)) => state.buffer = chunk.into(),
                    Some(Err(error)) => return Some((Err(error), None)),
                    None => {
                        return match state.decoder.finish() {
                            Ok(()) if !state.emitted => match state.decoder.schema() {
                                Some(schema) => {
                                    let batch = RecordBatch::new_empty(schema.clone());
                                    Some((Ok(batch), None))
                                }
                                None => None,
                            },
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

    /// A zero-row `FORMAT ArrowStream` answer from ClickHouse 24.12.6.70,
    /// captured verbatim with `clickhouse local`:
    ///
    /// ```text
    /// SELECT CAST('excitatory' AS Enum8('unknown' = 0, 'excitatory' = 1,
    ///     'inhibitory' = 2, 'modulatory' = 3)) AS polarity,
    ///   CAST('x' AS LowCardinality(String)) AS cell_type
    /// WHERE 0 FORMAT ArrowStream
    /// ```
    ///
    /// It is 200 bytes: the schema message and the end-of-stream marker, no
    /// record batch. Its schema is ClickHouse's own mapping (`polarity` is
    /// `Int8`), which is the point of keeping it.
    const CLICKHOUSE_EMPTY_STREAM: &[u8] =
        include_bytes!("../fixtures/clickhouse-24.12-empty.arrows");

    fn chunked(bytes: &[u8], width: usize) -> Vec<Result<Buffer, ArrowError>> {
        bytes
            .chunks(width)
            .map(|chunk| Ok(Buffer::from(chunk.to_vec())))
            .collect()
    }

    #[tokio::test]
    async fn decodes_batches_split_into_seven_byte_chunks() {
        let (schema, batches) = test_batches();
        let chunks = chunked(&encode(&schema, &batches), 7);

        let decoded: Vec<RecordBatch> = decode_arrow_ipc(stream::iter(chunks))
            .try_collect()
            .await
            .expect("decode");

        assert_eq!(decoded, batches);
        assert_eq!(decoded.len(), 2, "no extra empty batch after real batches");
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

    #[tokio::test]
    async fn a_schema_only_stream_yields_one_empty_batch_with_that_schema() {
        let (schema, _) = test_batches();
        // Split at an arbitrary boundary smaller than any message.
        let chunks = chunked(&encode(&schema, &[]), 5);

        let decoded: Vec<RecordBatch> = decode_arrow_ipc(stream::iter(chunks))
            .try_collect()
            .await
            .expect("decode");

        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].num_rows(), 0);
        assert_eq!(decoded[0].schema().as_ref(), schema.as_ref());
    }

    #[tokio::test]
    async fn a_clickhouse_empty_result_keeps_the_schema_of_its_schema_message() {
        let chunks = chunked(CLICKHOUSE_EMPTY_STREAM, 3);

        let decoded: Vec<RecordBatch> = decode_arrow_ipc(stream::iter(chunks))
            .try_collect()
            .await
            .expect("decode");

        assert_eq!(decoded.len(), 1);
        let batch = &decoded[0];
        assert_eq!(batch.num_rows(), 0);
        assert_eq!(&batch.schema().field(0).name()[..], "polarity");
        assert_eq!(batch.schema().field(0).data_type(), &DataType::Int8);
        assert_eq!(&batch.schema().field(1).name()[..], "cell_type");
        assert_eq!(batch.schema().field(1).data_type(), &DataType::Utf8);
    }

    #[tokio::test]
    async fn a_truncated_stream_is_an_error() {
        let (schema, batches) = test_batches();
        let bytes = encode(&schema, &batches);
        // Cut inside the schema message, before the stream can end cleanly.
        let chunks = chunked(&bytes[..20], 7);

        let decoded: Vec<Result<RecordBatch, ArrowError>> =
            decode_arrow_ipc(stream::iter(chunks)).collect().await;

        assert!(
            decoded.iter().any(std::result::Result::is_err),
            "a stream that ends mid-message must not look like an empty result"
        );
    }

    #[tokio::test]
    async fn a_stream_truncated_after_its_schema_is_an_error_not_an_empty_result() {
        let (schema, batches) = test_batches();
        // The schema-only encoding is the schema message plus an 8-byte
        // end-of-stream marker, so this cut lands inside the first batch.
        let schema_len = encode(&schema, &[]).len() - 8;
        let bytes = encode(&schema, &batches);
        let chunks = chunked(&bytes[..schema_len + 10], 7);

        let decoded: Vec<Result<RecordBatch, ArrowError>> =
            decode_arrow_ipc(stream::iter(chunks)).collect().await;

        assert!(decoded.iter().any(std::result::Result::is_err));
        assert!(
            decoded.iter().all(std::result::Result::is_err),
            "no schema-bearing empty batch may be synthesized before the error"
        );
    }

    #[tokio::test]
    async fn a_real_zero_row_batch_is_not_duplicated() {
        let (schema, _) = test_batches();
        let empty = RecordBatch::new_empty(schema.clone());
        let chunks = chunked(&encode(&schema, std::slice::from_ref(&empty)), 7);

        let decoded: Vec<RecordBatch> = decode_arrow_ipc(stream::iter(chunks))
            .try_collect()
            .await
            .expect("decode");

        assert_eq!(decoded, vec![empty]);
    }
}

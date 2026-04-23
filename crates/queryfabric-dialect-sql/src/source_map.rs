use queryfabric_ir::QuerySourceSpan;
use sqlparser::tokenizer::{Location, Span as SqlSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePiece {
    pub rewritten_start: usize,
    pub rewritten_len: usize,
    pub origin: Option<QuerySourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMap {
    rewritten_sql: String,
    pieces: Vec<SourcePiece>,
    line_starts: Vec<usize>,
}

impl SourceMap {
    pub fn new(rewritten_sql: String, pieces: Vec<SourcePiece>) -> Self {
        let line_starts = compute_line_starts(&rewritten_sql);
        Self {
            rewritten_sql,
            pieces,
            line_starts,
        }
    }

    pub fn identity(_source_sql: &str, body: &str, body_offset: usize) -> Self {
        Self::new(
            body.to_owned(),
            vec![SourcePiece {
                rewritten_start: 0,
                rewritten_len: body.len(),
                origin: Some(QuerySourceSpan {
                    offset: body_offset,
                    len: body.len(),
                }),
            }],
        )
    }

    pub fn rewritten_sql(&self) -> &str {
        &self.rewritten_sql
    }

    fn location_to_offset(&self, location: Location) -> Option<usize> {
        if location.line == 0 || location.column == 0 {
            return None;
        }
        let line_idx = usize::try_from(location.line.saturating_sub(1)).ok()?;
        let line_start = *self.line_starts.get(line_idx)?;
        let col = usize::try_from(location.column.saturating_sub(1)).ok()?;
        Some(line_start + col)
    }

    fn span_to_rewritten(&self, span: SqlSpan) -> Option<QuerySourceSpan> {
        if span == SqlSpan::empty() {
            return None;
        }
        let start = self.location_to_offset(span.start)?;
        let end = self.location_to_offset(span.end)?;
        Some(QuerySourceSpan {
            offset: start,
            len: end.saturating_sub(start).max(1),
        })
    }

    pub fn map_sql_span(&self, span: SqlSpan) -> Option<QuerySourceSpan> {
        let rewritten = self.span_to_rewritten(span)?;
        let rewritten_end = rewritten.offset + rewritten.len;
        self.pieces
            .iter()
            .filter_map(|piece| {
                let piece_start = piece.rewritten_start;
                let piece_end = piece.rewritten_start + piece.rewritten_len;
                (piece_start < rewritten_end && rewritten.offset < piece_end)
                    .then_some(piece.origin)
                    .flatten()
            })
            .reduce(QuerySourceSpan::union)
    }
}

fn compute_line_starts(input: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (idx, ch) in input.char_indices() {
        if ch == '\n' {
            starts.push(idx + 1);
        }
    }
    starts
}

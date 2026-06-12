use queryfabric_dialect_sql::{SourceMap, SourcePiece, parse_sql_with_source_map};
use queryfabric_ir::{Dialect, DialectMetadata, ParsedQuery, QuerySourceSpan, Result};

#[derive(Debug, Default, Clone, Copy)]
pub struct SyqlDialect;

impl Dialect for SyqlDialect {
    fn name(&self) -> &'static str {
        "syql"
    }

    fn parse(&self, input: &str) -> Result<ParsedQuery> {
        parse_syql(input)
    }
}

#[derive(Debug, Clone)]
struct Fragment {
    text: String,
    origin: QuerySourceSpan,
}

pub fn parse_syql(input: &str) -> Result<ParsedQuery> {
    let (explain, rest, rest_offset) = strip_leading_explain(input);
    let mut metadata = DialectMetadata::default();
    let mut fragments = Vec::new();

    let mut cursor = 0usize;
    for line in rest.split('\n') {
        let line_len = line.len();
        let line_offset = rest_offset + cursor;
        cursor += line_len + 1;

        let trimmed_start = line.len() - line.trim_start().len();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let trimmed_offset = line_offset + trimmed_start;
        let uppercase = trimmed.to_ascii_uppercase();
        if let Some(value) = uppercase.strip_prefix("SCOPE ") {
            metadata.insert("syql.scope", value.to_ascii_lowercase());
            continue;
        }
        if let Some(value) = uppercase.strip_prefix("DOWNLOAD ") {
            metadata.insert("syql.download", value.to_ascii_lowercase());
            continue;
        }
        fragments.push(Fragment {
            text: trimmed.to_owned(),
            origin: QuerySourceSpan {
                offset: trimmed_offset,
                len: trimmed.len(),
            },
        });
    }

    while let Some(last) = fragments.last_mut() {
        let Some((prefix, prefix_len, value, key)) = strip_trailing_directive(&last.text) else {
            break;
        };
        metadata.insert(key, value.to_ascii_lowercase());
        last.text = prefix;
        last.origin.len = prefix_len;
        if last.text.is_empty() {
            fragments.pop();
        }
    }

    let from_first = fragments
        .first()
        .is_some_and(|fragment| fragment.text.to_ascii_uppercase().starts_with("FROM "));

    let mut rewritten = String::new();
    let mut pieces = Vec::new();

    if from_first {
        let prefix = "SELECT * ";
        rewritten.push_str(prefix);
        pieces.push(SourcePiece {
            rewritten_start: 0,
            rewritten_len: prefix.len(),
            origin: None,
        });
    }

    for (idx, fragment) in fragments.iter().enumerate() {
        if idx > 0 {
            let start = rewritten.len();
            rewritten.push(' ');
            pieces.push(SourcePiece {
                rewritten_start: start,
                rewritten_len: 1,
                origin: None,
            });
        }
        let start = rewritten.len();
        rewritten.push_str(&fragment.text);
        pieces.push(SourcePiece {
            rewritten_start: start,
            rewritten_len: fragment.text.len(),
            origin: Some(fragment.origin),
        });
    }

    parse_sql_with_source_map(
        "syql",
        input,
        SourceMap::new(rewritten, pieces),
        explain,
        metadata,
    )
}

fn strip_leading_explain(input: &str) -> (bool, &str, usize) {
    let trimmed_start = input.len() - input.trim_start().len();
    let trimmed = &input[trimmed_start..];
    let upper = trimmed.to_ascii_uppercase();
    if upper == "EXPLAIN" {
        (true, "", input.len())
    } else if upper.starts_with("EXPLAIN")
        && trimmed
            .get(7..)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
    {
        let explain_rest = &trimmed[7..];
        let rest_trimmed = explain_rest.trim_start();
        let body_offset = input.len() - rest_trimmed.len();
        (true, rest_trimmed, body_offset)
    } else {
        (false, trimmed, trimmed_start)
    }
}

fn strip_trailing_directive(input: &str) -> Option<(String, usize, String, &'static str)> {
    let upper = input.to_ascii_uppercase();
    for (needle, key) in [(" DOWNLOAD ", "syql.download"), (" SCOPE ", "syql.scope")] {
        if let Some(idx) = upper.rfind(needle) {
            let prefix = input[..idx].trim_end();
            let value = input[idx + needle.len()..].trim();
            if !value.is_empty() && !value.contains(char::is_whitespace) {
                return Some((prefix.to_owned(), prefix.len(), value.to_owned(), key));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse_syql;

    #[test]
    fn parses_from_first_and_directive_metadata() {
        let query = parse_syql(
            "EXPLAIN\nFROM records\nWHERE score > 100\nSCOPE federation\nDOWNLOAD parquet",
        )
        .expect("parse");
        assert!(query.explain());
        assert_eq!(
            query.dialect_metadata().get("syql.scope"),
            Some("federation")
        );
        assert_eq!(
            query.dialect_metadata().get("syql.download"),
            Some("parquet")
        );
        assert!(query.rendered_sql().starts_with("SELECT * FROM records"));
        assert!(query.syntax().node.span.is_some());
    }

    #[test]
    fn parses_inline_trailing_directives() {
        let query =
            parse_syql("FROM records WHERE score > 100 SCOPE remote DOWNLOAD csv").expect("parse");
        assert_eq!(query.dialect_metadata().get("syql.scope"), Some("remote"));
        assert_eq!(query.dialect_metadata().get("syql.download"), Some("csv"));
        assert_eq!(
            query.rendered_sql(),
            "SELECT * FROM records WHERE score > 100"
        );
    }
}

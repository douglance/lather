use crate::hygiene::source::{Facts, comment};
use proc_macro2::{LineColumn, TokenStream, TokenTree};
use std::collections::{BTreeMap, BTreeSet};

pub fn measure(source: &str, facts: &mut Facts) {
    let Ok(tokens) = source.parse::<TokenStream>() else {
        return;
    };
    let mut lines = BTreeSet::new();
    let mut literal = BTreeMap::new();
    let positions: Vec<_> = std::iter::once(0)
        .chain(source.match_indices('\n').map(|(n, _)| n + 1))
        .collect();
    let offset = |position: LineColumn| {
        let start = positions.get(position.line.saturating_sub(1)).copied()?;
        let end = positions
            .get(position.line)
            .copied()
            .unwrap_or(source.len());
        let line = &source[start..end];
        let column = if line.is_ascii() {
            (position.column <= line.len()).then_some(position.column)?
        } else {
            line.char_indices()
                .map(|(n, _)| n)
                .chain(std::iter::once(line.len()))
                .nth(position.column)?
        };
        Some(start + column)
    };
    collect(tokens, &mut lines, &mut literal, &offset, source);
    facts.code_lines = lines.len() as u64;
    let mut pos = 0;
    while pos < source.len() {
        let bytes = source.as_bytes();
        if let Some(end) = literal.get(&pos).copied().filter(|end| *end > pos) {
            pos = end;
        } else if bytes[pos..].starts_with(b"//") {
            let end = bytes[pos..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(source.len(), |n| pos + n);
            comment(
                facts,
                positions.partition_point(|&start| start <= pos),
                &source[pos..end],
            );
            pos = end;
        } else if bytes[pos..].starts_with(b"/*") {
            let end = block_end(bytes, pos);
            comment(
                facts,
                positions.partition_point(|&start| start <= pos),
                &source[pos..end],
            );
            pos = end;
        } else {
            pos += 1;
        }
    }
}

fn collect(
    tokens: TokenStream,
    lines: &mut BTreeSet<usize>,
    literals: &mut BTreeMap<usize, usize>,
    offset: &impl Fn(LineColumn) -> Option<usize>,
    source: &str,
) {
    for token in tokens {
        let span = token.span();
        let starts_in_comment = offset(span.start())
            .and_then(|n| source.get(n..))
            .is_some_and(|tail| {
                tail.starts_with("///")
                    || tail.starts_with("//!")
                    || tail.starts_with("/**")
                    || tail.starts_with("/*!")
            });
        if starts_in_comment {
            continue;
        }
        match token {
            TokenTree::Group(group) => {
                lines.insert(group.span_open().start().line);
                lines.insert(group.span_close().start().line);
                collect(group.stream(), lines, literals, offset, source);
            }
            TokenTree::Literal(_) => {
                lines.extend(span.start().line..=span.end().line);
                if let (Some(start), Some(end)) = (offset(span.start()), offset(span.end())) {
                    literals.insert(start, end);
                }
            }
            _ => {
                lines.insert(span.start().line);
            }
        }
    }
}

fn block_end(bytes: &[u8], start: usize) -> usize {
    let mut pos = start + 2;
    let mut nesting = 1;
    while pos + 1 < bytes.len() {
        if bytes[pos..].starts_with(b"/*") {
            nesting += 1;
            pos += 2;
        } else if bytes[pos..].starts_with(b"*/") {
            pos += 2;
            nesting -= 1;
            if nesting == 0 {
                return pos;
            }
        } else {
            pos += 1;
        }
    }
    bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_lexing_preserves_strings_and_nested_blocks() {
        let mut facts = Facts::default();
        measure(
            r###"//! TODO in a doc
            fn probe() {
                let crab_🦀 = r#"// FIXME /* comment */"#;
                /* outer /* inner */ TODO */
                let text = "http://host/HACK"; // HACK finish
            }"###
                .replace("crab_🦀", "café")
                .as_str(),
            &mut facts,
        );
        assert_eq!(
            facts
                .findings
                .iter()
                .map(|(_, line, _)| *line)
                .collect::<Vec<_>>(),
            [1, 4, 5]
        );
        assert_eq!(facts.comment_lines, 3);
    }
}

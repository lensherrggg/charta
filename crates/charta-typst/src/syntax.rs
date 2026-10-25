//! Small, span-friendly syntax helpers for the read-only Typst subset.

pub(crate) struct ParsedString {
    pub value: String,
    pub content_start: usize,
    pub content_end: usize,
    pub next: usize,
}

pub(crate) fn skip_spaces(text: &str, mut index: usize, limit: usize) -> usize {
    while index < limit {
        match text[index..].chars().next().unwrap() {
            ' ' | '\t' => index += 1,
            _ => break,
        }
    }
    index
}

pub(crate) fn read_identifier(text: &str, start: usize, limit: usize) -> (String, usize) {
    let mut index = start;
    while index < limit {
        let ch = text[index..].chars().next().unwrap();
        if ch.is_alphanumeric() || ch == '_' || ch == '-' {
            index += ch.len_utf8();
        } else {
            break;
        }
    }
    (text[start..index].to_owned(), index)
}

pub(crate) fn find_matching(text: &str, open_index: usize, limit: usize) -> Option<usize> {
    let limit = limit.min(text.len());
    if open_index >= limit {
        return None;
    }
    let open = text.as_bytes()[open_index];
    let close = match open {
        b'(' => b')',
        b'[' => b']',
        b'{' => b'}',
        _ => return None,
    };
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut index = open_index;
    while index < limit {
        let byte = bytes[index];
        match byte {
            b'\\' => {
                index += 1;
                if index < limit {
                    index += char_len(text, index);
                }
                continue;
            }
            b'"' => {
                index = skip_string(text, index, limit);
                continue;
            }
            b'`' => {
                index = skip_raw(text, index, limit);
                continue;
            }
            _ => {}
        }
        if byte == open {
            depth += 1;
        } else if byte == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(index);
            }
        }
        index += char_len(text, index);
    }
    None
}

/// Find the first top-level content block `[...]` after `from`.
pub(crate) fn find_content_block(text: &str, from: usize, limit: usize) -> Option<(usize, usize)> {
    let limit = limit.min(text.len());
    let mut index = from;
    let mut depth = 0usize;
    while index < limit {
        let ch = text[index..].chars().next().unwrap();
        match ch {
            '\\' => {
                index += 1;
                if index < limit {
                    index += char_len(text, index);
                }
            }
            '"' => index = skip_string(text, index, limit),
            '`' => index = skip_raw(text, index, limit),
            '(' | '{' => {
                depth += 1;
                index += 1;
            }
            ')' | '}' => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            '[' => {
                let close = find_matching(text, index, limit)?;
                if depth == 0 {
                    return Some((index, close));
                }
                index = close + 1;
            }
            _ => index += ch.len_utf8(),
        }
    }
    None
}

/// End of a statement: the next newline at bracket depth zero.
pub(crate) fn statement_end(text: &str, start: usize, limit: usize) -> usize {
    let limit = limit.min(text.len());
    let mut index = start;
    let mut depth = 0usize;
    while index < limit {
        let ch = text[index..].chars().next().unwrap();
        match ch {
            '\\' => {
                index += 1;
                if index < limit {
                    index += char_len(text, index);
                }
            }
            '"' => index = skip_string(text, index, limit),
            '`' => index = skip_raw(text, index, limit),
            '(' | '[' | '{' => {
                depth += 1;
                index += 1;
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            '\n' if depth == 0 => return index,
            _ => index += ch.len_utf8(),
        }
    }
    limit
}

pub(crate) fn parse_string_literal(text: &str, start: usize, limit: usize) -> Option<ParsedString> {
    let limit = limit.min(text.len());
    if start >= limit || text.as_bytes()[start] != b'"' {
        return None;
    }
    let content_start = start + 1;
    let mut index = content_start;
    let mut value = String::new();
    while index < limit {
        let ch = text[index..].chars().next().unwrap();
        if ch == '"' {
            return Some(ParsedString {
                value,
                content_start,
                content_end: index,
                next: index + 1,
            });
        }
        if ch == '\\' {
            index += 1;
            if index >= limit {
                return None;
            }
            let escaped = text[index..].chars().next().unwrap();
            value.push(match escaped {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '"' => '"',
                '\\' => '\\',
                other => other,
            });
            index += escaped.len_utf8();
            continue;
        }
        value.push(ch);
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn split_top_level(
    text: &str,
    start: usize,
    end: usize,
    delimiter: char,
) -> Vec<(usize, usize)> {
    let end = end.min(text.len());
    let mut parts = Vec::new();
    let mut part_start = start;
    let mut index = start;
    let mut depth = 0i32;
    let mut in_string: Option<char> = None;
    let mut escaped = false;
    while index < end {
        let ch = text[index..].chars().next().unwrap();
        if escaped {
            escaped = false;
            index += ch.len_utf8();
            continue;
        }
        if let Some(quote) = in_string {
            if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                in_string = None;
            }
            index += ch.len_utf8();
            continue;
        }
        match ch {
            '"' | '\'' => in_string = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ if ch == delimiter && depth == 0 => {
                parts.push((part_start, index));
                part_start = index + ch.len_utf8();
            }
            _ => {}
        }
        index += ch.len_utf8();
    }
    parts.push((part_start, end));
    parts
}

pub(crate) fn find_top_level_colon(text: &str, start: usize, end: usize) -> Option<usize> {
    let mut index = start;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    while index < end {
        let ch = text[index..].chars().next().unwrap();
        if escaped {
            escaped = false;
            index += ch.len_utf8();
            continue;
        }
        if in_string {
            if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            index += ch.len_utf8();
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ':' if depth == 0 => return Some(index),
            _ => {}
        }
        index += ch.len_utf8();
    }
    None
}

pub(crate) fn trim_range(text: &str, start: usize, end: usize) -> (usize, usize) {
    let end = end.min(text.len());
    if start >= end {
        return (start, start);
    }
    let slice = &text[start..end];
    let leading = slice.len() - slice.trim_start().len();
    let trailing = slice.trim_end().len();
    (start + leading, start + trailing)
}

pub(crate) fn unquote(text: &str, start: usize, end: usize) -> String {
    let (start, end) = trim_range(text, start, end);
    if let Some(parsed) = parse_string_literal(text, start, end) {
        if parsed.next >= end {
            return parsed.value;
        }
    }
    text[start..end].to_owned()
}

pub(crate) fn char_len(text: &str, index: usize) -> usize {
    text[index..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or(1)
}

fn skip_string(text: &str, start: usize, limit: usize) -> usize {
    let mut index = start + 1;
    while index < limit {
        match text.as_bytes()[index] {
            b'\\' => {
                index += 1;
                if index < limit {
                    index += char_len(text, index);
                }
            }
            b'"' => return index + 1,
            _ => index += 1,
        }
    }
    limit
}

fn skip_raw(text: &str, start: usize, limit: usize) -> usize {
    let bytes = text.as_bytes();
    let run = bytes[start..]
        .iter()
        .take_while(|byte| **byte == b'`')
        .count();
    let mut index = start + run.max(1);
    while index < limit {
        if bytes[index] == b'`' {
            let closing = bytes[index..]
                .iter()
                .take_while(|byte| **byte == b'`')
                .count();
            if closing >= run {
                return index + closing;
            }
            index += closing;
        } else {
            index += 1;
        }
    }
    limit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_nested_brackets_and_strings() {
        let text = "#text(fill: red)[a [b] \"c]\"]";
        let open = text.find('(').unwrap();
        let close = find_matching(text, open, text.len()).unwrap();
        assert_eq!(&text[close..close + 1], ")");
        let content = find_content_block(text, close, text.len()).unwrap();
        assert_eq!(&text[content.0..=content.1], "[a [b] \"c]\"]");
    }

    #[test]
    fn splits_named_arguments() {
        let text = "font: \"A, B\", size: 12pt, fill: rgb(1, 2, 3)";
        let parts = split_top_level(text, 0, text.len(), ',');
        assert_eq!(parts.len(), 3);
        assert_eq!(text[parts[1].0..parts[1].1].trim(), "size: 12pt");
    }

    #[test]
    fn statement_end_respects_brackets() {
        let text = "#set text(size: 12pt)\nnext";
        let end = statement_end(text, 0, text.len());
        assert_eq!(&text[..end], "#set text(size: 12pt)");
    }
}

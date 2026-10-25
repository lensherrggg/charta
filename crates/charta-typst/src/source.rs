use charta::SourceSpan;

/// Source text with byte-exact line mapping.
///
/// Comments are replaced by spaces before parsing so that byte offsets and
/// line breaks stay identical to the original file.
pub(crate) struct SourceText {
    pub file: String,
    pub text: String,
    line_starts: Vec<usize>,
}

impl SourceText {
    pub fn new(file: impl Into<String>, raw: &str) -> Self {
        let text = strip_comments(raw);
        let mut line_starts = vec![0];
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(index + 1);
            }
        }
        Self {
            file: file.into(),
            text,
            line_starts,
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    pub fn line_start(&self, line: usize) -> usize {
        self.line_starts
            .get(line)
            .copied()
            .unwrap_or(self.text.len())
    }

    pub fn line_end(&self, line: usize) -> usize {
        let next = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.text.len());
        let mut end = next;
        if end > self.line_start(line) && self.text.as_bytes()[end - 1] == b'\n' {
            end -= 1;
        }
        if end > self.line_start(line) && self.text.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }
        end
    }

    pub fn line_text(&self, line: usize) -> &str {
        &self.text[self.line_start(line)..self.line_end(line)]
    }

    pub fn line_index(&self, byte: usize) -> usize {
        let byte = byte.min(self.text.len());
        match self.line_starts.binary_search(&byte) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        }
    }

    pub fn is_blank(&self, line: usize) -> bool {
        self.line_text(line).trim().is_empty()
    }

    /// Byte offset just after leading whitespace.
    pub fn after_indent(&self, line: usize) -> usize {
        let start = self.line_start(line);
        let end = self.line_end(line);
        let mut offset = start;
        for ch in self.text[start..end].chars() {
            if ch == ' ' || ch == '\t' {
                offset += ch.len_utf8();
            } else {
                break;
            }
        }
        offset
    }

    pub fn indent_width(&self, line: usize) -> usize {
        let start = self.line_start(line);
        let end = self.line_end(line);
        self.text[start..end]
            .chars()
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .map(|ch| if ch == '\t' { 4 } else { 1 })
            .sum()
    }

    pub fn span(&self, start: usize, end: usize) -> SourceSpan {
        let start = start.min(self.text.len());
        let end = end.max(start).min(self.text.len());
        let start_line = self.line_index(start);
        let end_line = self.line_index(end);
        let start_line_start = self.line_start(start_line);
        let end_line_start = self.line_start(end_line);
        let column = self.text[start_line_start..start].chars().count() + 1;
        let end_column = self.text[end_line_start..end].chars().count() + 1;
        SourceSpan::new(
            &self.file,
            start_line + 1,
            column,
            end_line + 1,
            end_column,
            start,
            end,
        )
    }

    pub fn line_span(&self, line: usize) -> SourceSpan {
        self.span(self.line_start(line), self.line_end(line))
    }

    pub fn snippet(&self, start: usize, end: usize, max_chars: usize) -> String {
        let start = start.min(self.text.len());
        let end = end.max(start).min(self.text.len());
        let mut out: String = self.text[start..end].chars().take(max_chars).collect();
        if self.text[start..end].chars().count() > max_chars {
            out.push('…');
        }
        out
    }
}

/// Replace `//` and `/* */` comments with spaces while preserving newlines and
/// every byte offset. `//` after a colon (as in URLs) is not treated as a
/// comment; this conservative rule avoids corrupting plain-text URLs.
pub(crate) fn strip_comments(input: &str) -> String {
    let mut bytes = input.as_bytes().to_vec();
    let len = bytes.len();
    let mut index = 0;
    while index < len {
        match bytes[index] {
            b'\\' => {
                index += 1;
                if index < len {
                    index += char_len(input, index);
                }
            }
            b'"' => {
                index += 1;
                while index < len {
                    match bytes[index] {
                        b'\\' => {
                            index += 1;
                            if index < len {
                                index += char_len(input, index);
                            }
                        }
                        b'"' => {
                            index += 1;
                            break;
                        }
                        _ => index += 1,
                    }
                }
            }
            b'`' => {
                let run = count_byte(&bytes, index, b'`');
                index += run;
                while index < len {
                    if bytes[index] == b'`' && count_byte(&bytes, index, b'`') >= run {
                        index += run;
                        break;
                    }
                    index += 1;
                }
            }
            b'/' if index + 1 < len
                && bytes[index + 1] == b'/'
                && !preceded_by_colon(&bytes, index) =>
            {
                while index < len && bytes[index] != b'\n' {
                    bytes[index] = b' ';
                    index += 1;
                }
            }
            b'/' if index + 1 < len && bytes[index + 1] == b'*' => {
                let mut depth = 1;
                bytes[index] = b' ';
                bytes[index + 1] = b' ';
                index += 2;
                while index < len && depth > 0 {
                    if index + 1 < len && bytes[index] == b'/' && bytes[index + 1] == b'*' {
                        depth += 1;
                        bytes[index] = b' ';
                        bytes[index + 1] = b' ';
                        index += 2;
                    } else if index + 1 < len && bytes[index] == b'*' && bytes[index + 1] == b'/' {
                        depth -= 1;
                        bytes[index] = b' ';
                        bytes[index + 1] = b' ';
                        index += 2;
                    } else {
                        if bytes[index] != b'\n' && bytes[index] != b'\r' {
                            bytes[index] = b' ';
                        }
                        index += 1;
                    }
                }
            }
            _ => {
                index += char_len(input, index);
            }
        }
    }
    String::from_utf8(bytes).unwrap_or_default()
}

fn char_len(input: &str, index: usize) -> usize {
    input[index..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or(1)
}

fn count_byte(bytes: &[u8], start: usize, byte: u8) -> usize {
    bytes[start..]
        .iter()
        .take_while(|value| **value == byte)
        .count()
}

fn preceded_by_colon(bytes: &[u8], index: usize) -> bool {
    let mut cursor = index;
    while cursor > 0 {
        cursor -= 1;
        match bytes[cursor] {
            b' ' | b'\t' => continue,
            b':' => return true,
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_stripping_preserves_offsets_and_newlines() {
        let source = "第一行 // 注释\n第二行 /* 块\n注释 */ 正文\nhttps://example.com/a";
        let stripped = strip_comments(source);
        assert_eq!(stripped.len(), source.len());
        assert_eq!(stripped.lines().count(), source.lines().count());
        assert!(!stripped.contains("注释"));
        assert!(stripped.contains("https://example.com/a"));
        assert!(stripped.contains("正文"));
    }

    #[test]
    fn spans_map_bytes_to_lines_and_columns() {
        let source = SourceText::new("a.typ", "第一行\n第二行\n");
        let start = source.text.find("第二行").unwrap();
        let end = start + "第二行".len();
        let span = source.span(start, end);
        assert_eq!(span.file, "a.typ");
        assert_eq!(span.line, 2);
        assert_eq!(span.column, 1);
        assert_eq!(span.end_column, 4);
    }
}

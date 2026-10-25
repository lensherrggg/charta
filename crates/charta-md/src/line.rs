use charta::SourceSpan;

#[derive(Debug, Clone)]
pub(crate) struct Line<'a> {
    pub number: usize,
    pub start: usize,
    pub text: &'a str,
}

impl Line<'_> {
    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub fn trimmed_end(&self) -> usize {
        self.text.trim_end().len()
    }
}

pub(crate) fn split_lines(source: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for raw in source.split_inclusive('\n') {
        let mut text = raw;
        if let Some(stripped) = text.strip_suffix('\n') {
            text = stripped;
        }
        if let Some(stripped) = text.strip_suffix('\r') {
            text = stripped;
        }
        lines.push(Line {
            number: lines.len() + 1,
            start,
            text,
        });
        start += raw.len();
    }
    lines
}

pub(crate) fn span(file: &str, line: &Line<'_>, start: usize, end: usize) -> SourceSpan {
    let column = line.text[..start].chars().count() + 1;
    let end_column = line.text[..end].chars().count() + 1;
    SourceSpan::new(
        file,
        line.number,
        column,
        line.number,
        end_column,
        line.start + start,
        line.start + end,
    )
}

pub(crate) fn full_span(file: &str, line: &Line<'_>) -> SourceSpan {
    span(file, line, 0, line.text.len())
}

pub(crate) fn leading_width(text: &str) -> usize {
    let mut width = 0;
    for ch in text.chars() {
        match ch {
            ' ' => width += 1,
            '\t' => width += 4,
            _ => break,
        }
    }
    width
}

/// Byte offset just after the leading whitespace.
pub(crate) fn after_indent(text: &str) -> usize {
    let mut offset = 0;
    for ch in text.chars() {
        if ch == ' ' || ch == '\t' {
            offset += ch.len_utf8();
        } else {
            break;
        }
    }
    offset
}

pub(crate) fn snippet(text: &str, max_chars: usize) -> String {
    let mut out: String = text.chars().take(max_chars).collect();
    if text.chars().count() > max_chars {
        out.push('…');
    }
    out
}

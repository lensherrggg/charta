//! Block-level Markdown parsing for the controlled subset.

use crate::{inline, line};
use charta::frontmatter::{self, YamlLine};
use charta::{
    Alignment, Block, ImageBlock, Inline, Issue, ListBlock, ListItem, Metadata, SourceSpan,
    TableBlock, TableCell, TableRow,
};
use line::{after_indent, full_span, leading_width, snippet, span, split_lines, Line};

pub(crate) struct Parsed {
    pub metadata: Metadata,
    pub blocks: Vec<Block>,
    pub issues: Vec<Issue>,
}

pub(crate) fn parse(source: &str, file: &str) -> Parsed {
    let lines = split_lines(source);
    let mut parser = Parser {
        file: file.to_owned(),
        lines,
        index: 0,
        metadata: Metadata::new(),
        issues: Vec::new(),
        blocks: Vec::new(),
    };
    parser.parse_document();
    Parsed {
        metadata: parser.metadata,
        blocks: parser.blocks,
        issues: parser.issues,
    }
}

struct Parser<'a> {
    file: String,
    lines: Vec<Line<'a>>,
    index: usize,
    metadata: Metadata,
    issues: Vec<Issue>,
    blocks: Vec<Block>,
}

impl Parser<'_> {
    fn parse_document(&mut self) {
        self.parse_front_matter();
        while self.index < self.lines.len() {
            if self.lines[self.index].is_blank() {
                self.index += 1;
                continue;
            }
            self.parse_one();
        }
    }

    fn parse_front_matter(&mut self) {
        if self.lines.is_empty() {
            return;
        }
        if self.lines[0].text.trim_end() != "---" {
            return;
        }
        let close = self
            .lines
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, line)| {
                let text = line.text.trim_end();
                text == "---" || text == "..."
            })
            .map(|(index, _)| index);
        let Some(close) = close else {
            let line = self.lines[0].clone();
            self.issues.push(Issue::schema(
                "front matter 未闭合",
                "front-matter",
                "以 `---` 或 `...` 结束",
                "缺失结束行",
                Some(full_span(&self.file, &line)),
            ));
            self.index = 1;
            return;
        };

        let yaml_lines: Vec<YamlLine<'_>> = self.lines[1..close]
            .iter()
            .map(|line| frontmatter::YamlLine {
                number: line.number,
                start: line.start,
                text: line.text,
            })
            .collect();
        match frontmatter::parse(&yaml_lines) {
            Ok(metadata) => self.metadata = metadata,
            Err(error) => {
                let location = SourceSpan::new(
                    &self.file,
                    error.line,
                    error.column,
                    error.line,
                    error.column,
                    error.byte_start,
                    error.byte_end,
                );
                self.issues.push(Issue::schema(
                    format!("front matter 解析失败：{}", error.message),
                    "front-matter",
                    "受支持的 YAML 子集",
                    error.message,
                    Some(location),
                ));
            }
        }
        self.index = close + 1;
    }

    fn parse_one(&mut self) {
        let text = self.lines[self.index].text.to_owned();
        if let Some(marker) = heading_marker(&text) {
            if marker.level > 6 {
                let line = self.lines[self.index].clone();
                self.issues.push(Issue::unsupported(
                    "标题层级超过 6 级",
                    snippet(&text, 40),
                    "作为普通段落保留",
                    Some(full_span(&self.file, &line)),
                ));
                self.parse_paragraph();
            } else {
                self.parse_heading(marker);
            }
            return;
        }
        if fence_marker(&text).is_some() {
            self.parse_code_block();
            return;
        }
        if is_thematic_break(&text) {
            let line = self.lines[self.index].clone();
            self.issues.push(Issue::unsupported(
                "不支持分隔线（thematic break）",
                snippet(&text, 40),
                "已忽略（IR 无对应结构角色）",
                Some(full_span(&self.file, &line)),
            ));
            self.index += 1;
            return;
        }
        if quote_content_start(&text).is_some() {
            self.parse_blockquote();
            return;
        }
        if let Some(image) = inline::parse_standalone_image(&text) {
            let line = self.lines[self.index].clone();
            let source = full_span(&self.file, &line);
            let caption = image
                .title
                .as_deref()
                .filter(|title| !title.trim().is_empty())
                .map(|title| vec![Inline::text(title, source.clone())]);
            self.blocks.push(Block::image(
                ImageBlock {
                    path: image.path,
                    alt: Some(image.alt),
                    title: image.title,
                    caption,
                },
                source,
            ));
            self.index += 1;
            return;
        }
        if self.is_table_start(self.index) {
            self.parse_table();
            return;
        }
        if is_link_definition(&text) {
            let line = self.lines[self.index].clone();
            self.issues.push(Issue::unsupported(
                "不支持链接引用定义",
                snippet(&text, 40),
                "已忽略该定义",
                Some(full_span(&self.file, &line)),
            ));
            self.index += 1;
            return;
        }
        if is_list_item_line(&text) {
            self.parse_list();
            return;
        }
        if is_html_block(&text) {
            self.parse_html_block();
            return;
        }
        self.parse_paragraph();
    }

    fn parse_heading(&mut self, marker: HeadingMarker) {
        let line = self.lines[self.index].clone();
        let text = line.text;
        let mut start = marker.after_hashes;
        while start < text.len() {
            let ch = text[start..].chars().next().unwrap();
            if ch.is_whitespace() {
                start += ch.len_utf8();
            } else {
                break;
            }
        }
        let mut end = line.trimmed_end();
        if end > start && text[start..end].ends_with('#') {
            let content = &text[start..end];
            let hashes = content.chars().rev().take_while(|&ch| ch == '#').count();
            let before_end = end - hashes;
            let before = &text[start..before_end];
            if before.is_empty() || before.ends_with(char::is_whitespace) {
                end = start + before.trim_end().len();
            }
        }
        let content = if end > start {
            inline::parse(&self.file, &line, start, end, &mut self.issues)
        } else {
            Vec::new()
        };
        self.blocks.push(Block::heading(
            marker.level as u8,
            content,
            full_span(&self.file, &line),
        ));
        self.index += 1;
    }

    fn parse_code_block(&mut self) {
        let opening = self.lines[self.index].clone();
        let Some(fence) = fence_marker(opening.text) else {
            self.index += 1;
            return;
        };
        self.issues.push(Issue::unsupported(
            "不支持围栏代码块",
            snippet(opening.text, 40),
            "代码内容按纯文本段落保留（不做语法高亮）",
            Some(full_span(&self.file, &opening)),
        ));
        self.index += 1;
        while self.index < self.lines.len() {
            let line = self.lines[self.index].clone();
            if is_closing_fence(line.text, fence.ch, fence.len) {
                self.index += 1;
                break;
            }
            let end = line.trimmed_end();
            if end > 0 {
                let content = vec![Inline::text(
                    &line.text[..end],
                    span(&self.file, &line, 0, end),
                )];
                self.blocks
                    .push(Block::paragraph(content, full_span(&self.file, &line)));
            }
            self.index += 1;
        }
    }

    fn parse_blockquote(&mut self) {
        let first = self.lines[self.index].clone();
        let mut paragraphs = Vec::new();
        let mut current: Vec<(Line<'_>, usize, usize)> = Vec::new();
        let mut last = first.clone();

        while self.index < self.lines.len() {
            let line = self.lines[self.index].clone();
            let Some(start) = quote_content_start(line.text) else {
                break;
            };
            let end = line.trimmed_end();
            if end <= start {
                flush_quote_paragraph(&self.file, &mut current, &mut paragraphs, &mut self.issues);
            } else {
                current.push((line.clone(), start, end));
            }
            last = line;
            self.index += 1;
        }
        flush_quote_paragraph(&self.file, &mut current, &mut paragraphs, &mut self.issues);

        if !paragraphs.is_empty() {
            let start_span = full_span(&self.file, &first);
            let end_span = full_span(&self.file, &last);
            let source = SourceSpan::new(
                &self.file,
                start_span.line,
                start_span.column,
                end_span.end_line,
                end_span.end_column,
                start_span.byte_start,
                end_span.byte_end,
            );
            self.blocks.push(Block::quote(paragraphs, source));
        }
    }

    fn parse_html_block(&mut self) {
        let first = self.lines[self.index].clone();
        self.issues.push(Issue::unsupported(
            "不支持 HTML 块",
            snippet(first.text, 40),
            "按纯文本段落保留",
            Some(full_span(&self.file, &first)),
        ));
        let mut inlines = Vec::new();
        let mut last = first.clone();
        while self.index < self.lines.len() {
            let line = self.lines[self.index].clone();
            if line.is_blank() {
                break;
            }
            let end = line.trimmed_end();
            if end > 0 {
                if !inlines.is_empty() {
                    inlines.push(Inline::text(" ", span(&self.file, &line, 0, 0)));
                }
                inlines.push(Inline::text(
                    &line.text[..end],
                    span(&self.file, &line, 0, end),
                ));
            }
            last = line;
            self.index += 1;
        }
        if !inlines.is_empty() {
            let start = full_span(&self.file, &first);
            let end = full_span(&self.file, &last);
            let source = SourceSpan::new(
                &self.file,
                start.line,
                start.column,
                end.end_line,
                end.end_column,
                start.byte_start,
                end.byte_end,
            );
            self.blocks.push(Block::paragraph(inlines, source));
        }
    }

    fn parse_table(&mut self) {
        let header_line = self.lines[self.index].clone();
        let delimiter_line = self.lines[self.index + 1].clone();
        let alignments = delimiter_alignments(delimiter_line.text).unwrap_or_default();
        let columns = alignments.len();
        let header_cells = self.table_row(&header_line, columns, &alignments, true);
        let header = TableRow::new(header_cells);
        self.index += 2;

        let mut rows = Vec::new();
        let mut last = delimiter_line.clone();
        while self.index < self.lines.len() {
            let line = self.lines[self.index].clone();
            if line.is_blank() || !has_unescaped_pipe(line.text) {
                break;
            }
            rows.push(TableRow::new(self.table_row(
                &line,
                columns,
                &alignments,
                false,
            )));
            last = line;
            self.index += 1;
        }

        let start = full_span(&self.file, &header_line);
        let end = full_span(&self.file, &last);
        let source = SourceSpan::new(
            &self.file,
            start.line,
            start.column,
            end.end_line,
            end.end_column,
            start.byte_start,
            end.byte_end,
        );
        self.blocks.push(Block::table(
            TableBlock {
                header: vec![header],
                rows,
                column_alignments: alignments,
                caption: None,
            },
            source,
        ));
    }

    fn table_row(
        &mut self,
        line: &Line<'_>,
        columns: usize,
        alignments: &[Alignment],
        header: bool,
    ) -> Vec<TableCell> {
        let ranges = trim_cell_ranges(line.text, split_table_cells(line.text));
        if ranges.len() != columns {
            self.issues.push(Issue::unsupported(
                format!(
                    "表格{}列数与分隔行不一致（{} vs {}）",
                    if header { "表头" } else { "行" },
                    ranges.len(),
                    columns
                ),
                snippet(line.text, 60),
                "已按分隔行列数补齐或截断",
                Some(full_span(&self.file, line)),
            ));
        }
        let mut cells = Vec::with_capacity(columns);
        for index in 0..columns {
            let content = match ranges.get(index) {
                Some(&(start, end)) if end > start => {
                    inline::parse(&self.file, line, start, end, &mut self.issues)
                }
                _ => Vec::new(),
            };
            cells.push(TableCell {
                content,
                colspan: 1,
                rowspan: 1,
                alignment: alignments.get(index).copied(),
            });
        }
        cells
    }

    fn parse_list(&mut self) {
        let first = self.lines[self.index].clone();
        let base_width = leading_width(first.text);
        let Some(marker) = marker_at(first.text) else {
            self.index += 1;
            return;
        };
        let list = self.parse_list_at(base_width, marker.ordered, marker.start);
        let last = self.lines[self.index.saturating_sub(1)].clone();
        let start = full_span(&self.file, &first);
        let end = full_span(&self.file, &last);
        let source = SourceSpan::new(
            &self.file,
            start.line,
            start.column,
            end.end_line,
            end.end_column,
            start.byte_start,
            end.byte_end,
        );
        self.blocks.push(Block::list(list, source));
    }

    fn parse_list_at(&mut self, base_width: usize, ordered: bool, start: u32) -> ListBlock {
        let mut items = Vec::new();
        while self.index < self.lines.len() {
            let line = self.lines[self.index].clone();
            if line.is_blank() || leading_width(line.text) != base_width {
                break;
            }
            let Some(marker) = marker_at(line.text) else {
                break;
            };
            if marker.ordered != ordered {
                break;
            }
            self.index += 1;

            let mut content_start = marker.content_start;
            let content_end = line.trimmed_end();
            if content_start > content_end {
                content_start = content_end;
            }
            if is_task_marker(&line.text[content_start..content_end]) {
                self.issues.push(Issue::unsupported(
                    "不支持任务列表复选框",
                    snippet(&line.text[content_start..content_end], 40),
                    "移除复选框标记，保留文字",
                    Some(span(&self.file, &line, content_start, content_end)),
                ));
                content_start += 4;
            }
            let mut content = if content_end > content_start {
                inline::parse(
                    &self.file,
                    &line,
                    content_start,
                    content_end,
                    &mut self.issues,
                )
            } else {
                Vec::new()
            };

            let mut children = Vec::new();
            loop {
                if self.index >= self.lines.len() {
                    break;
                }
                let next = self.lines[self.index].clone();
                if next.is_blank() {
                    break;
                }
                let next_width = leading_width(next.text);
                if next_width <= base_width {
                    break;
                }
                if let Some(child_marker) = marker_at(next.text) {
                    let child =
                        self.parse_list_at(next_width, child_marker.ordered, child_marker.start);
                    children.push(child);
                    continue;
                }
                self.index += 1;
                let end = next.trimmed_end();
                let indent = after_indent(next.text);
                if end > indent {
                    if !content.is_empty() {
                        content.push(Inline::text(" ", span(&self.file, &next, indent, indent)));
                    }
                    content.extend(inline::parse(
                        &self.file,
                        &next,
                        indent,
                        end,
                        &mut self.issues,
                    ));
                }
            }
            items.push(ListItem { content, children });
        }
        if ordered {
            ListBlock::ordered(start, items)
        } else {
            ListBlock::unordered(items)
        }
    }

    fn parse_paragraph(&mut self) {
        let first = self.lines[self.index].clone();
        let mut inlines = Vec::new();
        let mut last = first.clone();
        let mut consumed = false;

        while self.index < self.lines.len() {
            let line = self.lines[self.index].clone();
            if line.is_blank() {
                break;
            }
            if consumed && self.is_block_start(self.index) {
                break;
            }
            if consumed && is_setext_underline(line.text) {
                self.issues.push(Issue::unsupported(
                    "不支持 Setext 标题语法",
                    snippet(line.text, 40),
                    "已忽略该下划线，前一段保留为正文",
                    Some(full_span(&self.file, &line)),
                ));
                self.index += 1;
                break;
            }

            let original_len = line.text.len();
            let mut end = line.trimmed_end();
            if line.text.ends_with("  ") && original_len > 0 {
                self.issues.push(Issue::unsupported(
                    "不支持行尾双空格硬换行",
                    snippet(line.text, 40),
                    "按空格合并到同一段落",
                    Some(span(&self.file, &line, end, original_len)),
                ));
            }
            if end > 0 && line.text[..end].ends_with('\\') {
                self.issues.push(Issue::unsupported(
                    "不支持行尾反斜杠硬换行",
                    snippet(line.text, 40),
                    "按空格合并到同一段落",
                    Some(span(&self.file, &line, end - 1, end)),
                ));
                end -= 1;
                end = line.text[..end].trim_end().len();
            }

            if end > 0 {
                if consumed && !inlines.is_empty() {
                    inlines.push(Inline::text(" ", span(&self.file, &line, 0, 0)));
                }
                inlines.extend(inline::parse(&self.file, &line, 0, end, &mut self.issues));
            }
            consumed = true;
            last = line;
            self.index += 1;
        }

        if !inlines.is_empty() {
            let start = full_span(&self.file, &first);
            let end = full_span(&self.file, &last);
            let source = SourceSpan::new(
                &self.file,
                start.line,
                start.column,
                end.end_line,
                end.end_column,
                start.byte_start,
                end.byte_end,
            );
            self.blocks.push(Block::paragraph(inlines, source));
        }
    }

    fn is_block_start(&self, index: usize) -> bool {
        let text = &self.lines[index].text;
        if heading_marker(text).is_some()
            || fence_marker(text).is_some()
            || is_thematic_break(text)
            || quote_content_start(text).is_some()
            || inline::parse_standalone_image(text).is_some()
            || is_link_definition(text)
            || is_list_item_line(text)
            || is_html_block(text)
        {
            return true;
        }
        self.is_table_start(index)
    }

    fn is_table_start(&self, index: usize) -> bool {
        if index + 1 >= self.lines.len() {
            return false;
        }
        if !has_unescaped_pipe(self.lines[index].text) {
            return false;
        }
        let delimiter = &self.lines[index + 1].text;
        has_unescaped_pipe(delimiter) && delimiter_alignments(delimiter).is_some()
    }
}

fn flush_quote_paragraph(
    file: &str,
    current: &mut Vec<(Line<'_>, usize, usize)>,
    paragraphs: &mut Vec<Block>,
    issues: &mut Vec<Issue>,
) {
    if current.is_empty() {
        return;
    }
    let first = current[0].0.clone();
    let last = current[current.len() - 1].0.clone();
    let mut inlines = Vec::new();
    for (line, start, end) in current.drain(..) {
        if !inlines.is_empty() {
            inlines.push(Inline::text(" ", span(file, &line, start, start)));
        }
        inlines.extend(inline::parse(file, &line, start, end, issues));
    }
    let start_span = full_span(file, &first);
    let end_span = full_span(file, &last);
    let source = SourceSpan::new(
        file,
        start_span.line,
        start_span.column,
        end_span.end_line,
        end_span.end_column,
        start_span.byte_start,
        end_span.byte_end,
    );
    paragraphs.push(Block::paragraph(inlines, source));
}

#[derive(Debug, Clone, Copy)]
struct HeadingMarker {
    level: usize,
    after_hashes: usize,
}

fn heading_marker(text: &str) -> Option<HeadingMarker> {
    let indent = after_indent(text);
    if indent > 3 {
        return None;
    }
    let rest = &text[indent..];
    let hashes = rest.chars().take_while(|&ch| ch == '#').count();
    if hashes == 0 {
        return None;
    }
    let after_hashes = indent + hashes;
    if let Some(next) = text[after_hashes..].chars().next() {
        if !next.is_whitespace() {
            return None;
        }
    }
    Some(HeadingMarker {
        level: hashes,
        after_hashes,
    })
}

#[derive(Debug, Clone, Copy)]
struct Fence {
    ch: char,
    len: usize,
}

fn fence_marker(text: &str) -> Option<Fence> {
    let indent = after_indent(text);
    if indent > 3 {
        return None;
    }
    let rest = &text[indent..];
    let ch = rest.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let len = rest
        .chars()
        .take_while(|&candidate| candidate == ch)
        .count();
    (len >= 3).then_some(Fence { ch, len })
}

fn is_closing_fence(text: &str, ch: char, min_len: usize) -> bool {
    let indent = after_indent(text);
    if indent > 3 {
        return false;
    }
    let rest = &text[indent..];
    let len = rest
        .chars()
        .take_while(|&candidate| candidate == ch)
        .count();
    len >= min_len && rest[len..].trim().is_empty()
}

fn is_thematic_break(text: &str) -> bool {
    let indent = after_indent(text);
    if indent > 3 {
        return false;
    }
    let trimmed = text[indent..].trim_end();
    let chars: Vec<char> = trimmed.chars().filter(|ch| !ch.is_whitespace()).collect();
    if chars.len() < 3 {
        return false;
    }
    let first = chars[0];
    matches!(first, '-' | '*' | '_') && chars.iter().all(|ch| *ch == first)
}

fn is_setext_underline(text: &str) -> bool {
    let indent = after_indent(text);
    if indent > 3 {
        return false;
    }
    let trimmed = text[indent..].trim();
    if trimmed.is_empty() {
        return false;
    }
    trimmed.chars().all(|ch| ch == '=') || trimmed.chars().all(|ch| ch == '-')
}

fn quote_content_start(text: &str) -> Option<usize> {
    let indent = after_indent(text);
    if indent > 3 {
        return None;
    }
    if !text[indent..].starts_with('>') {
        return None;
    }
    let mut start = indent + 1;
    if let Some(ch) = text[start..].chars().next() {
        if ch == ' ' || ch == '\t' {
            start += ch.len_utf8();
        }
    }
    Some(start)
}

fn is_html_block(text: &str) -> bool {
    let indent = after_indent(text);
    let rest = &text[indent..];
    let mut chars = rest.chars();
    if chars.next() != Some('<') {
        return false;
    }
    let Some(next) = chars.next() else {
        return false;
    };
    (next.is_ascii_alphabetic() || next == '/' || next == '!' || next == '?') && rest.contains('>')
}

fn is_link_definition(text: &str) -> bool {
    let indent = after_indent(text);
    if indent > 3 {
        return false;
    }
    let rest = &text[indent..];
    if !rest.starts_with('[') {
        return false;
    }
    let Some(close) = rest.find(']') else {
        return false;
    };
    close > 1 && rest[close + 1..].starts_with(':')
}

#[derive(Debug, Clone, Copy)]
struct ListMarker {
    ordered: bool,
    start: u32,
    content_start: usize,
}

fn marker_at(text: &str) -> Option<ListMarker> {
    let indent = after_indent(text);
    let marker = parse_list_marker(&text[indent..])?;
    Some(ListMarker {
        content_start: indent + marker.content_start,
        ..marker
    })
}

fn parse_list_marker(text: &str) -> Option<ListMarker> {
    if text.is_empty() {
        return None;
    }
    let first = text.chars().next().unwrap();
    if matches!(first, '-' | '+' | '*') {
        let after = first.len_utf8();
        if after == text.len() {
            return Some(ListMarker {
                ordered: false,
                start: 1,
                content_start: after,
            });
        }
        let next = text[after..].chars().next().unwrap();
        if next == ' ' || next == '\t' {
            return Some(ListMarker {
                ordered: false,
                start: 1,
                content_start: after + next.len_utf8(),
            });
        }
        return None;
    }
    let digits = text.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits == 0 || digits > 9 {
        return None;
    }
    let after_digits = text[..digits].len();
    let delimiter = text[after_digits..].chars().next()?;
    if delimiter != '.' && delimiter != ')' {
        return None;
    }
    let after_delimiter = after_digits + delimiter.len_utf8();
    let content_start = if after_delimiter == text.len() {
        after_delimiter
    } else {
        let next = text[after_delimiter..].chars().next().unwrap();
        if next == ' ' || next == '\t' {
            after_delimiter + next.len_utf8()
        } else {
            return None;
        }
    };
    let start = text[..digits].parse().ok()?;
    Some(ListMarker {
        ordered: true,
        start,
        content_start,
    })
}

fn is_list_item_line(text: &str) -> bool {
    marker_at(text).is_some() && !is_thematic_break(text)
}

fn is_task_marker(text: &str) -> bool {
    text.starts_with("[ ] ") || text.starts_with("[x] ") || text.starts_with("[X] ")
}

fn has_unescaped_pipe(text: &str) -> bool {
    let mut escaped = false;
    for ch in text.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '|' => return true,
            _ => {}
        }
    }
    false
}

fn split_table_cells(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '|' => {
                ranges.push((start, index));
                start = index + 1;
            }
            _ => {}
        }
    }
    ranges.push((start, text.len()));
    ranges
}

fn trim_cell_ranges(text: &str, ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    let mut ranges: Vec<(usize, usize)> = ranges
        .into_iter()
        .map(|(start, end)| {
            let slice = &text[start..end];
            let leading = slice.len() - slice.trim_start().len();
            let trailing = slice.trim_end().len();
            (start + leading, start + trailing)
        })
        .collect();
    if ranges.first().is_some_and(|(start, end)| start == end) {
        ranges.remove(0);
    }
    if ranges.last().is_some_and(|(start, end)| start == end) {
        ranges.pop();
    }
    ranges
}

fn delimiter_alignments(text: &str) -> Option<Vec<Alignment>> {
    if !has_unescaped_pipe(text) {
        return None;
    }
    let ranges = trim_cell_ranges(text, split_table_cells(text));
    if ranges.is_empty() {
        return None;
    }
    let mut alignments = Vec::new();
    for (start, end) in ranges {
        let cell = &text[start..end];
        let left = cell.starts_with(':');
        let right = cell.ends_with(':');
        let core = cell.trim_matches(':').trim();
        if core.len() < 3 || !core.chars().all(|ch| ch == '-') {
            return None;
        }
        alignments.push(match (left, right) {
            (true, true) => Alignment::Center,
            (false, true) => Alignment::Right,
            _ => Alignment::Left,
        });
    }
    Some(alignments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use charta::BlockKind;

    fn parse(source: &str) -> Parsed {
        super::parse(source, "test.md")
    }

    #[test]
    fn parses_front_matter_headings_and_paragraphs() {
        let parsed = parse(
            "---\ntitle: 通知\ndate: 2026-10-25\n---\n\n# 一级标题\n\n第一行\n第二行\n\n## 二级 ##\n",
        );
        assert!(parsed.issues.is_empty());
        assert_eq!(parsed.metadata.get("title").unwrap().as_str(), Some("通知"));
        assert_eq!(parsed.blocks.len(), 3);
        assert!(matches!(
            parsed.blocks[0].kind,
            BlockKind::Heading { level: 1, .. }
        ));
        assert!(matches!(parsed.blocks[1].kind, BlockKind::Paragraph { .. }));
        if let BlockKind::Paragraph { content } = &parsed.blocks[1].kind {
            assert_eq!(
                content
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>(),
                "第一行 第二行"
            );
        }
        assert!(matches!(
            parsed.blocks[2].kind,
            BlockKind::Heading { level: 2, .. }
        ));
    }

    #[test]
    fn parses_nested_lists_tables_quotes_images() {
        let parsed = parse(
            "- 第一项\n  - 子项\n- 第二项\n\n| 名称 | 数量 |\n|:---|---:|\n| 甲 | 1 |\n\n> 引用\n> 第二行\n\n![图](a.png)\n",
        );
        assert!(parsed.issues.is_empty(), "{:?}", parsed.issues);
        assert_eq!(parsed.blocks.len(), 4);
        let BlockKind::List(list) = &parsed.blocks[0].kind else {
            panic!("expected list")
        };
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].children.len(), 1);
        let BlockKind::Table(table) = &parsed.blocks[1].kind else {
            panic!("expected table")
        };
        assert_eq!(table.header.len(), 1);
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.column_alignments[0], Alignment::Left);
        assert_eq!(table.column_alignments[1], Alignment::Right);
        let BlockKind::Quote { blocks } = &parsed.blocks[2].kind else {
            panic!("expected quote")
        };
        assert_eq!(blocks.len(), 1);
        let BlockKind::Image(image) = &parsed.blocks[3].kind else {
            panic!("expected image")
        };
        assert_eq!(image.path, "a.png");
    }

    #[test]
    fn image_title_becomes_a_caption() {
        let parsed = parse("![示意图](a.png \"各环节合规率对比\")\n");
        assert!(parsed.issues.is_empty());
        let BlockKind::Image(image) = &parsed.blocks[0].kind else {
            panic!("expected image")
        };
        let caption = image.caption.as_ref().expect("caption");
        assert_eq!(caption.len(), 1);
        assert_eq!(caption[0].text, "各环节合规率对比");

        let parsed = parse("![示意图](a.png)\n");
        let BlockKind::Image(image) = &parsed.blocks[0].kind else {
            panic!("expected image")
        };
        assert!(image.caption.is_none());
    }

    #[test]
    fn reports_unsupported_syntax_with_locations() {
        let parsed = parse(
            "# 标题\n\n[链接](https://example.com)\n\n```\ncode\n```\n\n---\n\n####### 七级\n",
        );
        assert!(parsed
            .issues
            .iter()
            .any(|issue| issue.message.contains("链接")));
        assert!(parsed
            .issues
            .iter()
            .any(|issue| issue.message.contains("代码块")));
        assert!(parsed
            .issues
            .iter()
            .any(|issue| issue.message.contains("分隔线")));
        assert!(parsed
            .issues
            .iter()
            .any(|issue| issue.message.contains("超过 6 级")));
        assert!(parsed
            .issues
            .iter()
            .all(|issue| issue.location.as_ref().unwrap().file == "test.md"));
    }

    #[test]
    fn front_matter_errors_are_blocking() {
        let parsed = parse("---\ntitle: [\n---\n\n正文\n");
        assert_eq!(parsed.issues.len(), 1);
        assert!(parsed.issues[0].is_blocking());
        assert!(parsed.issues[0].message.contains("front matter"));
        assert_eq!(parsed.blocks.len(), 1);
    }
}

//! Block-level Typst parsing for the read-only subset.

use crate::inline;
use crate::metadata;
use crate::source::SourceText;
use crate::syntax::{
    find_content_block, find_matching, find_top_level_colon, parse_string_literal, read_identifier,
    skip_spaces, split_top_level, statement_end, trim_range,
};
use charta::{
    Alignment, Block, Determinism, ImageBlock, Inline, Issue, ListBlock, ListItem, Metadata,
    Severity, SourceSpan, TableBlock, TableCell, TableRow,
};
use std::path::{Path, PathBuf};

pub(crate) struct ParseSink {
    pub issues: Vec<Issue>,
    pub include_stack: Vec<PathBuf>,
    pub max_depth: usize,
}

pub(crate) struct Parsed {
    pub metadata: Metadata,
    pub blocks: Vec<Block>,
    pub issues: Vec<Issue>,
}

pub(crate) fn parse_main(path: &Path, raw: &str, max_depth: usize) -> Parsed {
    let mut sink = ParseSink {
        issues: Vec::new(),
        include_stack: Vec::new(),
        max_depth,
    };
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    sink.include_stack.push(canonical);
    let (metadata, blocks) = parse_file(path, raw, true, &mut sink);
    sink.include_stack.pop();
    Parsed {
        metadata,
        blocks,
        issues: sink.issues,
    }
}

fn parse_file(
    path: &Path,
    raw: &str,
    allow_metadata: bool,
    sink: &mut ParseSink,
) -> (Metadata, Vec<Block>) {
    let source = SourceText::new(path.display().to_string(), raw);
    let mut metadata = Metadata::new();
    let mut start_line = 0;
    if allow_metadata {
        let parsed = metadata::parse(&source, &mut sink.issues);
        metadata = parsed.metadata;
        start_line = parsed.next_line;
    }
    let mut parser = BlockParser {
        source: &source,
        sink,
        index: start_line,
        end_line: source.line_count(),
        blocks: Vec::new(),
    };
    parser.parse();
    (metadata, std::mem::take(&mut parser.blocks))
}

struct BlockParser<'a> {
    source: &'a SourceText,
    sink: &'a mut ParseSink,
    index: usize,
    end_line: usize,
    blocks: Vec<Block>,
}

impl BlockParser<'_> {
    fn parse(&mut self) {
        while self.index < self.end_line {
            if self.source.is_blank(self.index) {
                self.index += 1;
                continue;
            }
            self.parse_one();
        }
    }

    fn parse_one(&mut self) {
        let line = self.index;
        let text = &self.source.text;
        let start = self.source.after_indent(line);

        if let Some((level, content_start)) = heading_marker(self.source, line) {
            self.parse_heading(level, content_start);
            return;
        }
        if raw_fence(self.source, line).is_some() {
            self.parse_raw_block();
            return;
        }
        if let Some((kind, _, _)) = standalone_call(self.source, line) {
            self.parse_block_call(kind);
            return;
        }
        if include_keyword(text, start) {
            self.parse_include();
            return;
        }
        if code_keyword(text, start).is_some() {
            self.parse_code_construct();
            return;
        }
        if list_marker(self.source, line).is_some() {
            self.parse_list();
            return;
        }
        self.parse_paragraph();
    }

    fn parse_heading(&mut self, level: usize, content_start: usize) {
        let line = self.index;
        let end = self.source.line_end(line);
        let (content_start, content_end) = trim_range(&self.source.text, content_start, end);
        let source_span = self.source.line_span(line);
        if level > 6 {
            self.sink.issues.push(Issue::unsupported(
                "标题层级超过 6 级",
                self.source.snippet(content_start, content_end, 40),
                "按 6 级标题处理",
                Some(source_span.clone()),
            ));
        }
        let outcome = inline::parse(
            self.source,
            content_start,
            content_end,
            &[],
            &mut self.sink.issues,
        );
        let mut block = Block::heading(level.min(6) as u8, outcome.runs, source_span);
        if let Some((reason, impact)) = outcome.degraded {
            block = block.with_determinism(Determinism::NonDeterministic { reason, impact });
        }
        self.blocks.push(block);
        self.index += 1;
    }

    fn parse_raw_block(&mut self) {
        let line = self.index;
        let Some(fence) = raw_fence(self.source, line) else {
            self.index += 1;
            return;
        };
        self.sink.issues.push(Issue::unsupported(
            "不支持 Typst 原始块",
            self.source
                .snippet(self.source.line_start(line), self.source.line_end(line), 40),
            "内容按纯文本段落保留",
            Some(self.source.line_span(line)),
        ));
        self.index += 1;
        while self.index < self.end_line {
            let current = self.index;
            let start = self.source.after_indent(current);
            let end = self.source.line_end(current);
            let run = self.source.text[start..end]
                .chars()
                .take_while(|ch| *ch == '`')
                .count();
            if run >= fence {
                self.index += 1;
                break;
            }
            let (start, end) = trim_range(&self.source.text, start, self.source.line_end(current));
            if start < end {
                let run = vec![Inline::text(
                    &self.source.text[start..end],
                    self.source.span(start, end),
                )];
                self.blocks
                    .push(Block::paragraph(run, self.source.span(start, end)));
            }
            self.index += 1;
        }
    }

    fn parse_list(&mut self) {
        let first = self.index;
        let Some(marker) = list_marker(self.source, first) else {
            self.index += 1;
            return;
        };
        let (list, degraded) = self.parse_list_at(marker.indent, marker.ordered);
        let last = self.index.saturating_sub(1).max(first);
        let span = self
            .source
            .span(self.source.line_start(first), self.source.line_end(last));
        let mut block = Block::list(list, span);
        if let Some((reason, impact)) = degraded {
            block = block.with_determinism(Determinism::NonDeterministic { reason, impact });
        }
        self.blocks.push(block);
    }

    fn parse_list_at(
        &mut self,
        base_indent: usize,
        ordered: bool,
    ) -> (ListBlock, Option<(String, String)>) {
        let mut items = Vec::new();
        let mut degraded = None;
        while self.index < self.end_line {
            if self.source.is_blank(self.index) {
                break;
            }
            let Some(marker) = list_marker(self.source, self.index) else {
                break;
            };
            if marker.indent != base_indent || marker.ordered != ordered {
                break;
            }
            let line = self.index;
            let (content_start, content_end) = trim_range(
                &self.source.text,
                marker.content_start,
                self.source.line_end(line),
            );
            let mut content = self.inline_content(content_start, content_end, &mut degraded);
            self.index += 1;

            let mut children = Vec::new();
            while self.index < self.end_line && !self.source.is_blank(self.index) {
                let indent = self.source.indent_width(self.index);
                if indent <= base_indent {
                    break;
                }
                if let Some(child_marker) = list_marker(self.source, self.index) {
                    let (child, child_degraded) =
                        self.parse_list_at(child_marker.indent, child_marker.ordered);
                    merge_degraded(&mut degraded, child_degraded);
                    children.push(child);
                    continue;
                }
                let line = self.index;
                let start = self.source.after_indent(line);
                let (start, end) = trim_range(&self.source.text, start, self.source.line_end(line));
                if start < end {
                    if !content.is_empty() {
                        content.push(Inline::text(" ", self.source.span(start, start)));
                    }
                    content.extend(self.inline_content(start, end, &mut degraded));
                }
                self.index += 1;
            }
            items.push(ListItem { content, children });
        }
        let list = if ordered {
            ListBlock::ordered(1, items)
        } else {
            ListBlock::unordered(items)
        };
        (list, degraded)
    }

    fn inline_content(
        &mut self,
        start: usize,
        end: usize,
        degraded: &mut Option<(String, String)>,
    ) -> Vec<Inline> {
        let (start, end) = trim_range(&self.source.text, start, end);
        if start >= end {
            return Vec::new();
        }
        let outcome = inline::parse(self.source, start, end, &[], &mut self.sink.issues);
        merge_degraded(degraded, outcome.degraded);
        outcome.runs
    }

    fn parse_paragraph(&mut self) {
        let first = self.index;
        let mut end_line = first;
        while end_line < self.end_line {
            if self.source.is_blank(end_line) {
                break;
            }
            if end_line > first && is_block_start(self.source, end_line) {
                break;
            }
            end_line += 1;
        }
        let (start, _) = trim_range(
            &self.source.text,
            self.source.line_start(first),
            self.source.line_end(first),
        );
        let (_, end) = trim_range(
            &self.source.text,
            self.source.line_start(end_line - 1),
            self.source.line_end(end_line - 1),
        );
        if start < end {
            let outcome = inline::parse(self.source, start, end, &[], &mut self.sink.issues);
            if !outcome.runs.is_empty() {
                let mut block = Block::paragraph(outcome.runs, self.source.span(start, end));
                if let Some((reason, impact)) = outcome.degraded {
                    block =
                        block.with_determinism(Determinism::NonDeterministic { reason, impact });
                }
                self.blocks.push(block);
            }
        }
        self.index = end_line.max(self.index + 1).min(self.end_line);
    }

    fn parse_include(&mut self) {
        let line = self.index;
        let line_end = self.source.line_end(line);
        let text = &self.source.text;
        let start = self.source.after_indent(line);
        let after_ident = start + "#include".len();
        let spaced = skip_spaces(text, after_ident, line_end);
        let Some(parsed) = parse_string_literal(text, spaced, line_end) else {
            self.sink.issues.push(Issue::non_deterministic(
                "`include` 路径不是字面量",
                "include 路径需要求值",
                "该文件内容未进入 IR",
                Some(self.source.span(start, line_end)),
            ));
            self.index += 1;
            return;
        };
        if !text[parsed.next..line_end].trim().is_empty() {
            self.sink.issues.push(Issue::unsupported(
                "`include` 行尾存在多余内容",
                self.source.snippet(parsed.next, line_end, 40),
                "已忽略行尾内容",
                Some(self.source.span(parsed.next, line_end)),
            ));
        }

        let current = Path::new(&self.source.file);
        let base = current.parent().unwrap_or_else(|| Path::new("."));
        let resolved = if Path::new(&parsed.value).is_absolute() {
            PathBuf::from(&parsed.value)
        } else {
            base.join(&parsed.value)
        };
        let canonical = std::fs::canonicalize(&resolved).unwrap_or_else(|_| resolved.clone());

        if self.sink.include_stack.len() >= self.sink.max_depth {
            self.sink.issues.push(
                Issue::unsupported(
                    "include 嵌套过深",
                    self.source.snippet(start, line_end, 40),
                    format!("超过最大深度 {}，该文件未展开", self.sink.max_depth),
                    Some(self.source.span(start, line_end)),
                )
                .with_severity(Severity::Blocking),
            );
            self.index += 1;
            return;
        }
        if self
            .sink
            .include_stack
            .iter()
            .any(|path| path == &canonical)
        {
            self.sink.issues.push(
                Issue::unsupported(
                    "检测到循环 include",
                    self.source.snippet(start, line_end, 40),
                    "为避免无限展开，该引用已跳过",
                    Some(self.source.span(start, line_end)),
                )
                .with_severity(Severity::Blocking),
            );
            self.index += 1;
            return;
        }

        let raw = match std::fs::read_to_string(&resolved) {
            Ok(raw) => raw,
            Err(error) => {
                self.sink.issues.push(
                    Issue::unsupported(
                        format!("include 文件无法读取：{}", parsed.value),
                        self.source.snippet(start, line_end, 60),
                        format!("{error}；该文件内容未进入 IR"),
                        Some(self.source.span(start, line_end)),
                    )
                    .with_severity(Severity::Blocking),
                );
                self.index += 1;
                return;
            }
        };

        self.sink.include_stack.push(canonical);
        let (_, blocks) = parse_file(&resolved, &raw, false, self.sink);
        self.sink.include_stack.pop();
        self.blocks.extend(blocks);
        self.index += 1;
    }

    fn parse_block_call(&mut self, kind: BlockCallKind) {
        let line = self.index;
        let Some((_, open, close)) = standalone_call(self.source, line) else {
            self.index += 1;
            return;
        };
        let close_line = self.source.line_index(close);
        let span = self.source.span(self.source.line_start(line), close + 1);
        let result = match kind {
            BlockCallKind::Image => self
                .build_image(open, close, None)
                .map(|(image, degraded)| (Block::image(image, span.clone()), degraded)),
            BlockCallKind::Table => self
                .build_table(open, close, None)
                .map(|(table, degraded)| (Block::table(table, span.clone()), degraded)),
            BlockCallKind::Figure => self.parse_figure(open, close, span.clone()),
        };
        if let Some((block, degraded)) = result {
            let block = match degraded {
                Some((reason, impact)) => {
                    block.with_determinism(Determinism::NonDeterministic { reason, impact })
                }
                None => block,
            };
            self.blocks.push(block);
        }
        self.index = (close_line + 1).max(self.index + 1).min(self.end_line);
    }

    fn build_image(
        &mut self,
        open: usize,
        close: usize,
        figure_caption: Option<Vec<Inline>>,
    ) -> Option<(ImageBlock, Option<(String, String)>)> {
        let args = arguments(&self.source.text, open + 1, close);
        let mut path = None;
        let mut alt = None;
        let mut caption = figure_caption;
        let mut degraded = None;
        let mut ignored = Vec::new();
        for arg in args {
            match arg {
                Argument::Positional { start, end } => {
                    if path.is_some() {
                        self.sink.issues.push(Issue::unsupported(
                            "`#image` 存在多余的位置参数",
                            self.source.snippet(start, end, 40),
                            "仅使用第一个路径参数，其余已忽略",
                            Some(self.source.span(start, end)),
                        ));
                        continue;
                    }
                    match string_value(&self.source.text, start, end) {
                        Some(value) if !value.trim().is_empty() => path = Some(value),
                        _ => {
                            self.sink.issues.push(Issue::non_deterministic(
                                "`#image` 路径不是字面量",
                                "图片路径需要求值",
                                "该图片未进入 IR",
                                Some(self.source.span(start, end)),
                            ));
                            return None;
                        }
                    }
                }
                Argument::Named {
                    name,
                    value_start,
                    value_end,
                } => match name.as_str() {
                    "alt" => match string_value(&self.source.text, value_start, value_end) {
                        Some(value) => alt = Some(value),
                        None => self.sink.issues.push(Issue::unsupported(
                            "无法静态解析 `alt` 参数",
                            self.source.snippet(value_start, value_end, 40),
                            "已忽略该参数",
                            Some(self.source.span(value_start, value_end)),
                        )),
                    },
                    "caption" => {
                        if caption.is_none() {
                            self.parse_caption(value_start, value_end, &mut caption, &mut degraded);
                        }
                    }
                    "width" | "height" | "fit" => ignored.push(name),
                    other => self.sink.issues.push(Issue::unsupported(
                        format!("不支持的 `#image` 参数 `{other}`"),
                        self.source.snippet(value_start, value_end, 40),
                        "已忽略该参数",
                        Some(self.source.span(value_start, value_end)),
                    )),
                },
            }
        }
        if !ignored.is_empty() {
            self.sink.issues.push(Issue::unsupported(
                format!("`#image` 参数未采纳：{}", ignored.join("、")),
                self.source.snippet(open, close + 1, 60),
                "图片按后端最大宽高自动缩放",
                Some(self.source.span(open, close + 1)),
            ));
        }
        let Some(path) = path else {
            self.sink.issues.push(Issue::unsupported(
                "`#image` 缺少字面量路径",
                self.source.snippet(open, close + 1, 60),
                "该图片未进入 IR",
                Some(self.source.span(open, close + 1)),
            ));
            return None;
        };
        Some((
            ImageBlock {
                path,
                alt,
                title: None,
                caption,
            },
            degraded,
        ))
    }

    fn build_table(
        &mut self,
        open: usize,
        close: usize,
        figure_caption: Option<Vec<Inline>>,
    ) -> Option<(TableBlock, Option<(String, String)>)> {
        let args = arguments(&self.source.text, open + 1, close);
        let mut columns = None;
        let mut explicit_widths = false;
        let mut caption = figure_caption;
        let mut header_cells = Vec::new();
        let mut cells = Vec::new();
        let mut ignored = Vec::new();
        let mut degraded = None;

        for arg in args {
            match arg {
                Argument::Named {
                    name,
                    value_start,
                    value_end,
                } => match name.as_str() {
                    "columns" => match parse_columns(&self.source.text, value_start, value_end) {
                        Some((count, widths)) => {
                            columns = Some(count);
                            explicit_widths = widths;
                        }
                        None => self.sink.issues.push(Issue::unsupported(
                            "无法静态解析 `columns` 参数",
                            self.source.snippet(value_start, value_end, 40),
                            "已忽略该参数，按表头或单元格推断列数",
                            Some(self.source.span(value_start, value_end)),
                        )),
                    },
                    "caption" => {
                        if caption.is_none() {
                            self.parse_caption(value_start, value_end, &mut caption, &mut degraded);
                        }
                    }
                    "align" | "stroke" | "inset" | "fill" | "gutter" | "column-gutter"
                    | "row-gutter" | "breakable" => ignored.push(name),
                    other => self.sink.issues.push(Issue::unsupported(
                        format!("不支持的 `#table` 参数 `{other}`"),
                        self.source.snippet(value_start, value_end, 40),
                        "已忽略该参数",
                        Some(self.source.span(value_start, value_end)),
                    )),
                },
                Argument::Positional { start, end } => {
                    let (name, _) = dotted_identifier(&self.source.text, start, end);
                    match name.as_str() {
                        "table.header" | "header" => {
                            let mut header = self.parse_table_header(start, end, &mut degraded);
                            header_cells.append(&mut header);
                        }
                        "table.cell" | "cell" => {
                            if let Some(cell) =
                                self.parse_table_cell(start, end, &mut degraded, &mut ignored)
                            {
                                cells.push(cell);
                            }
                        }
                        "table.hline" | "hline" | "table.vline" | "vline" => {
                            ignored.push("表格线".to_owned());
                        }
                        "table.footer" | "footer" => {
                            self.sink.issues.push(Issue::unsupported(
                                "不支持 `table.footer`",
                                self.source.snippet(start, end, 40),
                                "已忽略表脚",
                                Some(self.source.span(start, end)),
                            ));
                        }
                        _ => match content_range(&self.source.text, start, end) {
                            Some((content_start, content_end)) => {
                                cells.push(TableCell::new(self.inline_content(
                                    content_start,
                                    content_end,
                                    &mut degraded,
                                )));
                            }
                            None => self.sink.issues.push(Issue::unsupported(
                                "暂不支持该表格参数",
                                self.source.snippet(start, end, 40),
                                "已忽略该参数",
                                Some(self.source.span(start, end)),
                            )),
                        },
                    }
                }
            }
        }

        if !ignored.is_empty() {
            ignored.sort();
            ignored.dedup();
            self.sink.issues.push(Issue::unsupported(
                format!("表格样式参数未采纳：{}", ignored.join("、")),
                self.source.snippet(open, close + 1, 60),
                "表格边框、内边距与对齐沿用后端样式",
                Some(self.source.span(open, close + 1)),
            ));
        }
        if explicit_widths {
            self.sink.issues.push(Issue::unsupported(
                "列宽设置未采纳",
                self.source.snippet(open, close + 1, 60),
                "按等宽列渲染",
                Some(self.source.span(open, close + 1)),
            ));
        }
        if header_cells.is_empty() && cells.is_empty() {
            self.sink.issues.push(Issue::unsupported(
                "`#table` 没有可用单元格",
                self.source.snippet(open, close + 1, 60),
                "该表格未进入 IR",
                Some(self.source.span(open, close + 1)),
            ));
            return None;
        }

        let column_count = match columns {
            Some(count) => count,
            None if !header_cells.is_empty() => header_cells.len(),
            None => {
                self.sink.issues.push(Issue::unsupported(
                    "`#table` 未指定 `columns`",
                    self.source.snippet(open, close + 1, 60),
                    "按单列布局；多列表格请显式写出 `columns: N`",
                    Some(self.source.span(open, close + 1)),
                ));
                1
            }
        };
        let header = chunk_rows(header_cells, column_count);
        let rows = chunk_rows(cells, column_count);
        Some((
            TableBlock {
                header,
                rows,
                column_alignments: Vec::new(),
                caption,
            },
            degraded,
        ))
    }

    fn parse_table_header(
        &mut self,
        start: usize,
        end: usize,
        degraded: &mut Option<(String, String)>,
    ) -> Vec<TableCell> {
        let (_, after) = dotted_identifier(&self.source.text, start, end);
        let mut cursor = skip_spaces(&self.source.text, after, end);
        let mut cells = Vec::new();
        if cursor < end && self.source.text.as_bytes()[cursor] == b'(' {
            let Some(close) = find_matching(&self.source.text, cursor, end) else {
                self.sink.issues.push(Issue::unsupported(
                    "`table.header` 参数列表未闭合",
                    self.source.snippet(start, end, 40),
                    "已忽略该表头",
                    Some(self.source.span(start, end)),
                ));
                return cells;
            };
            for arg in arguments(&self.source.text, cursor + 1, close) {
                match arg {
                    Argument::Positional { start, end } => {
                        match content_range(&self.source.text, start, end) {
                            Some((content_start, content_end)) => cells.push(TableCell::new(
                                self.inline_content(content_start, content_end, degraded),
                            )),
                            None => self.sink.issues.push(Issue::unsupported(
                                "`table.header` 仅支持内容块参数",
                                self.source.snippet(start, end, 40),
                                "已忽略该参数",
                                Some(self.source.span(start, end)),
                            )),
                        }
                    }
                    Argument::Named { name, .. } => {
                        if name == "repeat" || name == "level" {
                            self.sink.issues.push(Issue::unsupported(
                                format!("`table.header` 的 `{name}` 参数未采纳"),
                                self.source.snippet(start, end, 40),
                                "表头按单次表头行渲染，不设置跨页重复",
                                Some(self.source.span(start, end)),
                            ));
                        } else {
                            self.sink.issues.push(Issue::unsupported(
                                format!("不支持的 `table.header` 参数 `{name}`"),
                                self.source.snippet(start, end, 40),
                                "已忽略该参数",
                                Some(self.source.span(start, end)),
                            ));
                        }
                    }
                }
            }
            cursor = close + 1;
        }
        if cursor < end {
            match content_block_sequence(&self.source.text, cursor, end) {
                Some(blocks) => {
                    for (content_start, content_end) in blocks {
                        cells.push(TableCell::new(self.inline_content(
                            content_start,
                            content_end,
                            degraded,
                        )));
                    }
                }
                None => self.sink.issues.push(Issue::unsupported(
                    "`table.header` 尾部存在非内容块参数",
                    self.source.snippet(cursor, end, 40),
                    "已忽略无法识别的部分",
                    Some(self.source.span(cursor, end)),
                )),
            }
        }
        if cells.is_empty() {
            self.sink.issues.push(Issue::unsupported(
                "`table.header` 没有内容块",
                self.source.snippet(start, end, 40),
                "已忽略该表头",
                Some(self.source.span(start, end)),
            ));
        }
        cells
    }

    fn parse_table_cell(
        &mut self,
        start: usize,
        end: usize,
        degraded: &mut Option<(String, String)>,
        ignored: &mut Vec<String>,
    ) -> Option<TableCell> {
        let (_, after) = dotted_identifier(&self.source.text, start, end);
        let mut cursor = skip_spaces(&self.source.text, after, end);
        let mut body = None;
        let mut colspan = 1u32;
        let mut rowspan = 1u32;
        let mut alignment = None;
        if cursor < end && self.source.text.as_bytes()[cursor] == b'(' {
            let Some(close) = find_matching(&self.source.text, cursor, end) else {
                self.sink.issues.push(Issue::unsupported(
                    "`table.cell` 参数列表未闭合",
                    self.source.snippet(start, end, 40),
                    "已忽略该单元格",
                    Some(self.source.span(start, end)),
                ));
                return None;
            };
            for arg in arguments(&self.source.text, cursor + 1, close) {
                match arg {
                    Argument::Positional { start, end } => {
                        match content_range(&self.source.text, start, end) {
                            Some(range) if body.is_none() => body = Some(range),
                            Some(_) => self.sink.issues.push(Issue::unsupported(
                                "`table.cell` 存在多个位置参数",
                                self.source.snippet(start, end, 40),
                                "仅使用第一个内容块",
                                Some(self.source.span(start, end)),
                            )),
                            None => self.sink.issues.push(Issue::unsupported(
                                "`table.cell` 仅支持内容块参数",
                                self.source.snippet(start, end, 40),
                                "已忽略该参数",
                                Some(self.source.span(start, end)),
                            )),
                        }
                    }
                    Argument::Named {
                        name,
                        value_start,
                        value_end,
                    } => match name.as_str() {
                        "colspan" => {
                            match parse_positive_int(&self.source.text, value_start, value_end) {
                                Some(value) => colspan = value,
                                None => self.sink.issues.push(Issue::unsupported(
                                    "`colspan` 需要正整数",
                                    self.source.snippet(value_start, value_end, 40),
                                    "按 1 处理",
                                    Some(self.source.span(value_start, value_end)),
                                )),
                            }
                        }
                        "rowspan" => {
                            match parse_positive_int(&self.source.text, value_start, value_end) {
                                Some(value) => rowspan = value,
                                None => self.sink.issues.push(Issue::unsupported(
                                    "`rowspan` 需要正整数",
                                    self.source.snippet(value_start, value_end, 40),
                                    "按 1 处理",
                                    Some(self.source.span(value_start, value_end)),
                                )),
                            }
                        }
                        "align" => {
                            match parse_alignment(&self.source.text, value_start, value_end) {
                                Some(value) => alignment = Some(value),
                                None => self.sink.issues.push(Issue::unsupported(
                                    "无法静态解析单元格 `align`",
                                    self.source.snippet(value_start, value_end, 40),
                                    "已忽略该参数",
                                    Some(self.source.span(value_start, value_end)),
                                )),
                            }
                        }
                        "fill" | "inset" | "stroke" | "breakable" => ignored.push(name),
                        other => self.sink.issues.push(Issue::unsupported(
                            format!("不支持的 `table.cell` 参数 `{other}`"),
                            self.source.snippet(value_start, value_end, 40),
                            "已忽略该参数",
                            Some(self.source.span(value_start, value_end)),
                        )),
                    },
                }
            }
            cursor = close + 1;
        }
        if cursor < end {
            match content_block_sequence(&self.source.text, cursor, end) {
                Some(blocks) => {
                    if body.is_none() {
                        body = blocks.first().copied();
                    }
                    if blocks.len() > 1 {
                        self.sink.issues.push(Issue::unsupported(
                            "`table.cell` 存在多个内容块",
                            self.source.snippet(cursor, end, 40),
                            "仅使用第一个内容块",
                            Some(self.source.span(cursor, end)),
                        ));
                    }
                }
                None => self.sink.issues.push(Issue::unsupported(
                    "`table.cell` 尾部存在非内容块参数",
                    self.source.snippet(cursor, end, 40),
                    "已忽略无法识别的部分",
                    Some(self.source.span(cursor, end)),
                )),
            }
        }
        let content = match body {
            Some((content_start, content_end)) => {
                self.inline_content(content_start, content_end, degraded)
            }
            None => Vec::new(),
        };
        Some(TableCell {
            content,
            colspan,
            rowspan,
            alignment,
        })
    }

    fn parse_caption(
        &mut self,
        value_start: usize,
        value_end: usize,
        caption: &mut Option<Vec<Inline>>,
        degraded: &mut Option<(String, String)>,
    ) {
        match content_range(&self.source.text, value_start, value_end) {
            Some((start, end)) => {
                *caption = Some(self.inline_content(start, end, degraded));
            }
            None => self.sink.issues.push(Issue::unsupported(
                "题注参数需要内容块",
                self.source.snippet(value_start, value_end, 40),
                "已忽略该参数",
                Some(self.source.span(value_start, value_end)),
            )),
        }
    }

    fn parse_figure(
        &mut self,
        open: usize,
        close: usize,
        span: SourceSpan,
    ) -> Option<(Block, Option<(String, String)>)> {
        let args = arguments(&self.source.text, open + 1, close);
        let mut caption = None;
        let mut body = None;
        let mut ignored = Vec::new();
        let mut degraded = None;
        for arg in args {
            match arg {
                Argument::Positional { start, end } => {
                    if body.is_some() {
                        self.sink.issues.push(Issue::unsupported(
                            "`#figure` 仅支持一个内容参数",
                            self.source.snippet(start, end, 40),
                            "已忽略多余内容",
                            Some(self.source.span(start, end)),
                        ));
                        continue;
                    }
                    body = Some((start, end));
                }
                Argument::Named {
                    name,
                    value_start,
                    value_end,
                } => match name.as_str() {
                    "caption" => {
                        if caption.is_none() {
                            self.parse_caption(value_start, value_end, &mut caption, &mut degraded);
                        }
                    }
                    "gap" | "kind" | "supplement" | "numbering" | "placement" | "scope" => {
                        ignored.push(name);
                    }
                    other => self.sink.issues.push(Issue::unsupported(
                        format!("不支持的 `#figure` 参数 `{other}`"),
                        self.source.snippet(value_start, value_end, 40),
                        "已忽略该参数",
                        Some(self.source.span(value_start, value_end)),
                    )),
                },
            }
        }
        if !ignored.is_empty() {
            self.sink.issues.push(Issue::unsupported(
                format!("`#figure` 参数未采纳：{}", ignored.join("、")),
                self.source.snippet(open, close + 1, 60),
                "仅应用题注文字，编号与补充文字沿用后端规则",
                Some(self.source.span(open, close + 1)),
            ));
        }
        let Some((body_start, body_end)) = body else {
            self.sink.issues.push(Issue::unsupported(
                "`#figure` 缺少内容参数",
                self.source.snippet(open, close + 1, 60),
                "该图未进入 IR",
                Some(span),
            ));
            return None;
        };
        if let Some((inner_open, inner_close)) =
            inner_call(&self.source.text, body_start, body_end, "image")
        {
            let (image, image_degraded) = self.build_image(inner_open, inner_close, caption)?;
            merge_degraded(&mut degraded, image_degraded);
            return Some((Block::image(image, span), degraded));
        }
        if let Some((inner_open, inner_close)) =
            inner_call(&self.source.text, body_start, body_end, "table")
        {
            let (table, table_degraded) = self.build_table(inner_open, inner_close, caption)?;
            merge_degraded(&mut degraded, table_degraded);
            return Some((Block::table(table, span), degraded));
        }
        self.sink.issues.push(Issue::unsupported(
            "`#figure` 内容暂不支持",
            self.source.snippet(body_start, body_end, 60),
            "仅支持 `image(...)` 或 `table(...)` 作为 figure 内容；该图未进入 IR",
            Some(self.source.span(body_start, body_end)),
        ));
        None
    }

    fn parse_code_construct(&mut self) {
        let line = self.index;
        let text = &self.source.text;
        let start = self.source.after_indent(line);
        let (ident, after_ident) = match code_keyword(text, start) {
            Some(found) => found,
            None => {
                self.index += 1;
                return;
            }
        };

        let (reason, impact) = match ident.as_str() {
            "for" | "while" => (
                "循环生成的内容不可静态确定",
                "循环体按字面量提取一次；实际重复次数与变量替换未知",
            ),
            "if" => (
                "条件分支不可静态确定",
                "按 then 分支字面量提取一次；未评估条件，else 分支被忽略",
            ),
            "show" => (
                "show 规则需要求值",
                "规则未应用；受其影响的内容样式可能与源文件不同",
            ),
            "import" => (
                "import 符号产出的内容不可静态确定",
                "依赖该符号的内容未进入 IR 或仅按字面量处理",
            ),
            "let" => ("let 绑定需要求值", "绑定值未展开；后续变量引用可能缺失"),
            "set" => {
                if set_targets_text(self.source, after_ident) {
                    (
                        "顶层 #set text 的生效范围需要求值",
                        "该设置未应用；后续内容按目标后端的样式渲染",
                    )
                } else {
                    ("set 规则需要求值", "该设置未应用")
                }
            }
            _ => ("未知的 Typst 构造", "按字面量处理"),
        };

        let mut construct_end = statement_end(text, start, text.len());
        if matches!(ident.as_str(), "for" | "while" | "if") {
            if let Some((open, close)) = find_content_block(text, after_ident, text.len()) {
                self.parse_degraded_range(open + 1, close, reason, impact);
                construct_end = close + 1;
                if ident == "if" {
                    construct_end = consume_else(text, construct_end);
                }
            } else {
                self.sink.issues.push(Issue::unsupported(
                    format!("无法定位 `#{ident}` 的内容块"),
                    self.source.snippet(start, construct_end, 40),
                    "已丢弃该代码",
                    Some(self.source.span(start, construct_end)),
                ));
            }
        }

        self.sink.issues.push(Issue::non_deterministic(
            format!("`#{ident}` 不可静态确定"),
            reason,
            impact,
            Some(self.source.span(start, construct_end)),
        ));
        let end_byte = construct_end.saturating_sub(1).max(start);
        let end_line = self.source.line_index(end_byte);
        self.index = (end_line + 1).max(self.index + 1).min(self.end_line);
    }

    fn parse_degraded_range(
        &mut self,
        inner_start: usize,
        inner_end: usize,
        reason: &str,
        impact: &str,
    ) {
        if inner_start >= inner_end {
            return;
        }
        let source = self.source;
        let start_line = source.line_index(inner_start);
        let last_byte = inner_end.saturating_sub(1).max(inner_start);
        let end_line = source.line_index(last_byte);
        let mut groups = Vec::new();
        let mut group_start: Option<usize> = None;
        let mut group_end = inner_start;
        for line in start_line..=end_line {
            let line_start = source.line_start(line).max(inner_start);
            let line_end = source.line_end(line).min(inner_end);
            let (start, end) = trim_range(&source.text, line_start, line_end);
            if start >= end {
                if let Some(group_start) = group_start.take() {
                    groups.push((group_start, group_end));
                }
            } else {
                group_start.get_or_insert(start);
                group_end = end;
            }
        }
        if let Some(group_start) = group_start.take() {
            groups.push((group_start, group_end));
        }
        for (start, end) in groups {
            flush_degraded(
                self.source,
                &mut self.blocks,
                &mut self.sink.issues,
                start,
                end,
                reason,
                impact,
            );
        }
    }
}

fn flush_degraded(
    source: &SourceText,
    blocks: &mut Vec<Block>,
    issues: &mut Vec<Issue>,
    start: usize,
    end: usize,
    reason: &str,
    impact: &str,
) {
    if start >= end {
        return;
    }
    let outcome = inline::parse(source, start, end, &[], issues);
    if outcome.runs.is_empty() {
        return;
    }
    blocks.push(
        Block::paragraph(outcome.runs, source.span(start, end)).with_determinism(
            Determinism::NonDeterministic {
                reason: reason.to_owned(),
                impact: impact.to_owned(),
            },
        ),
    );
}

fn is_block_start(source: &SourceText, line: usize) -> bool {
    if source.is_blank(line) {
        return false;
    }
    let text = &source.text;
    let start = source.after_indent(line);
    heading_marker(source, line).is_some()
        || raw_fence(source, line).is_some()
        || standalone_call(source, line).is_some()
        || include_keyword(text, start)
        || code_keyword(text, start).is_some()
        || list_marker(source, line).is_some()
}

fn heading_marker(source: &SourceText, line: usize) -> Option<(usize, usize)> {
    if source.indent_width(line) > 3 {
        return None;
    }
    let start = source.after_indent(line);
    let end = source.line_end(line);
    let text = &source.text[start..end];
    let level = text.chars().take_while(|ch| *ch == '=').count();
    if level == 0 {
        return None;
    }
    let after = start + level;
    if let Some(next) = source.text[after..end].chars().next() {
        if !next.is_whitespace() {
            return None;
        }
    }
    let content_start = skip_spaces(&source.text, after, end);
    Some((level, content_start))
}

fn raw_fence(source: &SourceText, line: usize) -> Option<usize> {
    if source.indent_width(line) > 3 {
        return None;
    }
    let start = source.after_indent(line);
    let end = source.line_end(line);
    let run = source.text[start..end]
        .chars()
        .take_while(|ch| *ch == '`')
        .count();
    (run >= 3).then_some(run)
}

fn include_keyword(text: &str, start: usize) -> bool {
    if !text[start..].starts_with('#') {
        return false;
    }
    let (ident, after) = read_identifier(text, start + 1, text.len());
    ident == "include"
        && (after == text.len()
            || text[after..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_whitespace() || ch == '"'))
}

fn code_keyword(text: &str, start: usize) -> Option<(String, usize)> {
    if !text[start..].starts_with('#') {
        return None;
    }
    let (ident, after) = read_identifier(text, start + 1, text.len());
    match ident.as_str() {
        "for" | "while" | "if" | "show" | "set" | "import" | "let" => Some((ident, after)),
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct ListMarker {
    ordered: bool,
    indent: usize,
    content_start: usize,
}

fn list_marker(source: &SourceText, line: usize) -> Option<ListMarker> {
    let indent = source.indent_width(line);
    let start = source.after_indent(line);
    let end = source.line_end(line);
    let ch = source.text[start..end].chars().next()?;
    if ch != '-' && ch != '+' {
        return None;
    }
    let after = start + ch.len_utf8();
    if after < end {
        let next = source.text[after..end].chars().next().unwrap();
        if next != ' ' && next != '\t' {
            return None;
        }
    }
    let content_start = skip_spaces(&source.text, after, end);
    Some(ListMarker {
        ordered: ch == '+',
        indent,
        content_start,
    })
}

fn merge_degraded(target: &mut Option<(String, String)>, candidate: Option<(String, String)>) {
    if target.is_none() {
        *target = candidate;
    }
}

enum Argument {
    Positional {
        start: usize,
        end: usize,
    },
    Named {
        name: String,
        value_start: usize,
        value_end: usize,
    },
}

fn arguments(text: &str, start: usize, end: usize) -> Vec<Argument> {
    let mut args = Vec::new();
    for (part_start, part_end) in split_top_level(text, start, end, ',') {
        let (part_start, part_end) = trim_range(text, part_start, part_end);
        if part_start >= part_end {
            continue;
        }
        if let Some(colon) = find_top_level_colon(text, part_start, part_end) {
            let key = text[part_start..colon].trim();
            if is_name(key) {
                let (value_start, value_end) = trim_range(text, colon + 1, part_end);
                args.push(Argument::Named {
                    name: key.to_owned(),
                    value_start,
                    value_end,
                });
                continue;
            }
        }
        args.push(Argument::Positional {
            start: part_start,
            end: part_end,
        });
    }
    args
}

fn is_name(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-')
}

fn string_value(text: &str, start: usize, end: usize) -> Option<String> {
    let (start, end) = trim_range(text, start, end);
    let parsed = parse_string_literal(text, start, end)?;
    (parsed.next == end).then_some(parsed.value)
}

fn content_range(text: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let (start, end) = trim_range(text, start, end);
    if start >= end || text.as_bytes()[start] != b'[' {
        return None;
    }
    let close = find_matching(text, start, end)?;
    (close + 1 == end).then_some((start + 1, close))
}

fn content_block_sequence(text: &str, start: usize, end: usize) -> Option<Vec<(usize, usize)>> {
    let mut cursor = start;
    let mut blocks = Vec::new();
    while cursor < end {
        cursor = skip_spaces(text, cursor, end);
        if cursor >= end {
            break;
        }
        if text.as_bytes()[cursor] != b'[' {
            return None;
        }
        let close = find_matching(text, cursor, end)?;
        blocks.push((cursor + 1, close));
        cursor = close + 1;
    }
    (!blocks.is_empty()).then_some(blocks)
}

fn dotted_identifier(text: &str, start: usize, end: usize) -> (String, usize) {
    let mut index = start;
    while index < end {
        let ch = text[index..].chars().next().unwrap();
        if ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.') {
            index += ch.len_utf8();
        } else {
            break;
        }
    }
    (text[start..index].to_owned(), index)
}

fn parse_columns(text: &str, start: usize, end: usize) -> Option<(usize, bool)> {
    let (start, end) = trim_range(text, start, end);
    if start >= end {
        return None;
    }
    if let Ok(count) = text[start..end].parse::<usize>() {
        return (count > 0).then_some((count, false));
    }
    if text.as_bytes()[start] != b'(' {
        return None;
    }
    let close = find_matching(text, start, end)?;
    if close + 1 != end {
        return None;
    }
    let mut count = 0;
    let mut explicit_widths = false;
    for (part_start, part_end) in split_top_level(text, start + 1, close, ',') {
        let (part_start, part_end) = trim_range(text, part_start, part_end);
        if part_start >= part_end {
            continue;
        }
        count += 1;
        if &text[part_start..part_end] != "auto" {
            explicit_widths = true;
        }
    }
    (count > 0).then_some((count, explicit_widths))
}

fn parse_positive_int(text: &str, start: usize, end: usize) -> Option<u32> {
    let (start, end) = trim_range(text, start, end);
    let value = text[start..end].parse::<u32>().ok()?;
    (value > 0).then_some(value)
}

fn parse_alignment(text: &str, start: usize, end: usize) -> Option<Alignment> {
    let (start, end) = trim_range(text, start, end);
    match text[start..end].trim() {
        "left" => Some(Alignment::Left),
        "center" => Some(Alignment::Center),
        "right" => Some(Alignment::Right),
        "justify" => Some(Alignment::Justify),
        _ => None,
    }
}

fn chunk_rows(cells: Vec<TableCell>, columns: usize) -> Vec<TableRow> {
    let columns = columns.max(1);
    let mut rows = Vec::new();
    let mut current = Vec::new();
    let mut used = 0usize;
    for cell in cells {
        let span = cell.colspan.max(1) as usize;
        if used > 0 && used + span > columns {
            rows.push(TableRow::new(std::mem::take(&mut current)));
            used = 0;
        }
        used += span;
        current.push(cell);
        if used >= columns {
            rows.push(TableRow::new(std::mem::take(&mut current)));
            used = 0;
        }
    }
    if !current.is_empty() {
        rows.push(TableRow::new(current));
    }
    rows
}

fn inner_call(text: &str, start: usize, end: usize, name: &str) -> Option<(usize, usize)> {
    let (start, end) = trim_range(text, start, end);
    if start >= end {
        return None;
    }
    let mut cursor = start;
    if text.as_bytes()[cursor] == b'#' {
        cursor = skip_spaces(text, cursor + 1, end);
    }
    let (ident, after) = read_identifier(text, cursor, end);
    if ident != name {
        return None;
    }
    let open = skip_spaces(text, after, end);
    if open >= end || text.as_bytes()[open] != b'(' {
        return None;
    }
    let close = find_matching(text, open, end)?;
    (close + 1 == end).then_some((open, close))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BlockCallKind {
    Image,
    Table,
    Figure,
}

fn standalone_call(source: &SourceText, line: usize) -> Option<(BlockCallKind, usize, usize)> {
    if source.indent_width(line) > 3 {
        return None;
    }
    let start = source.after_indent(line);
    let text = &source.text;
    if !text[start..].starts_with('#') {
        return None;
    }
    let (ident, after) = read_identifier(text, start + 1, text.len());
    let kind = match ident.as_str() {
        "image" => BlockCallKind::Image,
        "table" => BlockCallKind::Table,
        "figure" => BlockCallKind::Figure,
        _ => return None,
    };
    let open = skip_spaces(text, after, text.len());
    if open >= text.len() || text.as_bytes()[open] != b'(' {
        return None;
    }
    let close = find_matching(text, open, text.len())?;
    let close_line = source.line_index(close);
    if !text[close + 1..source.line_end(close_line)]
        .trim()
        .is_empty()
    {
        return None;
    }
    Some((kind, open, close))
}

fn set_targets_text(source: &SourceText, after_ident: usize) -> bool {
    let end = source.line_end(source.line_index(after_ident));
    let start = skip_spaces(&source.text, after_ident, end);
    let (ident, after) = read_identifier(&source.text, start, end);
    ident == "text"
        && source.text[after..end]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_whitespace() || ch == '(')
}

fn consume_else(text: &str, start: usize) -> usize {
    let mut index = start;
    while index < text.len() {
        match text[index..].chars().next().unwrap() {
            ' ' | '\t' | '\r' | '\n' => index += 1,
            _ => break,
        }
    }
    if !text[index..].starts_with("else") {
        return start;
    }
    let after_else = index + 4;
    let spaced = skip_spaces(text, after_else, text.len());
    if spaced < text.len() && text.as_bytes()[spaced] == b'[' {
        if let Some(close) = find_matching(text, spaced, text.len()) {
            return close + 1;
        }
    }
    start
}

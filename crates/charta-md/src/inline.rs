//! Inline Markdown parsing for the controlled subset.
//!
//! Emphasis is structural. Every other construct that Markdown allows but the
//! controlled subset excludes is reported and degraded explicitly.

use crate::line::{snippet, span, Line};
use charta::{Inline, Issue};

pub(crate) fn parse(
    file: &str,
    line: &Line<'_>,
    start: usize,
    end: usize,
    issues: &mut Vec<Issue>,
) -> Vec<Inline> {
    let mut runs = Vec::new();
    parse_segment(file, line, start, end, false, false, &mut runs, issues);
    runs
}

#[allow(clippy::too_many_arguments)]
fn parse_segment(
    file: &str,
    line: &Line<'_>,
    start: usize,
    end: usize,
    strong: bool,
    emphasis: bool,
    runs: &mut Vec<Inline>,
    issues: &mut Vec<Issue>,
) {
    let text = line.text;
    let mut index = start;
    while index < end {
        let ch = text[index..].chars().next().unwrap();
        match ch {
            '\\' => {
                let next = index + 1;
                if next < end {
                    let escaped = text[next..].chars().next().unwrap();
                    let after = next + escaped.len_utf8();
                    push_run(
                        runs,
                        &text[next..after],
                        strong,
                        emphasis,
                        span(file, line, index, after),
                    );
                    index = after;
                } else {
                    push_run(runs, "\\", strong, emphasis, span(file, line, index, end));
                    index = end;
                }
            }
            '*' | '_' => {
                if let Some((delimiter, add_strong, add_emphasis)) = emphasis_marker(&text[index..])
                {
                    let inner_start = index + delimiter.len();
                    if let Some(close) = text[inner_start..end].find(delimiter) {
                        let inner_end = inner_start + close;
                        if inner_end > inner_start {
                            parse_segment(
                                file,
                                line,
                                inner_start,
                                inner_end,
                                strong || add_strong,
                                emphasis || add_emphasis,
                                runs,
                                issues,
                            );
                            index = inner_end + delimiter.len();
                            continue;
                        }
                    }
                    let marker_end = (index + delimiter.len()).min(end);
                    issues.push(Issue::unsupported(
                        "强调标记未闭合，按字面文本处理",
                        snippet(&text[index..marker_end], 8),
                        "按字面文本保留",
                        Some(span(file, line, index, marker_end)),
                    ));
                    push_run(
                        runs,
                        delimiter,
                        strong,
                        emphasis,
                        span(file, line, index, marker_end),
                    );
                    index = marker_end;
                } else {
                    let after = index + ch.len_utf8();
                    push_run(
                        runs,
                        &text[index..after],
                        strong,
                        emphasis,
                        span(file, line, index, after),
                    );
                    index = after;
                }
            }
            '`' => {
                if let Some(close) = text[index + 1..end].find('`') {
                    let content_start = index + 1;
                    let content_end = content_start + close;
                    let link_end = content_end + 1;
                    issues.push(Issue::unsupported(
                        "不支持行内代码",
                        snippet(&text[index..link_end], 40),
                        "移除反引号，内容按普通文本保留",
                        Some(span(file, line, index, link_end)),
                    ));
                    push_run(
                        runs,
                        &text[content_start..content_end],
                        strong,
                        emphasis,
                        span(file, line, content_start, content_end),
                    );
                    index = link_end;
                } else {
                    let after = index + 1;
                    push_run(runs, "`", strong, emphasis, span(file, line, index, after));
                    index = after;
                }
            }
            '!' => {
                if text[index..].starts_with("![") {
                    if let Some((alt_start, alt_end, link_end)) =
                        inline_image_range(text, index, end)
                    {
                        issues.push(Issue::unsupported(
                            "不支持行内图片",
                            snippet(&text[index..link_end], 40),
                            "保留替代文字，丢弃图片",
                            Some(span(file, line, index, link_end)),
                        ));
                        push_run(
                            runs,
                            &text[alt_start..alt_end],
                            strong,
                            emphasis,
                            span(file, line, alt_start, alt_end),
                        );
                        index = link_end;
                        continue;
                    }
                }
                let after = index + 1;
                push_run(runs, "!", strong, emphasis, span(file, line, index, after));
                index = after;
            }
            '[' => {
                if let Some(footnote_end) = footnote_reference_end(text, index, end) {
                    issues.push(Issue::unsupported(
                        "不支持脚注引用",
                        snippet(&text[index..footnote_end], 40),
                        "按字面文本保留",
                        Some(span(file, line, index, footnote_end)),
                    ));
                    push_run(
                        runs,
                        &text[index..footnote_end],
                        strong,
                        emphasis,
                        span(file, line, index, footnote_end),
                    );
                    index = footnote_end;
                } else if let Some((label_start, label_end, reference_end)) =
                    reference_link_range(text, index, end)
                {
                    issues.push(Issue::unsupported(
                        "不支持引用式链接",
                        snippet(&text[index..reference_end], 40),
                        "保留链接文字，丢弃引用标签",
                        Some(span(file, line, index, reference_end)),
                    ));
                    parse_segment(
                        file,
                        line,
                        label_start,
                        label_end,
                        strong,
                        emphasis,
                        runs,
                        issues,
                    );
                    index = reference_end;
                } else if let Some((label_start, label_end, link_end)) =
                    link_range(text, index, end)
                {
                    issues.push(Issue::unsupported(
                        "不支持链接",
                        snippet(&text[index..link_end], 40),
                        "保留链接文字，丢弃 URL",
                        Some(span(file, line, index, link_end)),
                    ));
                    parse_segment(
                        file,
                        line,
                        label_start,
                        label_end,
                        strong,
                        emphasis,
                        runs,
                        issues,
                    );
                    index = link_end;
                } else {
                    let after = index + 1;
                    push_run(runs, "[", strong, emphasis, span(file, line, index, after));
                    index = after;
                }
            }
            '~' if text[index..].starts_with("~~") => {
                if let Some(close) = text[index + 2..end].find("~~") {
                    let content_start = index + 2;
                    let content_end = content_start + close;
                    let construct_end = content_end + 2;
                    issues.push(Issue::unsupported(
                        "不支持删除线",
                        snippet(&text[index..construct_end], 40),
                        "移除删除线标记，文字按普通文本保留",
                        Some(span(file, line, index, construct_end)),
                    ));
                    parse_segment(
                        file,
                        line,
                        content_start,
                        content_end,
                        strong,
                        emphasis,
                        runs,
                        issues,
                    );
                    index = construct_end;
                } else {
                    let after = index + 2.min(end - index);
                    push_run(
                        runs,
                        &text[index..after],
                        strong,
                        emphasis,
                        span(file, line, index, after),
                    );
                    index = after;
                }
            }
            '<' => {
                if let Some(close) = text[index..end].find('>') {
                    let construct_end = index + close + 1;
                    issues.push(Issue::unsupported(
                        "不支持内联 HTML 或自动链接",
                        snippet(&text[index..construct_end], 40),
                        "按字面文本保留",
                        Some(span(file, line, index, construct_end)),
                    ));
                    push_run(
                        runs,
                        &text[index..construct_end],
                        strong,
                        emphasis,
                        span(file, line, index, construct_end),
                    );
                    index = construct_end;
                } else {
                    let after = index + 1;
                    push_run(runs, "<", strong, emphasis, span(file, line, index, after));
                    index = after;
                }
            }
            '&' => {
                if let Some((entity_end, decoded)) = entity(text, index, end) {
                    issues.push(Issue::unsupported(
                        "不支持 HTML 实体",
                        snippet(&text[index..entity_end], 40),
                        if decoded.is_some() {
                            "已解码为对应字符"
                        } else {
                            "按字面文本保留"
                        },
                        Some(span(file, line, index, entity_end)),
                    ));
                    let value = decoded.unwrap_or_else(|| &text[index..entity_end]);
                    push_run(
                        runs,
                        value,
                        strong,
                        emphasis,
                        span(file, line, index, entity_end),
                    );
                    index = entity_end;
                } else {
                    let after = index + 1;
                    push_run(runs, "&", strong, emphasis, span(file, line, index, after));
                    index = after;
                }
            }
            _ => {
                let after_char = index + ch.len_utf8();
                let next = next_special(text, after_char, end);
                push_run(
                    runs,
                    &text[index..next],
                    strong,
                    emphasis,
                    span(file, line, index, next),
                );
                index = next;
            }
        }
    }
}

fn emphasis_marker(text: &str) -> Option<(&'static str, bool, bool)> {
    if text.starts_with("***") {
        Some(("***", true, true))
    } else if text.starts_with("**") {
        Some(("**", true, false))
    } else if text.starts_with('*') {
        Some(("*", false, true))
    } else if text.starts_with("___") {
        Some(("___", true, true))
    } else if text.starts_with("__") {
        Some(("__", true, false))
    } else if text.starts_with('_') {
        Some(("_", false, true))
    } else {
        None
    }
}

fn next_special(text: &str, from: usize, end: usize) -> usize {
    let mut index = from;
    while index < end {
        let ch = text[index..].chars().next().unwrap();
        if matches!(ch, '\\' | '*' | '_' | '`' | '[' | '!' | '<' | '~' | '&') {
            return index;
        }
        index += ch.len_utf8();
    }
    end
}

fn link_range(text: &str, start: usize, end: usize) -> Option<(usize, usize, usize)> {
    let close = text[start + 1..end].find("](")?;
    let label_end = start + 1 + close;
    let url_start = label_end + 2;
    let close_paren = text[url_start..end].find(')')?;
    let url_end = url_start + close_paren;
    if url_end == url_start {
        return None;
    }
    Some((start + 1, label_end, url_end + 1))
}

fn footnote_reference_end(text: &str, start: usize, end: usize) -> Option<usize> {
    if !text[start..end].starts_with("[^") {
        return None;
    }
    let close = text[start + 2..end].find(']')?;
    Some(start + 2 + close + 1)
}

fn reference_link_range(text: &str, start: usize, end: usize) -> Option<(usize, usize, usize)> {
    let label_end = start + 1 + text[start + 1..end].find(']')?;
    let label_ref_start = label_end + 1;
    if label_ref_start >= end || text.as_bytes()[label_ref_start] != b'[' {
        return None;
    }
    let ref_end = label_ref_start + 1 + text[label_ref_start + 1..end].find(']')?;
    Some((start + 1, label_end, ref_end + 1))
}

fn inline_image_range(text: &str, start: usize, end: usize) -> Option<(usize, usize, usize)> {
    let close = text[start + 2..end].find("](")?;
    let alt_end = start + 2 + close;
    let path_start = alt_end + 2;
    let close_paren = text[path_start..end].find(')')?;
    let path_end = path_start + close_paren;
    if path_end == path_start {
        return None;
    }
    Some((start + 2, alt_end, path_end + 1))
}

fn entity(text: &str, start: usize, end: usize) -> Option<(usize, Option<&str>)> {
    let close = text[start + 1..end].find(';')?;
    if close == 0 || close > 32 {
        return None;
    }
    let name = &text[start + 1..start + 1 + close];
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '#')
    {
        return None;
    }
    let decoded = match name {
        "amp" => Some("&"),
        "lt" => Some("<"),
        "gt" => Some(">"),
        "quot" => Some("\""),
        "apos" => Some("'"),
        "nbsp" => Some("\u{a0}"),
        _ => None,
    };
    Some((start + 1 + close + 1, decoded))
}

fn push_run(
    runs: &mut Vec<Inline>,
    text: &str,
    strong: bool,
    emphasis: bool,
    source: charta::SourceSpan,
) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = runs.last_mut() {
        if last.strong == strong
            && last.emphasis == emphasis
            && last.source.file == source.file
            && last.source.byte_end == source.byte_start
        {
            last.text.push_str(text);
            last.source.byte_end = source.byte_end;
            last.source.end_line = source.end_line;
            last.source.end_column = source.end_column;
            return;
        }
    }
    runs.push(Inline {
        text: text.to_owned(),
        strong,
        emphasis,
        intents: Vec::new(),
        source,
    });
}

pub(crate) struct StandaloneImage {
    pub alt: String,
    pub path: String,
    pub title: Option<String>,
}

pub(crate) fn parse_standalone_image(text: &str) -> Option<StandaloneImage> {
    let trimmed = text.trim();
    if !trimmed.starts_with("![") || !trimmed.ends_with(')') {
        return None;
    }
    let close = trimmed.find("](")?;
    let alt = &trimmed[2..close];
    let inner = trimmed[close + 2..trimmed.len() - 1].trim();
    if inner.is_empty() {
        return None;
    }

    let (path, title) = if let Some(rest) = inner.strip_suffix('"') {
        if let Some(open) = rest.rfind('"') {
            let path = inner[..open].trim();
            let title = inner[open + 1..inner.len() - 1].to_owned();
            (path, Some(title))
        } else {
            (inner, None)
        }
    } else if let Some(rest) = inner.strip_suffix('\'') {
        if let Some(open) = rest.rfind('\'') {
            let path = inner[..open].trim();
            let title = inner[open + 1..inner.len() - 1].to_owned();
            (path, Some(title))
        } else {
            (inner, None)
        }
    } else {
        (inner, None)
    };

    let path = path.trim();
    if path.is_empty() || path.contains(char::is_whitespace) {
        return None;
    }
    let path = path
        .strip_prefix('<')
        .and_then(|value| value.strip_suffix('>'))
        .unwrap_or(path);
    Some(StandaloneImage {
        alt: alt.to_owned(),
        path: path.to_owned(),
        title,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line::split_lines;

    fn parse_one(source: &str) -> (Vec<Inline>, Vec<Issue>) {
        let lines = split_lines(source);
        let mut issues = Vec::new();
        let runs = parse("test.md", &lines[0], 0, lines[0].text.len(), &mut issues);
        (runs, issues)
    }

    #[test]
    fn parses_strong_and_emphasis() {
        let (runs, issues) = parse_one("普通 **加粗** *斜体* 结尾");
        assert!(issues.is_empty());
        assert!(runs.iter().any(|run| run.strong && run.text == "加粗"));
        assert!(runs.iter().any(|run| run.emphasis && run.text == "斜体"));
        assert_eq!(
            runs.iter().map(|run| run.text.as_str()).collect::<String>(),
            "普通 加粗 斜体 结尾"
        );
    }

    #[test]
    fn reports_and_degrades_links_and_code() {
        let (runs, issues) = parse_one("查看 [链接](https://example.com) 与 `code`");
        assert_eq!(issues.len(), 2);
        assert!(issues[0].message.contains("链接"));
        assert!(issues[1].message.contains("行内代码"));
        let text: String = runs.iter().map(|run| run.text.as_str()).collect();
        assert!(text.contains("链接"));
        assert!(text.contains("code"));
        assert!(!text.contains("example.com"));
    }

    #[test]
    fn reports_footnote_reference_and_keeps_marker() {
        let (runs, issues) = parse_one("正文[^1]结尾");
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("脚注"));
        let text: String = runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(text, "正文[^1]结尾");
    }

    #[test]
    fn reports_reference_link_and_drops_reference_tag() {
        let (runs, issues) = parse_one("查看 [说明][ref] 内容");
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("引用式链接"));
        let text: String = runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(text, "查看 说明 内容");
    }

    #[test]
    fn reports_unclosed_emphasis() {
        let (_, issues) = parse_one("**未闭合");
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.contains("未闭合"));
    }

    #[test]
    fn parses_standalone_image() {
        let image = parse_standalone_image("![示意图](images/a.png \"标题\")").unwrap();
        assert_eq!(image.alt, "示意图");
        assert_eq!(image.path, "images/a.png");
        assert_eq!(image.title.as_deref(), Some("标题"));
    }
}

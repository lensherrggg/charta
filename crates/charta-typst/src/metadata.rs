//! Typst-native document metadata.
//!
//! Metadata is read from one top-level literal binding:
//!
//! ```text
//! #let charta-meta = (
//!   title: "…",
//!   date: "2026-10-25",
//! )
//! ```
//!
//! A `#let` binding produces no output, so the same file stays compilable by
//! Typst and the metadata never leaks into the rendered document.

use crate::source::SourceText;
use crate::syntax::{
    find_matching, find_top_level_colon, parse_string_literal, read_identifier, split_top_level,
    trim_range,
};
use charta::{Issue, MetaValue, Metadata};
use std::collections::BTreeMap;

pub(crate) struct ParsedMetadata {
    pub metadata: Metadata,
    pub next_line: usize,
}

impl ParsedMetadata {
    fn none() -> Self {
        Self {
            metadata: Metadata::new(),
            next_line: 0,
        }
    }
}

pub(crate) fn parse(source: &SourceText, issues: &mut Vec<Issue>) -> ParsedMetadata {
    let Some(line) = (0..source.line_count()).find(|line| !source.is_blank(*line)) else {
        return ParsedMetadata::none();
    };
    if let Some(close) = legacy_front_matter_close(source, line) {
        issues.push(Issue::unsupported(
            "Typst 不再解析 YAML front matter；请改用 `#let charta-meta = (...)`",
            source.snippet(source.line_start(line), source.line_end(close), 80),
            "该区块已跳过；元数据请写成 Typst 原生 `#let charta-meta` 绑定",
            Some(source.span(source.line_start(line), source.line_end(close))),
        ));
        return ParsedMetadata {
            metadata: Metadata::new(),
            next_line: close + 1,
        };
    }
    parse_let_binding(source, line, issues)
}

fn parse_let_binding(source: &SourceText, line: usize, issues: &mut Vec<Issue>) -> ParsedMetadata {
    let text = &source.text;
    let start = source.after_indent(line);
    if !text[start..].starts_with('#') {
        return ParsedMetadata::none();
    }
    let (keyword, after_keyword) = read_identifier(text, start + 1, text.len());
    if keyword != "let" {
        return ParsedMetadata::none();
    }
    let name_start = skip_ws(text, after_keyword, text.len());
    let (name, after_name) = read_identifier(text, name_start, text.len());
    if name != "charta-meta" {
        return ParsedMetadata::none();
    }
    let expected = "整行 `#let charta-meta = (...)`，值为受限字典字面量";
    let eq = skip_ws(text, after_name, text.len());
    if text.as_bytes().get(eq) != Some(&b'=') {
        return fail(
            source,
            issues,
            line,
            eq,
            eq + 1,
            expected,
            "`charta-meta` 后需要 `=`",
        );
    }
    let open = skip_ws(text, eq + 1, text.len());
    if text.as_bytes().get(open) != Some(&b'(') {
        return fail(
            source,
            issues,
            line,
            open,
            open + 1,
            expected,
            "`charta-meta` 的值必须是括号包裹的字典字面量",
        );
    }
    let Some(close) = find_matching(text, open, text.len()) else {
        return fail(
            source,
            issues,
            line,
            open,
            open + 1,
            expected,
            "`charta-meta` 的字典未闭合",
        );
    };
    let close_line = source.line_index(close);
    let line_end = source.line_end(close_line);
    let mut trailing_start = skip_ws(text, close + 1, line_end);
    if text.as_bytes().get(trailing_start) == Some(&b';') {
        trailing_start = skip_ws(text, trailing_start + 1, line_end);
    }
    if trailing_start < line_end {
        issues.push(Issue::schema(
            "`charta-meta` 块后存在多余内容",
            "charta-meta",
            expected,
            source.snippet(trailing_start, line_end, 40),
            Some(source.span(trailing_start, line_end)),
        ));
    }
    match parse_dictionary(text, open, close) {
        Ok(fields) => ParsedMetadata {
            metadata: Metadata { fields },
            next_line: close_line + 1,
        },
        Err(error) => {
            issues.push(Issue::schema(
                format!("`charta-meta` 解析失败：{}", error.message),
                "charta-meta",
                "字符串、数字、布尔、none、列表与嵌套字典",
                error.message,
                Some(source.span(error.start, error.end.max(error.start + 1))),
            ));
            ParsedMetadata {
                metadata: Metadata::new(),
                next_line: close_line + 1,
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn fail(
    source: &SourceText,
    issues: &mut Vec<Issue>,
    line: usize,
    start: usize,
    end: usize,
    expected: &str,
    message: impl Into<String>,
) -> ParsedMetadata {
    let message = message.into();
    issues.push(Issue::schema(
        format!("`charta-meta` 解析失败：{message}"),
        "charta-meta",
        expected,
        message,
        Some(source.span(start, end.max(start + 1))),
    ));
    ParsedMetadata {
        metadata: Metadata::new(),
        next_line: line + 1,
    }
}

fn legacy_front_matter_close(source: &SourceText, line: usize) -> Option<usize> {
    if source.line_text(line).trim_end() != "---" {
        return None;
    }
    let mut saw_field = false;
    for current in line + 1..source.line_count() {
        let text = source.line_text(current).trim_end();
        if text == "---" || text == "..." {
            return saw_field.then_some(current);
        }
        if looks_like_yaml_field(text) {
            saw_field = true;
        }
    }
    None
}

fn looks_like_yaml_field(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(colon) = trimmed.find(':') else {
        return false;
    };
    let key = &trimmed[..colon];
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-')
}

fn parse_dictionary(
    text: &str,
    open: usize,
    close: usize,
) -> Result<BTreeMap<String, MetaValue>, LiteralError> {
    let inner_start = skip_ws(text, open + 1, close);
    let inner_end = trim_end_index(text, open + 1, close);
    if inner_start >= inner_end {
        return Err(error("字典不能为空；空字典写作 `(:)`", open, close + 1));
    }
    if &text[inner_start..inner_end] == ":" {
        return Ok(BTreeMap::new());
    }
    if find_top_level_colon(text, inner_start, close).is_none() {
        return Err(error("需要 `字段: 值` 形式的字典", inner_start, inner_end));
    }
    let mut fields = BTreeMap::new();
    let mut cursor = inner_start;
    while cursor < inner_end {
        let (key, after_key) = parse_key(text, cursor, inner_end)?;
        cursor = skip_ws(text, after_key, inner_end);
        if cursor >= inner_end || text.as_bytes()[cursor] != b':' {
            return Err(error(
                format!("字段 `{key}` 后需要 `:`"),
                cursor,
                cursor + 1,
            ));
        }
        cursor = skip_ws(text, cursor + 1, inner_end);
        if cursor >= inner_end {
            return Err(error(
                format!("字段 `{key}` 缺少值"),
                cursor.saturating_sub(1),
                inner_end,
            ));
        }
        let (value, next) = parse_value(text, cursor, inner_end)?;
        if fields.insert(key.clone(), value).is_some() {
            return Err(error(format!("字段 `{key}` 重复"), cursor, next));
        }
        cursor = skip_ws(text, next, inner_end);
        if cursor >= inner_end {
            break;
        }
        if text.as_bytes()[cursor] != b',' {
            return Err(error("字段之间需要逗号", cursor, cursor + 1));
        }
        cursor = skip_ws(text, cursor + 1, inner_end);
    }
    Ok(fields)
}

fn parse_key(text: &str, start: usize, end: usize) -> Result<(String, usize), LiteralError> {
    if text.as_bytes()[start] == b'"' {
        let parsed = parse_string_literal(text, start, end)
            .ok_or_else(|| error("字段名字符串未闭合", start, end))?;
        if parsed.value.is_empty() {
            return Err(error("字段名不能为空", start, parsed.next));
        }
        return Ok((parsed.value, parsed.next));
    }
    let (name, next) = read_identifier(text, start, end);
    let valid = name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_alphabetic() || ch == '_');
    if !valid {
        return Err(error(
            "字段名需要标识符或字符串",
            start,
            (start + 1).min(end),
        ));
    }
    Ok((name, next))
}

fn parse_value(text: &str, start: usize, end: usize) -> Result<(MetaValue, usize), LiteralError> {
    let start = skip_ws(text, start, end);
    if start >= end {
        return Err(error("缺少值", start, end));
    }
    match text.as_bytes()[start] {
        b'"' => {
            let parsed = parse_string_literal(text, start, end)
                .ok_or_else(|| error("字符串未闭合", start, end))?;
            Ok((MetaValue::String(parsed.value), parsed.next))
        }
        b'(' => parse_parenthesized(text, start, end),
        b'-' | b'0'..=b'9' => parse_number(text, start, end),
        _ => {
            let (name, next) = read_identifier(text, start, end);
            match name.as_str() {
                "true" => Ok((MetaValue::Bool(true), next)),
                "false" => Ok((MetaValue::Bool(false), next)),
                "none" => Ok((MetaValue::Null, next)),
                "" => Err(error(
                    "无法识别的值；只接受字面量",
                    start,
                    (start + 1).min(end),
                )),
                _ => Err(error(
                    format!("不支持的值 `{name}`；只接受字面量"),
                    start,
                    next,
                )),
            }
        }
    }
}

fn parse_parenthesized(
    text: &str,
    start: usize,
    end: usize,
) -> Result<(MetaValue, usize), LiteralError> {
    let close = find_matching(text, start, end).ok_or_else(|| error("括号未闭合", start, end))?;
    let inner_start = skip_ws(text, start + 1, close);
    let inner_end = trim_end_index(text, start + 1, close);
    if inner_start >= inner_end {
        return Ok((MetaValue::List(Vec::new()), close + 1));
    }
    if &text[inner_start..inner_end] == ":" {
        return Ok((MetaValue::Map(BTreeMap::new()), close + 1));
    }
    if find_top_level_colon(text, inner_start, close).is_some() {
        let fields = parse_dictionary(text, start, close)?;
        return Ok((MetaValue::Map(fields), close + 1));
    }
    let parts = split_top_level(text, inner_start, close, ',');
    if parts.len() == 1 {
        let (part_start, part_end) = trim_range(text, parts[0].0, parts[0].1);
        let (value, next) = parse_value(text, part_start, part_end)?;
        let rest = skip_ws(text, next, part_end);
        if rest < part_end {
            return Err(error("值后存在多余内容", rest, part_end));
        }
        return Ok((value, close + 1));
    }
    let mut items = Vec::new();
    let last = parts.len().saturating_sub(1);
    for (index, (part_start, part_end)) in parts.into_iter().enumerate() {
        let (part_start, part_end) = trim_range(text, part_start, part_end);
        if part_start >= part_end {
            if index == last {
                continue;
            }
            return Err(error("列表项不能为空", part_start, part_end));
        }
        let (value, next) = parse_value(text, part_start, part_end)?;
        let rest = skip_ws(text, next, part_end);
        if rest < part_end {
            return Err(error("值后存在多余内容", rest, part_end));
        }
        items.push(value);
    }
    Ok((MetaValue::List(items), close + 1))
}

fn parse_number(text: &str, start: usize, end: usize) -> Result<(MetaValue, usize), LiteralError> {
    let bytes = text.as_bytes();
    let mut index = start;
    if bytes[index] == b'-' {
        index += 1;
    }
    let integer_start = index;
    while index < end && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if index == integer_start {
        return Err(error("数字格式不正确", start, (start + 1).min(end)));
    }
    let mut is_float = false;
    if index < end && bytes[index] == b'.' {
        is_float = true;
        index += 1;
        let fraction_start = index;
        while index < end && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == fraction_start {
            return Err(error("小数位数缺失", start, index));
        }
    }
    if index < end {
        let ch = text[index..].chars().next().unwrap();
        if ch.is_alphanumeric() || ch == '%' {
            return Err(error("数值不能带单位", index, index + ch.len_utf8()));
        }
    }
    let raw = &text[start..index];
    if is_float {
        let number = raw
            .parse::<f64>()
            .map_err(|_| error("数字格式不正确", start, index))?;
        Ok((MetaValue::Number(number), index))
    } else {
        let number = raw
            .parse::<i64>()
            .map_err(|_| error("整数超出范围", start, index))?;
        Ok((MetaValue::Integer(number), index))
    }
}

fn trim_end_index(text: &str, start: usize, end: usize) -> usize {
    start + text[start..end].trim_end().len()
}

fn skip_ws(text: &str, mut index: usize, limit: usize) -> usize {
    while index < limit {
        match text[index..].chars().next().unwrap() {
            ' ' | '\t' | '\r' | '\n' => index += 1,
            _ => break,
        }
    }
    index
}

struct LiteralError {
    message: String,
    start: usize,
    end: usize,
}

fn error(message: impl Into<String>, start: usize, end: usize) -> LiteralError {
    LiteralError {
        message: message.into(),
        start,
        end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_source(source: &str) -> (ParsedMetadata, Vec<Issue>) {
        let source = SourceText::new("a.typ", source);
        let mut issues = Vec::new();
        let parsed = parse(&source, &mut issues);
        (parsed, issues)
    }

    #[test]
    fn parses_nested_literals() {
        let (parsed, issues) = parse_source(
            "#let charta-meta = (\n  title: \"测试\",\n  date: \"2026-10-25\",\n  count: 3,\n  ratio: 1.5,\n  urgent: true,\n  note: none,\n  cc: (\"甲\", \"乙\"),\n  _charta: (heading_numbering: true),\n)\n\n= 正文\n",
        );
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(parsed.next_line, 10);
        let metadata = parsed.metadata;
        assert_eq!(metadata.get("title").unwrap().as_str(), Some("测试"));
        assert_eq!(metadata.get("date").unwrap().as_str(), Some("2026-10-25"));
        assert_eq!(metadata.get("count"), Some(&MetaValue::Integer(3)));
        assert_eq!(metadata.get("ratio"), Some(&MetaValue::Number(1.5)));
        assert_eq!(metadata.get("urgent"), Some(&MetaValue::Bool(true)));
        assert_eq!(metadata.get("note"), Some(&MetaValue::Null));
        assert_eq!(
            metadata.get("cc").unwrap().as_list().unwrap()[1].as_str(),
            Some("乙")
        );
        assert_eq!(
            metadata
                .get("_charta")
                .unwrap()
                .as_map()
                .unwrap()
                .get("heading_numbering"),
            Some(&MetaValue::Bool(true))
        );
    }

    #[test]
    fn follows_typst_array_and_grouping_semantics() {
        let (parsed, issues) =
            parse_source("#let charta-meta = (grouped: (\"单值\"), single: (1,), pair: (1, 2))\n");
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(
            parsed.metadata.get("grouped").unwrap().as_str(),
            Some("单值")
        );
        assert_eq!(
            parsed
                .metadata
                .get("single")
                .unwrap()
                .as_list()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            parsed
                .metadata
                .get("pair")
                .unwrap()
                .as_list()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn empty_dictionary_uses_typst_syntax() {
        let (parsed, issues) = parse_source("#let charta-meta = (:)\n");
        assert!(issues.is_empty(), "{issues:?}");
        assert!(parsed.metadata.is_empty());
        assert_eq!(parsed.next_line, 1);
    }

    #[test]
    fn accepts_trailing_semicolon() {
        let (parsed, issues) = parse_source("#let charta-meta = (title: \"测试\");\n\n= 正文\n");
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(parsed.metadata.get("title").unwrap().as_str(), Some("测试"));
        assert_eq!(parsed.next_line, 1);
    }

    #[test]
    fn allows_leading_comments_and_blank_lines() {
        let (parsed, issues) = parse_source("// 元数据\n\n#let charta-meta = (title: \"测试\")\n");
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(parsed.metadata.get("title").unwrap().as_str(), Some("测试"));
        assert_eq!(parsed.next_line, 3);
    }

    #[test]
    fn keeps_metadata_only_at_the_first_content_line() {
        let (parsed, issues) = parse_source("= 标题\n\n#let charta-meta = (title: \"x\")\n");
        assert!(issues.is_empty(), "{issues:?}");
        assert!(parsed.metadata.is_empty());
        assert_eq!(parsed.next_line, 0);
    }

    #[test]
    fn rejects_dynamic_and_malformed_values() {
        let (_, issues) = parse_source("#let charta-meta = (title: name)\n");
        assert!(issues
            .iter()
            .any(|issue| issue.is_blocking() && issue.message.contains("不支持的值 `name`")));
        let (_, issues) = parse_source("#let charta-meta = (title: 12pt)\n");
        assert!(issues
            .iter()
            .any(|issue| issue.is_blocking() && issue.message.contains("数值不能带单位")));
        let (_, issues) = parse_source("#let charta-meta = (title: \"x\", title: \"y\")\n");
        assert!(issues
            .iter()
            .any(|issue| issue.is_blocking() && issue.message.contains("字段 `title` 重复")));
    }

    #[test]
    fn reports_legacy_front_matter_and_skips_it() {
        let (parsed, issues) = parse_source("---\ntitle: 测试\ndate: 2026-10-25\n---\n\n= 正文\n");
        assert_eq!(parsed.next_line, 4);
        assert!(parsed.metadata.is_empty());
        let issue = issues
            .iter()
            .find(|issue| issue.message.contains("YAML front matter"))
            .expect("legacy front matter issue");
        assert!(issue.message.contains("charta-meta"));
    }

    #[test]
    fn leaves_plain_em_dash_content_alone() {
        let (parsed, issues) = parse_source("---\n\n= 正文\n");
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(parsed.next_line, 0);
    }
}

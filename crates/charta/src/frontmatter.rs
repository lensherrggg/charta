//! A deliberately small YAML subset for front matter.
//!
//! Supported: block mappings, block sequences, flow sequences, quoted and
//! plain scalars, comments, booleans, null, integers and floats. Unsupported
//! YAML features are reported as errors instead of being guessed.

use crate::{MetaValue, Metadata};
use std::collections::BTreeMap;

/// One physical line of a YAML front matter block.
#[derive(Debug, Clone)]
pub struct YamlLine<'a> {
    pub number: usize,
    pub start: usize,
    pub text: &'a str,
}

#[derive(Debug, Clone)]
pub struct YamlError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub byte_start: usize,
    pub byte_end: usize,
}

struct Line<'a> {
    number: usize,
    start: usize,
    indent: usize,
    text: &'a str,
}

/// Parse a YAML-subset front matter block into metadata.
pub fn parse(lines: &[YamlLine<'_>]) -> Result<Metadata, YamlError> {
    let mut prepared = Vec::new();
    for line in lines {
        if line.text.trim().is_empty() || line.text.trim_start().starts_with('#') {
            continue;
        }
        let indent = leading_spaces(line)?;
        prepared.push(Line {
            number: line.number,
            start: line.start,
            indent,
            text: line.text.trim_end(),
        });
    }

    if prepared.is_empty() {
        return Ok(Metadata::new());
    }
    if prepared[0].indent != 0 {
        return Err(error_at(&prepared[0], 0, "顶层字段不能缩进"));
    }
    if prepared[0].text.trim_start().starts_with("- ") {
        return Err(error_at(&prepared[0], 0, "front matter 顶层必须是映射"));
    }

    let mut index = 0;
    let value = parse_map(&prepared, &mut index, 0)?;
    if index < prepared.len() {
        return Err(error_at(&prepared[index], 0, "缩进或结构与上层字段不一致"));
    }
    match value {
        MetaValue::Map(fields) => Ok(Metadata { fields }),
        _ => Err(error_at(&prepared[0], 0, "front matter 顶层必须是映射")),
    }
}

fn parse_block(
    lines: &[Line<'_>],
    index: &mut usize,
    indent: usize,
) -> Result<MetaValue, YamlError> {
    let line = &lines[*index];
    if line.text.trim_start().starts_with("- ") {
        parse_list(lines, index, indent)
    } else {
        parse_map(lines, index, indent)
    }
}

fn parse_map(lines: &[Line<'_>], index: &mut usize, indent: usize) -> Result<MetaValue, YamlError> {
    let mut map = BTreeMap::new();
    while *index < lines.len() {
        let line = &lines[*index];
        if line.indent < indent {
            break;
        }
        if line.indent > indent {
            return Err(error_at(line, 0, "意外缩进"));
        }
        if line.text.trim_start().starts_with("- ") {
            return Err(error_at(line, 0, "此处不能是列表项"));
        }
        let (key, value_text) = split_key_value(line.text)
            .ok_or_else(|| error_at(line, 0, "缺少 `key: value` 结构"))?;
        let key = key.trim().to_owned();
        if key.is_empty() {
            return Err(error_at(line, 0, "字段名不能为空"));
        }
        if map.contains_key(&key) {
            return Err(error_at(line, 0, format!("字段 `{key}` 重复")));
        }
        let value_offset = line.text.len() - value_text.len();
        let value = if value_text.trim().is_empty() {
            *index += 1;
            if *index < lines.len() && lines[*index].indent > indent {
                let child_indent = lines[*index].indent;
                parse_block(lines, index, child_indent)?
            } else {
                MetaValue::Null
            }
        } else {
            let value = parse_inline_value(value_text, line, value_offset)?;
            *index += 1;
            value
        };
        map.insert(key, value);
    }
    Ok(MetaValue::Map(map))
}

fn parse_list(
    lines: &[Line<'_>],
    index: &mut usize,
    indent: usize,
) -> Result<MetaValue, YamlError> {
    let mut items = Vec::new();
    while *index < lines.len() {
        let line = &lines[*index];
        if line.indent < indent {
            break;
        }
        if line.indent > indent {
            return Err(error_at(line, 0, "意外缩进"));
        }
        let trimmed = line.text.trim_start();
        let Some(rest) = trimmed.strip_prefix("- ") else {
            break;
        };
        let dash_offset = line.indent + 2;
        let rest = rest.trim_end();
        if rest.is_empty() {
            *index += 1;
            if *index < lines.len() && lines[*index].indent > indent {
                let child_indent = lines[*index].indent;
                items.push(parse_block(lines, index, child_indent)?);
            } else {
                items.push(MetaValue::Null);
            }
            continue;
        }

        if let Some((key, value_text)) = split_key_value(rest) {
            let key = key.trim().to_owned();
            if key.is_empty() {
                return Err(error_at(line, dash_offset, "字段名不能为空"));
            }
            let mut item = BTreeMap::new();
            *index += 1;
            let item_indent = if *index < lines.len() && lines[*index].indent > indent {
                lines[*index].indent
            } else {
                indent + 2
            };

            let value = if value_text.trim().is_empty() {
                if *index < lines.len() && lines[*index].indent > item_indent {
                    let child_indent = lines[*index].indent;
                    parse_block(lines, index, child_indent)?
                } else {
                    MetaValue::Null
                }
            } else {
                let value_offset = rest.len() - value_text.len();
                parse_inline_value(value_text, line, dash_offset + value_offset)?
            };
            item.insert(key, value);

            while *index < lines.len() {
                let extra = &lines[*index];
                if extra.indent <= indent {
                    break;
                }
                if extra.indent != item_indent {
                    return Err(error_at(extra, 0, "列表项字段的缩进不一致"));
                }
                if extra.text.trim_start().starts_with("- ") {
                    return Err(error_at(extra, 0, "暂不支持在映射项内继续嵌套列表"));
                }
                let (key, value_text) = split_key_value(extra.text)
                    .ok_or_else(|| error_at(extra, 0, "缺少 `key: value` 结构"))?;
                let key = key.trim().to_owned();
                if item.contains_key(&key) {
                    return Err(error_at(extra, 0, format!("字段 `{key}` 重复")));
                }
                *index += 1;
                let value_offset = extra.text.len() - value_text.len();
                let value = if value_text.trim().is_empty() {
                    if *index < lines.len() && lines[*index].indent > item_indent {
                        let child_indent = lines[*index].indent;
                        parse_block(lines, index, child_indent)?
                    } else {
                        MetaValue::Null
                    }
                } else {
                    parse_inline_value(value_text, extra, value_offset)?
                };
                item.insert(key, value);
            }
            items.push(MetaValue::Map(item));
        } else {
            items.push(parse_inline_value(rest, line, dash_offset)?);
            *index += 1;
        }
    }
    Ok(MetaValue::List(items))
}

fn parse_inline_value(text: &str, line: &Line<'_>, offset: usize) -> Result<MetaValue, YamlError> {
    let text = strip_comment(text);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(MetaValue::Null);
    }
    let leading = text.len() - text.trim_start().len();
    let offset = offset + leading;

    if trimmed.starts_with('[') {
        return parse_flow_list(trimmed, line, offset);
    }
    if trimmed.starts_with('{') {
        return Err(error_at(
            line,
            offset,
            "暂不支持 flow mapping（`{...}`），请改用缩进映射",
        ));
    }
    if trimmed.starts_with('|') || trimmed.starts_with('>') {
        return Err(error_at(
            line,
            offset,
            "暂不支持 YAML 多行标量（`|` / `>`）",
        ));
    }
    if trimmed.starts_with('&') || trimmed.starts_with('*') || trimmed.starts_with('!') {
        return Err(error_at(line, offset, "暂不支持 YAML 锚点、别名或标签"));
    }
    if trimmed.starts_with('"') || trimmed.starts_with('\'') {
        let (decoded, consumed) = parse_quoted(trimmed, line, offset)?;
        if !trimmed[consumed..].trim().is_empty() {
            return Err(error_at(
                line,
                offset + consumed,
                "引号字符串之后存在多余内容",
            ));
        }
        return Ok(MetaValue::String(decoded));
    }
    Ok(infer_scalar(trimmed))
}

fn parse_flow_list(text: &str, line: &Line<'_>, offset: usize) -> Result<MetaValue, YamlError> {
    if !text.ends_with(']') {
        return Err(error_at(line, offset, "flow 列表缺少结尾 `]`"));
    }
    let inner = &text[1..text.len() - 1];
    if inner.trim().is_empty() {
        return Ok(MetaValue::List(Vec::new()));
    }
    let parts = split_flow_items(inner, line, offset + 1)?;
    let mut values = Vec::new();
    for (start, end) in parts {
        let raw = &inner[start..end];
        let leading = raw.len() - raw.trim_start().len();
        let item_offset = offset + 1 + start + leading;
        let part = raw.trim();
        if part.is_empty() {
            return Err(error_at(line, item_offset, "flow 列表包含空项"));
        }
        values.push(parse_inline_value(part, line, item_offset)?);
    }
    Ok(MetaValue::List(values))
}

fn split_flow_items(
    inner: &str,
    line: &Line<'_>,
    offset: usize,
) -> Result<Vec<(usize, usize)>, YamlError> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (index, ch) in inner.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if let Some(active) = quote {
            if active == '"' && ch == '\\' {
                escaped = true;
            } else if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '[' | '{' => depth += 1,
            ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push((start, index));
                start = index + 1;
            }
            _ => {}
        }
    }
    if quote.is_some() {
        return Err(error_at(line, offset, "flow 列表中的引号未闭合"));
    }
    if depth != 0 {
        return Err(error_at(line, offset, "flow 列表的括号不匹配"));
    }
    parts.push((start, inner.len()));
    Ok(parts)
}

fn parse_quoted(text: &str, line: &Line<'_>, offset: usize) -> Result<(String, usize), YamlError> {
    let quote = text.as_bytes()[0] as char;
    let mut out = String::new();
    let mut chars = text.char_indices().skip(1).peekable();
    while let Some((index, ch)) = chars.next() {
        if quote == '\'' {
            if ch == '\'' {
                if matches!(chars.peek(), Some((_, '\''))) {
                    chars.next();
                    out.push('\'');
                    continue;
                }
                return Ok((out, index + ch.len_utf8()));
            }
            out.push(ch);
        } else if ch == '\\' {
            let Some((_, escaped)) = chars.next() else {
                return Err(error_at(line, offset + index, "转义符后缺少字符"));
            };
            out.push(match escaped {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '\\' => '\\',
                '"' => '"',
                other => other,
            });
        } else if ch == '"' {
            return Ok((out, index + ch.len_utf8()));
        } else {
            out.push(ch);
        }
    }
    Err(error_at(line, offset, "引号字符串未闭合"))
}

fn infer_scalar(text: &str) -> MetaValue {
    match text {
        "null" | "Null" | "NULL" | "~" => MetaValue::Null,
        "true" | "True" | "TRUE" => MetaValue::Bool(true),
        "false" | "False" | "FALSE" => MetaValue::Bool(false),
        _ => {
            if let Ok(value) = text.parse::<i64>() {
                MetaValue::Integer(value)
            } else if (text.contains('.') || text.contains('e') || text.contains('E'))
                && text.parse::<f64>().is_ok()
            {
                MetaValue::Number(text.parse().unwrap_or_default())
            } else {
                MetaValue::String(text.to_owned())
            }
        }
    }
}

fn split_key_value(text: &str) -> Option<(&str, &str)> {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if let Some(active) = quote {
            if active == '"' && ch == '\\' {
                escaped = true;
            } else if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            ':' => {
                let next = text[index + 1..].chars().next();
                if next.is_none() || next.is_some_and(char::is_whitespace) {
                    return Some((&text[..index], &text[index + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

fn strip_comment(text: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut previous_whitespace = true;
    for (index, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            previous_whitespace = ch.is_whitespace();
            continue;
        }
        if let Some(active) = quote {
            if active == '"' && ch == '\\' {
                escaped = true;
            } else if ch == active {
                quote = None;
            }
            previous_whitespace = ch.is_whitespace();
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '#' if previous_whitespace => return &text[..index],
            _ => {}
        }
        previous_whitespace = ch.is_whitespace();
    }
    text
}

fn leading_spaces(line: &YamlLine<'_>) -> Result<usize, YamlError> {
    let mut indent = 0;
    for ch in line.text.chars() {
        match ch {
            ' ' => indent += 1,
            '\t' => {
                return Err(YamlError {
                    message: "front matter 缩进不支持 Tab".to_owned(),
                    line: line.number,
                    column: indent + 1,
                    byte_start: line.start + indent,
                    byte_end: line.start + indent + 1,
                })
            }
            _ => break,
        }
    }
    Ok(indent)
}

fn error_at(line: &Line<'_>, local: usize, message: impl Into<String>) -> YamlError {
    let prefix = line.text.get(..local).unwrap_or(line.text);
    YamlError {
        message: message.into(),
        line: line.number,
        column: prefix.chars().count() + 1,
        byte_start: line.start + local,
        byte_end: line.start + local,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_source(source: &str) -> Result<Metadata, YamlError> {
        let mut lines = Vec::new();
        let mut offset = 0;
        for (index, raw) in source.split_inclusive('\n').enumerate() {
            let text = raw.trim_end_matches(['\r', '\n']);
            lines.push(YamlLine {
                number: index + 1,
                start: offset,
                text,
            });
            offset += raw.len();
        }
        parse(&lines)
    }

    #[test]
    fn parses_scalars_lists_and_nested_maps() {
        let metadata = parse_source(
            "title: \"关于测试的通知\"\ndate: 2026-10-25\nurgent: false\ncount: 3\nratio: 1.5\nrecipient:\n  - 甲单位\n  - 乙单位\n_charta:\n  roles:\n    - name: appendix\n      description: 附录\n",
        )
        .unwrap();
        assert_eq!(
            metadata.get("title").unwrap().as_str(),
            Some("关于测试的通知")
        );
        assert_eq!(metadata.get("date").unwrap().as_str(), Some("2026-10-25"));
        assert!(matches!(
            metadata.get("urgent"),
            Some(MetaValue::Bool(false))
        ));
        assert!(matches!(metadata.get("count"), Some(MetaValue::Integer(3))));
        assert!(matches!(metadata.get("ratio"), Some(MetaValue::Number(_))));
        let recipients = metadata.get("recipient").unwrap().as_list().unwrap();
        assert_eq!(recipients.len(), 2);
        let control = metadata.get("_charta").unwrap().as_map().unwrap();
        let roles = control.get("roles").unwrap().as_list().unwrap();
        let role = roles[0].as_map().unwrap();
        assert_eq!(role.get("name").unwrap().as_str(), Some("appendix"));
    }

    #[test]
    fn supports_flow_lists_and_comments() {
        let metadata = parse_source("cc: [甲, \"乙, 丙\"] # 抄送\nnote: 'it''s fine'\n").unwrap();
        assert_eq!(metadata.get("cc").unwrap().as_list().unwrap().len(), 2);
        assert_eq!(metadata.get("note").unwrap().as_str(), Some("it's fine"));
    }

    #[test]
    fn rejects_unsupported_yaml() {
        let error = parse_source("text: |\n  hello\n").unwrap_err();
        assert!(error.message.contains("多行标量"));
        let error = parse_source("map: {a: 1}\n").unwrap_err();
        assert!(error.message.contains("flow mapping"));
        let error = parse_source("a: 1\na: 2\n").unwrap_err();
        assert!(error.message.contains("重复"));
    }
}

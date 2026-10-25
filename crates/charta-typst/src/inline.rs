//! Inline Typst markup for the read-only subset.
//!
//! Supported: text, strong/emphasis, `#text(...)[...]` style intents and
//! literal include at line start (handled by the block parser). Everything
//! else is reported explicitly and degraded.

use crate::source::SourceText;
use crate::syntax::{
    find_content_block, find_matching, find_top_level_colon, parse_string_literal, read_identifier,
    skip_spaces, split_top_level, statement_end, trim_range, unquote,
};
use charta::ir::normalize_hex_color;
use charta::{FrontendId, Inline, Issue, StyleIntent, StyleProperty};

pub(crate) struct InlineOutcome {
    pub runs: Vec<Inline>,
    pub degraded: Option<(String, String)>,
}

impl InlineOutcome {
    fn new() -> Self {
        Self {
            runs: Vec::new(),
            degraded: None,
        }
    }

    fn mark_degraded(&mut self, reason: impl Into<String>, impact: impl Into<String>) {
        if self.degraded.is_none() {
            self.degraded = Some((reason.into(), impact.into()));
        }
    }
}

pub(crate) fn parse(
    source: &SourceText,
    start: usize,
    end: usize,
    inherited: &[StyleIntent],
    issues: &mut Vec<Issue>,
) -> InlineOutcome {
    let mut outcome = InlineOutcome::new();
    parse_segment(
        source,
        start,
        end,
        false,
        false,
        inherited,
        &mut outcome,
        issues,
    );
    outcome
}

#[allow(clippy::too_many_arguments)]
fn parse_segment(
    source: &SourceText,
    start: usize,
    end: usize,
    strong: bool,
    emphasis: bool,
    intents: &[StyleIntent],
    outcome: &mut InlineOutcome,
    issues: &mut Vec<Issue>,
) {
    let text = &source.text;
    let mut index = start;
    while index < end {
        let ch = text[index..].chars().next().unwrap();
        match ch {
            '\\' => {
                if index + 1 < end {
                    let next = text[index + 1..].chars().next().unwrap();
                    if next == '\n' || next == '\r' {
                        push_run(
                            outcome,
                            " ",
                            strong,
                            emphasis,
                            intents,
                            source.span(index, index + 2),
                        );
                    } else {
                        push_run(
                            outcome,
                            &text[index + 1..index + 1 + next.len_utf8()],
                            strong,
                            emphasis,
                            intents,
                            source.span(index, index + 1 + next.len_utf8()),
                        );
                    }
                    index += 1 + next.len_utf8();
                } else {
                    push_run(
                        outcome,
                        "\\",
                        strong,
                        emphasis,
                        intents,
                        source.span(index, index + 1),
                    );
                    index += 1;
                }
            }
            '*' => {
                index = handle_emphasis(
                    source, index, end, "*", true, strong, emphasis, intents, outcome, issues,
                );
            }
            '_' => {
                index = handle_emphasis(
                    source, index, end, "_", false, strong, emphasis, intents, outcome, issues,
                );
            }
            '#' => {
                index = handle_code(
                    source, index, end, strong, emphasis, intents, outcome, issues,
                );
            }
            '$' => {
                index = handle_math(source, index, end, intents, outcome, issues);
            }
            '`' => {
                index = handle_raw(source, index, end, intents, outcome, issues);
            }
            '<' => {
                index = handle_label(source, index, end, issues);
            }
            '@' => {
                index = handle_reference(source, index, end, intents, outcome, issues);
            }
            '\n' | '\r' => {
                push_run(
                    outcome,
                    " ",
                    strong,
                    emphasis,
                    intents,
                    source.span(index, index + 1),
                );
                index += 1;
            }
            _ => {
                let next = next_special(text, index + ch.len_utf8(), end);
                push_run(
                    outcome,
                    &text[index..next],
                    strong,
                    emphasis,
                    intents,
                    source.span(index, next),
                );
                index = next;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_emphasis(
    source: &SourceText,
    index: usize,
    end: usize,
    marker: &str,
    add_strong: bool,
    strong: bool,
    emphasis: bool,
    intents: &[StyleIntent],
    outcome: &mut InlineOutcome,
    issues: &mut Vec<Issue>,
) -> usize {
    let content_start = index + marker.len();
    if let Some(close) = source.text[content_start..end].find(marker) {
        let content_end = content_start + close;
        if content_end > content_start {
            parse_segment(
                source,
                content_start,
                content_end,
                strong || add_strong,
                emphasis || !add_strong,
                intents,
                outcome,
                issues,
            );
            return content_end + marker.len();
        }
    }
    let marker_end = (index + marker.len()).min(end);
    issues.push(Issue::unsupported(
        format!("Typst 强调标记 `{marker}` 未闭合"),
        source.snippet(index, marker_end, 8),
        "按字面文本保留",
        Some(source.span(index, marker_end)),
    ));
    push_run(
        outcome,
        marker,
        strong,
        emphasis,
        intents,
        source.span(index, marker_end),
    );
    marker_end
}

#[allow(clippy::too_many_arguments)]
fn handle_code(
    source: &SourceText,
    index: usize,
    end: usize,
    strong: bool,
    emphasis: bool,
    intents: &[StyleIntent],
    outcome: &mut InlineOutcome,
    issues: &mut Vec<Issue>,
) -> usize {
    let text = &source.text;
    let ident_start = index + 1;
    let first = text[ident_start..end].chars().next();
    if matches!(first, Some('{' | '(' | '[')) {
        return handle_code_group(source, index, end, intents, outcome, issues);
    }
    let (ident, after_ident) = read_identifier(text, ident_start, end);
    if ident.is_empty() {
        push_run(
            outcome,
            "#",
            strong,
            emphasis,
            intents,
            source.span(index, index + 1),
        );
        return index + 1;
    }
    let spaced = skip_spaces(text, after_ident, end);
    match ident.as_str() {
        "text" => handle_text_call(
            source, index, spaced, end, strong, emphasis, intents, outcome, issues,
        ),
        "strong" | "emph" => handle_style_alias(
            source,
            index,
            ident.as_str(),
            spaced,
            end,
            strong,
            emphasis,
            intents,
            outcome,
            issues,
        ),
        "include" | "set" | "show" | "import" | "let" | "for" | "while" | "if" => {
            handle_inline_construct(
                source,
                index,
                ident.as_str(),
                after_ident,
                end,
                intents,
                outcome,
                issues,
            )
        }
        _ => handle_unknown_code(source, index, &ident, spaced, end, intents, outcome, issues),
    }
}

fn handle_code_group(
    source: &SourceText,
    index: usize,
    end: usize,
    intents: &[StyleIntent],
    outcome: &mut InlineOutcome,
    issues: &mut Vec<Issue>,
) -> usize {
    let text = &source.text;
    let open = index + 1;
    let Some(close) = find_matching(text, open, end) else {
        issues.push(Issue::unsupported(
            "Typst 代码表达式未闭合",
            source.snippet(index, end, 40),
            "已丢弃该表达式",
            Some(source.span(index, end)),
        ));
        outcome.mark_degraded("Typst 代码表达式未闭合", "表达式内容未进入 IR");
        return end;
    };
    let is_content = text.as_bytes()[open] == b'[';
    issues.push(Issue::unsupported(
        if is_content {
            "不支持 Typst 内容块表达式 `#[...]`"
        } else {
            "不支持 Typst 代码表达式 `#{...}` / `#(...)`"
        },
        source.snippet(index, close + 1, 40),
        if is_content {
            "保留内容块文字，未执行代码表达式"
        } else {
            "已丢弃该表达式"
        },
        Some(source.span(index, close + 1)),
    ));
    if is_content {
        parse_segment(
            source,
            open + 1,
            close,
            false,
            false,
            intents,
            outcome,
            issues,
        );
    }
    outcome.mark_degraded(
        "存在未求值的 Typst 代码表达式",
        "表达式中的变量、条件或函数结果可能缺失",
    );
    close + 1
}

#[allow(clippy::too_many_arguments)]
fn handle_text_call(
    source: &SourceText,
    code_start: usize,
    spaced: usize,
    end: usize,
    strong: bool,
    emphasis: bool,
    inherited: &[StyleIntent],
    outcome: &mut InlineOutcome,
    issues: &mut Vec<Issue>,
) -> usize {
    let text = &source.text;
    if spaced >= end || text.as_bytes()[spaced] != b'(' {
        issues.push(Issue::unsupported(
            "`#text` 缺少参数列表",
            source.snippet(code_start, spaced.max(code_start + 1), 40),
            "已忽略该调用",
            Some(source.span(code_start, spaced.max(code_start + 1))),
        ));
        outcome.mark_degraded("`#text` 调用无法解析", "该调用对应内容未进入 IR");
        return spaced.max(code_start + 1);
    }
    let Some(close) = find_matching(text, spaced, end) else {
        issues.push(Issue::unsupported(
            "`#text` 参数列表未闭合",
            source.snippet(code_start, end, 40),
            "已忽略该调用",
            Some(source.span(code_start, end)),
        ));
        outcome.mark_degraded("`#text` 调用无法解析", "该调用对应内容未进入 IR");
        return end;
    };
    let mut own = parse_text_args(source, spaced + 1, close, issues);

    let after = skip_spaces(text, close + 1, end);
    if after >= end || text.as_bytes()[after] != b'[' {
        issues.push(Issue::unsupported(
            "`#text` 调用缺少内容块",
            source.snippet(code_start, close + 1, 40),
            "已忽略该调用",
            Some(source.span(code_start, close + 1)),
        ));
        outcome.mark_degraded("`#text` 调用缺少内容", "该调用对应内容未进入 IR");
        return close + 1;
    }
    let Some(content_close) = find_matching(text, after, end) else {
        issues.push(Issue::unsupported(
            "`#text` 内容块未闭合",
            source.snippet(code_start, end, 40),
            "按字面文本保留到行尾",
            Some(source.span(code_start, end)),
        ));
        outcome.mark_degraded("`#text` 内容块无法闭合", "内容按字面文本保留");
        return end;
    };

    let mut all = inherited.to_vec();
    all.append(&mut own);
    parse_segment(
        source,
        after + 1,
        content_close,
        strong,
        emphasis,
        &all,
        outcome,
        issues,
    );
    content_close + 1
}

#[allow(clippy::too_many_arguments)]
fn handle_style_alias(
    source: &SourceText,
    code_start: usize,
    ident: &str,
    spaced: usize,
    end: usize,
    strong: bool,
    emphasis: bool,
    intents: &[StyleIntent],
    outcome: &mut InlineOutcome,
    issues: &mut Vec<Issue>,
) -> usize {
    let text = &source.text;
    if spaced >= end || text.as_bytes()[spaced] != b'[' {
        let stop = spaced.max(code_start + 1);
        issues.push(Issue::unsupported(
            format!("`#{ident}` 缺少内容块"),
            source.snippet(code_start, stop, 40),
            "已忽略该调用",
            Some(source.span(code_start, stop)),
        ));
        outcome.mark_degraded(
            format!("`#{ident}` 调用无法解析"),
            "该调用对应内容未进入 IR",
        );
        return stop;
    }
    let Some(close) = find_matching(text, spaced, end) else {
        issues.push(Issue::unsupported(
            format!("`#{ident}` 内容块未闭合"),
            source.snippet(code_start, end, 40),
            "按字面文本保留到行尾",
            Some(source.span(code_start, end)),
        ));
        outcome.mark_degraded(format!("`#{ident}` 内容块无法闭合"), "内容按字面文本保留");
        return end;
    };
    let add_strong = ident == "strong";
    parse_segment(
        source,
        spaced + 1,
        close,
        strong || add_strong,
        emphasis || !add_strong,
        intents,
        outcome,
        issues,
    );
    close + 1
}

#[allow(clippy::too_many_arguments)]
fn handle_inline_construct(
    source: &SourceText,
    start: usize,
    ident: &str,
    after_ident: usize,
    end: usize,
    intents: &[StyleIntent],
    outcome: &mut InlineOutcome,
    issues: &mut Vec<Issue>,
) -> usize {
    let (reason, impact) = match ident {
        "include" => (
            "行内 include 未展开",
            "include 仅支持行首整行静态展开，该引用内容未进入 IR",
        ),
        "set" => (
            "set 规则需要求值",
            "该设置未应用，相关内容按目标后端的默认样式渲染",
        ),
        "show" => (
            "show 规则需要求值",
            "该规则未应用，相关内容的样式可能与源文件不同",
        ),
        "import" => (
            "import 符号产出的内容不可静态确定",
            "依赖该符号的内容未展开",
        ),
        "let" => ("let 绑定需要求值", "绑定值未展开，后续变量引用可能缺失"),
        "for" | "while" => (
            "循环生成的内容需要求值",
            "循环体按字面量提取一次，重复次数与变量替换未知",
        ),
        "if" => (
            "条件分支需要求值",
            "按 then 分支字面量提取一次，未评估条件，else 分支被忽略",
        ),
        _ => ("未知的 Typst 构造", "相关内容按字面量处理"),
    };

    let mut construct_end = statement_end(&source.text, start, end);
    if matches!(ident, "for" | "while" | "if") {
        if let Some((open, close)) = find_content_block(&source.text, after_ident, end) {
            parse_segment(
                source,
                open + 1,
                close,
                false,
                false,
                intents,
                outcome,
                issues,
            );
            construct_end = close + 1;
            if ident == "if" {
                construct_end = consume_else_block(source, construct_end, end);
            }
        } else {
            issues.push(Issue::unsupported(
                format!("无法定位 `#{ident}` 的内容块"),
                source.snippet(start, construct_end, 40),
                "已丢弃该代码",
                Some(source.span(start, construct_end)),
            ));
        }
    }
    let span = source.span(start, construct_end);
    issues.push(Issue::non_deterministic(
        format!("`#{ident}` 不可静态确定"),
        reason,
        impact,
        Some(span),
    ));
    outcome.mark_degraded(reason, impact);
    construct_end
}

#[allow(clippy::too_many_arguments)]
fn handle_unknown_code(
    source: &SourceText,
    start: usize,
    ident: &str,
    spaced: usize,
    end: usize,
    intents: &[StyleIntent],
    outcome: &mut InlineOutcome,
    issues: &mut Vec<Issue>,
) -> usize {
    let text = &source.text;
    let mut construct_end = spaced;
    let mut content = None;
    if spaced < end && text.as_bytes()[spaced] == b'(' {
        if let Some(close) = find_matching(text, spaced, end) {
            construct_end = close + 1;
            let after = skip_spaces(text, close + 1, end);
            if after < end && text.as_bytes()[after] == b'[' {
                if let Some(content_close) = find_matching(text, after, end) {
                    content = Some((after + 1, content_close));
                    construct_end = content_close + 1;
                }
            }
        } else {
            construct_end = statement_end(text, start, end);
        }
    } else if spaced < end && text.as_bytes()[spaced] == b'[' {
        if let Some(content_close) = find_matching(text, spaced, end) {
            content = Some((spaced + 1, content_close));
            construct_end = content_close + 1;
        }
    }

    let handling = if content.is_some() {
        "保留内容块文字，丢弃该调用"
    } else {
        "已丢弃该调用"
    };
    issues.push(Issue::unsupported(
        format!("不支持的 Typst 函数或变量 `#{ident}`"),
        source.snippet(start, construct_end, 40),
        handling,
        Some(source.span(start, construct_end)),
    ));
    if let Some((inner_start, inner_close)) = content {
        parse_segment(
            source,
            inner_start,
            inner_close,
            false,
            false,
            intents,
            outcome,
            issues,
        );
    }
    outcome.mark_degraded(
        format!("未支持的调用 `#{ident}`"),
        "调用语义未知，仅按内容块字面量提取",
    );
    construct_end
}

fn consume_else_block(source: &SourceText, start: usize, end: usize) -> usize {
    let text = &source.text;
    let spaced = skip_spaces(text, start, end);
    if !text[spaced..end].starts_with("else") {
        return start;
    }
    let after_else = spaced + 4;
    let after = skip_spaces(text, after_else, end);
    if after < end && text.as_bytes()[after] == b'[' {
        if let Some(close) = find_matching(text, after, end) {
            return close + 1;
        }
    }
    start
}

fn handle_math(
    source: &SourceText,
    index: usize,
    end: usize,
    intents: &[StyleIntent],
    outcome: &mut InlineOutcome,
    issues: &mut Vec<Issue>,
) -> usize {
    let content_start = index + 1;
    if let Some(close) = source.text[content_start..end].find('$') {
        let content_end = content_start + close;
        issues.push(Issue::unsupported(
            "不支持 Typst 数学公式",
            source.snippet(index, content_end + 1, 40),
            "按字面文本保留公式源码",
            Some(source.span(index, content_end + 1)),
        ));
        push_run(
            outcome,
            &source.text[content_start..content_end],
            false,
            false,
            intents,
            source.span(content_start, content_end),
        );
        content_end + 1
    } else {
        push_run(
            outcome,
            "$",
            false,
            false,
            intents,
            source.span(index, index + 1),
        );
        index + 1
    }
}

fn handle_raw(
    source: &SourceText,
    index: usize,
    end: usize,
    intents: &[StyleIntent],
    outcome: &mut InlineOutcome,
    issues: &mut Vec<Issue>,
) -> usize {
    let bytes = source.text.as_bytes();
    let run = bytes[index..end]
        .iter()
        .take_while(|byte| **byte == b'`')
        .count();
    let content_start = index + run;
    let mut cursor = content_start;
    while cursor < end {
        if bytes[cursor] == b'`' {
            let closing = bytes[cursor..end]
                .iter()
                .take_while(|byte| **byte == b'`')
                .count();
            if closing >= run {
                issues.push(Issue::unsupported(
                    "不支持 Typst 行内原始文本",
                    source.snippet(index, cursor + closing, 40),
                    "移除反引号，内容按普通文本保留",
                    Some(source.span(index, cursor + closing)),
                ));
                push_run(
                    outcome,
                    &source.text[content_start..cursor],
                    false,
                    false,
                    intents,
                    source.span(content_start, cursor),
                );
                return cursor + closing;
            }
            cursor += closing;
        } else {
            cursor += 1;
        }
    }
    issues.push(Issue::unsupported(
        "Typst 行内原始文本未闭合",
        source.snippet(index, end, 40),
        "按字面文本保留",
        Some(source.span(index, end)),
    ));
    push_run(
        outcome,
        &source.text[index..end],
        false,
        false,
        intents,
        source.span(index, end),
    );
    end
}

fn handle_label(source: &SourceText, index: usize, end: usize, issues: &mut Vec<Issue>) -> usize {
    if let Some(close) = source.text[index..end].find('>') {
        let construct_end = index + close + 1;
        issues.push(Issue::unsupported(
            "不支持 Typst 标签",
            source.snippet(index, construct_end, 40),
            "已忽略标签（标签不产生可见内容）",
            Some(source.span(index, construct_end)),
        ));
        construct_end
    } else {
        index + 1
    }
}

fn handle_reference(
    source: &SourceText,
    index: usize,
    end: usize,
    intents: &[StyleIntent],
    outcome: &mut InlineOutcome,
    issues: &mut Vec<Issue>,
) -> usize {
    let mut cursor = index + 1;
    while cursor < end {
        let ch = source.text[cursor..].chars().next().unwrap();
        if ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':') {
            cursor += ch.len_utf8();
        } else {
            break;
        }
    }
    if cursor == index + 1 {
        push_run(
            outcome,
            "@",
            false,
            false,
            intents,
            source.span(index, index + 1),
        );
        return index + 1;
    }
    issues.push(Issue::unsupported(
        "不支持 Typst 引用",
        source.snippet(index, cursor, 40),
        "按字面文本保留",
        Some(source.span(index, cursor)),
    ));
    push_run(
        outcome,
        &source.text[index..cursor],
        false,
        false,
        intents,
        source.span(index, cursor),
    );
    cursor
}

fn parse_text_args(
    source: &SourceText,
    start: usize,
    end: usize,
    issues: &mut Vec<Issue>,
) -> Vec<StyleIntent> {
    let text = &source.text;
    let mut intents = Vec::new();
    for (part_start, part_end) in split_top_level(text, start, end, ',') {
        let (part_start, part_end) = trim_range(text, part_start, part_end);
        if part_start >= part_end {
            continue;
        }
        let Some(colon) = find_top_level_colon(text, part_start, part_end) else {
            issues.push(Issue::unsupported(
                "`#text` 暂不支持位置参数",
                source.snippet(part_start, part_end, 40),
                "已忽略该参数",
                Some(source.span(part_start, part_end)),
            ));
            continue;
        };
        let (key_start, key_end) = trim_range(text, part_start, colon);
        let (value_start, value_end) = trim_range(text, colon + 1, part_end);
        let key = &text[key_start..key_end];
        if value_start >= value_end {
            continue;
        }
        let value_span = source.span(value_start, value_end);
        match key {
            "font" => {
                if let Some((value, span)) = parse_font_value(source, value_start, value_end) {
                    intents.push(StyleIntent {
                        property: StyleProperty::FontFamily { value },
                        frontend: FrontendId::Typst,
                        source: span,
                    });
                } else {
                    issues.push(Issue::unsupported(
                        "无法静态解析 `font` 参数",
                        source.snippet(value_start, value_end, 40),
                        "已忽略该参数",
                        Some(value_span),
                    ));
                }
            }
            "size" => match parse_pt_size(text, value_start, value_end) {
                Ok(points) => intents.push(StyleIntent {
                    property: StyleProperty::FontSize {
                        points: points as f64,
                    },
                    frontend: FrontendId::Typst,
                    source: value_span,
                }),
                Err(message) => issues.push(Issue::unsupported(
                    format!("字号参数无法采纳：{message}"),
                    source.snippet(value_start, value_end, 40),
                    "仅支持 pt 单位的字号，已忽略该参数",
                    Some(value_span),
                )),
            },
            "fill" => match parse_color(text, value_start, value_end) {
                Some(hex) => intents.push(StyleIntent {
                    property: StyleProperty::Color { hex },
                    frontend: FrontendId::Typst,
                    source: value_span,
                }),
                None => issues.push(Issue::unsupported(
                    "无法静态解析 `fill` 颜色",
                    source.snippet(value_start, value_end, 40),
                    "已忽略该参数",
                    Some(value_span),
                )),
            },
            "weight" => match parse_weight(text, value_start, value_end) {
                Some(value) => intents.push(StyleIntent {
                    property: StyleProperty::Bold { value },
                    frontend: FrontendId::Typst,
                    source: value_span,
                }),
                None => issues.push(Issue::unsupported(
                    "无法静态解析 `weight` 参数",
                    source.snippet(value_start, value_end, 40),
                    "已忽略该参数",
                    Some(value_span),
                )),
            },
            "style" => match parse_font_style(text, value_start, value_end) {
                Some(value) => intents.push(StyleIntent {
                    property: StyleProperty::Italic { value },
                    frontend: FrontendId::Typst,
                    source: value_span,
                }),
                None => issues.push(Issue::unsupported(
                    "无法静态解析 `style` 参数",
                    source.snippet(value_start, value_end, 40),
                    "已忽略该参数",
                    Some(value_span),
                )),
            },
            other => issues.push(Issue::unsupported(
                format!("不支持的 `#text` 参数 `{other}`"),
                source.snippet(part_start, part_end, 40),
                "已忽略该参数",
                Some(source.span(part_start, part_end)),
            )),
        }
    }
    intents
}

fn parse_font_value(
    source: &SourceText,
    start: usize,
    end: usize,
) -> Option<(String, charta::SourceSpan)> {
    let text = &source.text;
    if text.as_bytes()[start] == b'"' {
        let parsed = parse_string_literal(text, start, end)?;
        if text[parsed.next..end].trim().is_empty() {
            return Some((
                parsed.value,
                source.span(parsed.content_start, parsed.content_end),
            ));
        }
    }
    if text.as_bytes()[start] == b'[' {
        let close = find_matching(text, start, end)?;
        let mut cursor = start + 1;
        while cursor < close {
            if text.as_bytes()[cursor] == b'"' {
                let parsed = parse_string_literal(text, cursor, close)?;
                return Some((
                    parsed.value,
                    source.span(parsed.content_start, parsed.content_end),
                ));
            }
            cursor += 1;
        }
    }
    None
}

fn parse_pt_size(text: &str, start: usize, end: usize) -> Result<f32, String> {
    let raw = &text[start..end];
    let mut number_end = 0;
    let bytes = raw.as_bytes();
    if number_end < bytes.len() && (bytes[number_end] == b'+' || bytes[number_end] == b'-') {
        number_end += 1;
    }
    let mut seen_dot = false;
    while number_end < bytes.len() {
        match bytes[number_end] {
            b'0'..=b'9' => number_end += 1,
            b'.' if !seen_dot => {
                seen_dot = true;
                number_end += 1;
            }
            _ => break,
        }
    }
    if number_end == 0 {
        return Err("缺少数字".to_owned());
    }
    let number: f64 = raw[..number_end]
        .parse()
        .map_err(|_| "数字格式不正确".to_owned())?;
    if number <= 0.0 {
        return Err("字号必须为正数".to_owned());
    }
    let unit = raw[number_end..].trim();
    if !unit.eq_ignore_ascii_case("pt") {
        return Err(format!("不支持的尺寸单位 `{unit}`"));
    }
    Ok(number as f32)
}

fn parse_color(text: &str, start: usize, end: usize) -> Option<String> {
    let raw = text[start..end].trim();
    if raw.starts_with('#') {
        return normalize_hex_color(raw);
    }
    if let Some(inner) = raw
        .strip_prefix("rgb(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let inner = inner.trim();
        if inner.starts_with('"') {
            let parsed = parse_string_literal(inner, 0, inner.len())?;
            return normalize_hex_color(&parsed.value);
        }
        let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
        if parts.len() == 3 {
            let values: Option<Vec<u8>> =
                parts.iter().map(|part| part.parse::<u8>().ok()).collect();
            let values = values?;
            return Some(format!(
                "#{:02X}{:02X}{:02X}",
                values[0], values[1], values[2]
            ));
        }
        return None;
    }
    named_color(raw)
}

fn named_color(name: &str) -> Option<String> {
    let rgb = match name.to_ascii_lowercase().as_str() {
        "black" => [0, 0, 0],
        "white" => [255, 255, 255],
        "red" => [255, 0, 0],
        "green" => [0, 128, 0],
        "blue" => [0, 0, 255],
        "yellow" => [255, 255, 0],
        "gray" | "grey" => [128, 128, 128],
        "orange" => [255, 165, 0],
        "purple" => [128, 0, 128],
        "teal" => [0, 128, 128],
        "navy" => [0, 0, 128],
        "maroon" => [128, 0, 0],
        "olive" => [128, 128, 0],
        "silver" => [192, 192, 192],
        "lime" => [0, 255, 0],
        "aqua" => [0, 255, 255],
        "fuchsia" => [255, 0, 255],
        _ => return None,
    };
    Some(format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]))
}

fn parse_weight(text: &str, start: usize, end: usize) -> Option<bool> {
    let value = unquote(text, start, end).to_ascii_lowercase();
    match value.as_str() {
        "bold" | "semibold" | "extrabold" | "black" => Some(true),
        "regular" | "normal" | "light" => Some(false),
        _ => value.parse::<u16>().ok().map(|weight| weight >= 600),
    }
}

fn parse_font_style(text: &str, start: usize, end: usize) -> Option<bool> {
    match unquote(text, start, end).to_ascii_lowercase().as_str() {
        "italic" | "oblique" => Some(true),
        "normal" => Some(false),
        _ => None,
    }
}

fn next_special(text: &str, from: usize, end: usize) -> usize {
    let mut index = from;
    while index < end {
        let ch = text[index..].chars().next().unwrap();
        if matches!(
            ch,
            '\\' | '*' | '_' | '#' | '$' | '`' | '<' | '@' | '\n' | '\r'
        ) {
            return index;
        }
        index += ch.len_utf8();
    }
    end
}

fn push_run(
    outcome: &mut InlineOutcome,
    text: &str,
    strong: bool,
    emphasis: bool,
    intents: &[StyleIntent],
    source: charta::SourceSpan,
) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = outcome.runs.last_mut() {
        if last.strong == strong
            && last.emphasis == emphasis
            && last.intents == intents
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
    outcome.runs.push(Inline {
        text: text.to_owned(),
        strong,
        emphasis,
        intents: intents.to_vec(),
        source,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_text(source: &str) -> InlineOutcome {
        let source = SourceText::new("a.typ", source);
        let mut issues = Vec::new();
        let outcome = parse(&source, 0, source.text.len(), &[], &mut issues);
        assert!(issues.is_empty(), "{issues:?}");
        outcome
    }

    #[test]
    fn parses_text_call_intents() {
        let outcome =
            parse_text("前 #text(font: \"SimSun\", size: 12pt, fill: #c00, weight: bold)[重点] 后");
        let run = outcome
            .runs
            .iter()
            .find(|run| run.text == "重点")
            .expect("styled run");
        assert_eq!(run.intents.len(), 4);
        assert!(run.intents.iter().any(|intent| matches!(
            &intent.property,
            StyleProperty::Bold { value } if *value
        )));
        assert!(run.intents.iter().any(|intent| matches!(
            &intent.property,
            StyleProperty::Color { hex } if hex == "#CC0000"
        )));
        assert!(run.intents.iter().any(|intent| matches!(
            &intent.property,
            StyleProperty::FontSize { points } if (*points - 12.0).abs() < f64::EPSILON
        )));
        assert!(run.intents.iter().any(|intent| matches!(
            &intent.property,
            StyleProperty::FontFamily { value } if value == "SimSun"
        )));
    }

    #[test]
    fn reports_unknown_code_and_keeps_content() {
        let source = SourceText::new("a.typ", "#link(\"https://x\")[链接文字]");
        let mut issues = Vec::new();
        let outcome = parse(&source, 0, source.text.len(), &[], &mut issues);
        assert!(outcome.degraded.is_some());
        assert!(outcome.runs.iter().any(|run| run.text == "链接文字"));
        assert!(issues
            .iter()
            .any(|issue| issue.message.contains("不支持的 Typst 函数")));
    }

    #[test]
    fn parses_strong_and_emphasis() {
        let outcome = parse_text("普通 *加粗* _斜体_");
        assert!(outcome
            .runs
            .iter()
            .any(|run| run.strong && run.text == "加粗"));
        assert!(outcome
            .runs
            .iter()
            .any(|run| run.emphasis && run.text == "斜体"));
    }

    #[test]
    fn parses_strong_and_emph_calls_as_aliases() {
        let outcome = parse_text("前 #strong[加粗] 中 #emph[斜体] 后");
        assert!(outcome
            .runs
            .iter()
            .any(|run| run.strong && !run.emphasis && run.text == "加粗"));
        assert!(outcome
            .runs
            .iter()
            .any(|run| run.emphasis && !run.strong && run.text == "斜体"));
    }

    #[test]
    fn style_alias_keeps_nested_markup() {
        let outcome = parse_text("#strong[外层 _内层_ 结尾]");
        let text: String = outcome.runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(text, "外层 内层 结尾");
        assert!(outcome.runs.iter().all(|run| run.strong));
        assert!(outcome.runs.iter().any(|run| run.emphasis));
    }
}

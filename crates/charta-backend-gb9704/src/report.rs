//! Report entries owned by the GB/T 9704 backend.
//!
//! Layout code decides what happened; this module owns the user-facing
//! messages, codes and severities.

use charta::{Issue, SourceSpan, StyleIntent};

pub(crate) fn degraded_content(reason: &str, impact: &str, source: &SourceSpan) -> Issue {
    Issue::render_warning(
        "不可静态确定内容按降级结构渲染",
        "degraded_content",
        Some(format!("原因：{reason}；影响：{impact}")),
        Some(source.clone()),
    )
}

pub(crate) fn image_read_failed(path: &str, reason: &str, source: &SourceSpan) -> Issue {
    Issue::render_warning(
        format!("图片读取失败：{path}"),
        "image_read_failed",
        Some(reason.to_owned()),
        Some(source.clone()),
    )
}

pub(crate) fn image_decode_failed(path: &str, reason: &str, source: &SourceSpan) -> Issue {
    Issue::render_warning(
        format!("图片格式无法嵌入：{path}"),
        "image_decode_failed",
        Some(reason.to_owned()),
        Some(source.clone()),
    )
}

pub(crate) fn list_nesting_flattened(source: &SourceSpan) -> Issue {
    Issue::render_warning(
        "列表嵌套超过四级，第五级起按正文段落渲染",
        "list_nesting_flattened",
        Some("GB/T 9704 后端只生成（1）、①、A.、a. 四级列表序数".to_owned()),
        Some(source.clone()),
    )
}

pub(crate) fn list_number_format_fallback(number: u32, source: &SourceSpan) -> Issue {
    Issue::render_warning(
        format!("圈号 {number} 超出 50，已回落为括号数字"),
        "list_number_format_fallback",
        Some("①–㊿ 覆盖 1–50；更大序号使用 (n)".to_owned()),
        Some(source.clone()),
    )
}

pub(crate) fn empty_table(source: &SourceSpan) -> Issue {
    Issue::render_warning("空表格已跳过", "empty_table", None, Some(source.clone()))
}

pub(crate) fn table_cells_overflow(row_index: usize, source: &SourceSpan) -> Issue {
    Issue::render_warning(
        "表格单元格超出网格列数，多余单元格已忽略",
        "table_cells_overflow",
        Some(format!("第 {} 行", row_index + 1)),
        Some(source.clone()),
    )
}

pub(crate) fn table_rowspan_truncated(source: &SourceSpan) -> Issue {
    Issue::render_warning(
        "表格纵向合并超出最后一行，已截断",
        "table_rowspan_truncated",
        None,
        Some(source.clone()),
    )
}

pub(crate) fn ignored_metadata(field: &str, reason: &str) -> Issue {
    Issue::render_warning(
        format!("元数据 `{field}` 未被当前后端使用"),
        "metadata_ignored",
        Some(reason.to_owned()),
        None,
    )
}

pub(crate) fn image_caption_static(source: &SourceSpan) -> Issue {
    Issue::render_warning(
        "图片题注采用静态文本编号，不是 Word 自动编号",
        "image_caption_static",
        Some("序号形式：图 N，由后端按带题注图片的出现顺序生成".to_owned()),
        Some(source.clone()),
    )
}

pub(crate) fn heading_numbering_word(source: &SourceSpan) -> Issue {
    Issue::render_warning(
        "标题采用 Word 内置多级列表编号",
        "heading_numbering_word",
        Some(
            "编号定义写入 numbering.xml，Word 会随标题增删自动重排；纯文本抽取不包含编号文字"
                .to_owned(),
        ),
        Some(source.clone()),
    )
}

pub(crate) fn heading_numbering_static(source: &SourceSpan) -> Issue {
    Issue::render_warning(
        "标题采用静态文本编号，不是 Word 自动编号",
        "heading_numbering_static",
        Some("序号形式：一、/（一）/1./（1），由后端按标题层级生成".to_owned()),
        Some(source.clone()),
    )
}

pub(crate) fn heading_numbering_flattened(source: &SourceSpan) -> Issue {
    Issue::render_warning(
        "标题层级超过四级，未生成序号",
        "heading_numbering_flattened",
        Some("GB/T 9704 层次序数只定义四级".to_owned()),
        Some(source.clone()),
    )
}

pub(crate) fn style_override(
    intent: &StyleIntent,
    adopted: impl ToString,
    reason: impl Into<String>,
) -> Issue {
    Issue::style_overridden(
        format!("样式意图 `{}` 被覆盖", intent.property.name()),
        intent.property.name(),
        intent.property.value_display(),
        adopted.to_string(),
        reason,
        Some(intent.source.clone()),
    )
}

pub(crate) fn disabled_intent_reason() -> &'static str {
    "GB/T 9704 后端未启用该属性的采纳"
}

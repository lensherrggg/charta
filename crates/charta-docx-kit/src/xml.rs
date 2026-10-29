use crate::model::{
    Border, Indent, LineSpacing, ParagraphBorders, ParagraphFormatting, RunProperties, Spacing,
};

pub(crate) const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

pub(crate) fn escape_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' => {}
            _ => out.push(ch),
        }
    }
    out
}

pub(crate) fn escape_attr(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' => {}
            _ => out.push(ch),
        }
    }
    out
}

pub(crate) fn mm_to_twips(mm: f32) -> i32 {
    (mm * 1440.0 / 25.4).round() as i32
}

pub(crate) fn pt_to_twips(points: f32) -> i32 {
    (points * 20.0).round() as i32
}

pub(crate) fn color_hex(color: [u8; 3]) -> String {
    format!("{:02X}{:02X}{:02X}", color[0], color[1], color[2])
}

pub(crate) fn paragraph_properties_xml(
    formatting: &ParagraphFormatting,
    include_style: bool,
) -> String {
    let mut inner = String::new();
    if include_style {
        if let Some(style) = &formatting.style {
            inner.push_str(&format!("<w:pStyle w:val=\"{}\"/>", escape_attr(style)));
        }
    }
    if formatting.keep_next {
        inner.push_str("<w:keepNext/>");
    }
    if formatting.keep_lines {
        inner.push_str("<w:keepLines/>");
    }
    if formatting.page_break_before {
        inner.push_str("<w:pageBreakBefore/>");
    }
    if let Some(numbering) = formatting.numbering {
        inner.push_str(&format!(
            "<w:numPr><w:ilvl w:val=\"{}\"/><w:numId w:val=\"{}\"/></w:numPr>",
            numbering.level, numbering.num_id
        ));
    }
    let borders = borders_xml(&formatting.borders);
    if !borders.is_empty() {
        inner.push_str(&borders);
    }
    if let Some(shading) = formatting.shading {
        inner.push_str(&format!(
            "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>",
            color_hex(shading)
        ));
    }
    if !formatting.tabs.is_empty() {
        inner.push_str("<w:tabs>");
        for tab in &formatting.tabs {
            inner.push_str(&format!(
                "<w:tab w:val=\"{}\" w:leader=\"{}\" w:pos=\"{}\"/>",
                tab.alignment.xml_value(),
                tab.leader.xml_value(),
                tab.position_twips
            ));
        }
        inner.push_str("</w:tabs>");
    }
    if let Some(word_wrap) = formatting.word_wrap {
        inner.push_str(&format!(
            "<w:wordWrap w:val=\"{}\"/>",
            if word_wrap { 1 } else { 0 }
        ));
    }
    let spacing = spacing_xml(&formatting.spacing);
    if !spacing.is_empty() {
        inner.push_str(&spacing);
    }
    let indent = indent_xml(&formatting.indent);
    if !indent.is_empty() {
        inner.push_str(&indent);
    }
    if let Some(alignment) = formatting.alignment {
        inner.push_str(&format!("<w:jc w:val=\"{}\"/>", alignment.xml_value()));
    }
    if let Some(level) = formatting.outline_level {
        inner.push_str(&format!("<w:outlineLvl w:val=\"{}\"/>", level.min(8)));
    }
    if inner.is_empty() {
        String::new()
    } else {
        format!("<w:pPr>{inner}</w:pPr>")
    }
}

fn spacing_xml(spacing: &Spacing) -> String {
    let mut attrs = String::new();
    if let Some(before) = spacing.before_pt {
        attrs.push_str(&format!(" w:before=\"{}\"", pt_to_twips(before)));
    }
    if let Some(after) = spacing.after_pt {
        attrs.push_str(&format!(" w:after=\"{}\"", pt_to_twips(after)));
    }
    match spacing.line {
        Some(LineSpacing::Single) => {
            attrs.push_str(" w:line=\"240\" w:lineRule=\"auto\"");
        }
        Some(LineSpacing::Exact { points }) => {
            attrs.push_str(&format!(
                " w:line=\"{}\" w:lineRule=\"exact\"",
                pt_to_twips(points)
            ));
        }
        Some(LineSpacing::AtLeast { points }) => {
            attrs.push_str(&format!(
                " w:line=\"{}\" w:lineRule=\"atLeast\"",
                pt_to_twips(points)
            ));
        }
        Some(LineSpacing::Multiple(value)) => {
            attrs.push_str(&format!(
                " w:line=\"{}\" w:lineRule=\"auto\"",
                (240.0 * value).round() as i32
            ));
        }
        None => {}
    }
    if attrs.is_empty() {
        String::new()
    } else {
        format!("<w:spacing{attrs}/>")
    }
}

fn indent_xml(indent: &Indent) -> String {
    let mut attrs = String::new();
    if let Some(chars) = indent.left_chars {
        attrs.push_str(&format!(
            " w:leftChars=\"{}\"",
            (chars * 100.0).round() as i32
        ));
    }
    if let Some(twips) = indent.left_twips {
        attrs.push_str(&format!(" w:left=\"{twips}\""));
    }
    if let Some(chars) = indent.right_chars {
        attrs.push_str(&format!(
            " w:rightChars=\"{}\"",
            (chars * 100.0).round() as i32
        ));
    }
    if let Some(twips) = indent.right_twips {
        attrs.push_str(&format!(" w:right=\"{twips}\""));
    }
    if let Some(chars) = indent.first_line_chars {
        attrs.push_str(&format!(
            " w:firstLineChars=\"{}\"",
            (chars * 100.0).round() as i32
        ));
    }
    if let Some(twips) = indent.first_line_twips {
        attrs.push_str(&format!(" w:firstLine=\"{twips}\""));
    }
    if let Some(chars) = indent.hanging_chars {
        attrs.push_str(&format!(
            " w:hangingChars=\"{}\"",
            (chars * 100.0).round() as i32
        ));
    }
    if let Some(twips) = indent.hanging_twips {
        attrs.push_str(&format!(" w:hanging=\"{twips}\""));
    }
    if attrs.is_empty() {
        String::new()
    } else {
        format!("<w:ind{attrs}/>")
    }
}

fn borders_xml(borders: &ParagraphBorders) -> String {
    let mut inner = String::new();
    if let Some(border) = borders.top {
        inner.push_str(&border_xml("top", &border));
    }
    if let Some(border) = borders.bottom {
        inner.push_str(&border_xml("bottom", &border));
    }
    if let Some(border) = borders.left {
        inner.push_str(&border_xml("left", &border));
    }
    if let Some(border) = borders.right {
        inner.push_str(&border_xml("right", &border));
    }
    if inner.is_empty() {
        String::new()
    } else {
        format!("<w:pBdr>{inner}</w:pBdr>")
    }
}

pub(crate) fn border_xml(tag: &str, border: &Border) -> String {
    format!(
        "<w:{tag} w:val=\"single\" w:sz=\"{}\" w:space=\"{}\" w:color=\"{}\"/>",
        border.size_eighths_pt,
        border.space_pt,
        border.color_hex()
    )
}

pub(crate) fn run_properties_xml(properties: &RunProperties) -> String {
    let mut inner = String::new();
    if properties.font_ascii.is_some()
        || properties.font_hansi.is_some()
        || properties.font_east_asia.is_some()
        || properties.font_cs.is_some()
    {
        let mut attrs = String::new();
        if let Some(font) = &properties.font_ascii {
            attrs.push_str(&format!(" w:ascii=\"{}\"", escape_attr(font)));
        }
        if let Some(font) = &properties.font_hansi {
            attrs.push_str(&format!(" w:hAnsi=\"{}\"", escape_attr(font)));
        }
        if let Some(font) = &properties.font_east_asia {
            attrs.push_str(&format!(" w:eastAsia=\"{}\"", escape_attr(font)));
        }
        if let Some(font) = &properties.font_cs {
            attrs.push_str(&format!(" w:cs=\"{}\"", escape_attr(font)));
        }
        inner.push_str(&format!("<w:rFonts{attrs}/>"));
    }
    if let Some(bold) = properties.bold {
        if bold {
            inner.push_str("<w:b/>");
        } else {
            inner.push_str("<w:b w:val=\"0\"/>");
        }
    }
    if let Some(italic) = properties.italic {
        if italic {
            inner.push_str("<w:i/>");
        } else {
            inner.push_str("<w:i w:val=\"0\"/>");
        }
    }
    if let Some(color) = properties.color {
        inner.push_str(&format!("<w:color w:val=\"{}\"/>", color_hex(color)));
    }
    if let Some(spacing) = properties.spacing_twips {
        inner.push_str(&format!("<w:spacing w:val=\"{spacing}\"/>"));
    }
    if let Some(scale) = properties.character_scale {
        inner.push_str(&format!("<w:w w:val=\"{}\"/>", scale.clamp(1, 600)));
    }
    if let Some(size) = properties.size_half_points {
        inner.push_str(&format!(
            "<w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/>"
        ));
    }
    if properties.underline {
        inner.push_str("<w:u w:val=\"single\"/>");
    }
    if inner.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{inner}</w:rPr>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_xml_and_drops_control_characters() {
        assert_eq!(escape_text("a<b&c>\"d\""), "a&lt;b&amp;c&gt;\"d\"");
        assert_eq!(escape_text("a\u{0}b"), "ab");
        assert_eq!(escape_attr("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn writes_exact_line_spacing_in_twips() {
        let formatting = ParagraphFormatting {
            spacing: Spacing::exact(28.0),
            ..ParagraphFormatting::default()
        };
        let xml = paragraph_properties_xml(&formatting, true);
        assert!(xml.contains("w:line=\"560\""));
        assert!(xml.contains("w:lineRule=\"exact\""));
    }

    #[test]
    fn writes_character_scaling() {
        let properties = RunProperties::default().character_scale(86);
        let xml = run_properties_xml(&properties);
        assert!(xml.contains("<w:w w:val=\"86\"/>"));
    }

    #[test]
    fn writes_word_wrap_off() {
        let formatting = ParagraphFormatting {
            word_wrap: Some(false),
            ..ParagraphFormatting::default()
        };
        let xml = paragraph_properties_xml(&formatting, true);
        assert!(xml.contains("<w:wordWrap w:val=\"0\"/>"));
    }

    #[test]
    fn writes_right_indent_in_chars_and_twips() {
        let formatting = ParagraphFormatting {
            indent: Indent {
                right_chars: Some(0.0),
                right_twips: Some(0),
                ..Indent::default()
            },
            ..ParagraphFormatting::default()
        };
        let xml = paragraph_properties_xml(&formatting, true);
        assert!(xml.contains("w:rightChars=\"0\""));
        assert!(xml.contains("w:right=\"0\""));
    }
}

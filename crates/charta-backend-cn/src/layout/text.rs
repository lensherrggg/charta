//! Run construction and style-intent adoption for layout.

use charta::{Inline, Issue, StyleIntent, StyleProperty};
use charta_docx_kit::{Paragraph, Run, RunProperties};

use crate::report;
use crate::styles::{color_hex, parse_color, DocStyles, RunConfig, STYLE_EMPHASIS, STYLE_STRONG};
use crate::{DocOptions, DocTemplate};

pub(crate) fn build_runs(
    inlines: &[Inline],
    base: &RunConfig,
    options: &DocOptions,
    styles: &DocStyles,
    issues: &mut Vec<Issue>,
) -> Vec<Run> {
    let mut runs = Vec::new();
    for inline in inlines {
        let mut font = base.east_asia_font.clone();
        let mut size = base.size_pt;
        let mut bold = base.bold || inline.strong;
        let mut italic = inline.emphasis;
        let mut color = base.color;

        if matches!(
            options.template,
            DocTemplate::General | DocTemplate::Compilation
        ) {
            italic = false;
            if inline.strong {
                if let Some(style) = styles.try_run(STYLE_STRONG) {
                    font = style.east_asia_font.clone();
                    bold = style.bold;
                }
            } else if inline.emphasis {
                if let Some(style) = styles.try_run(STYLE_EMPHASIS) {
                    font = style.east_asia_font.clone();
                    bold = style.bold;
                }
            }
        }

        for intent in &inline.intents {
            match &intent.property {
                StyleProperty::Color { hex } => {
                    if options.adopt_color {
                        if let Some(parsed) = parse_color(hex) {
                            color = parsed;
                        } else {
                            issues.push(override_intent(
                                intent,
                                color_hex(base.color),
                                format!("无法解析颜色值 `{hex}`"),
                            ));
                        }
                    } else {
                        issues.push(override_intent(
                            intent,
                            color_hex(base.color),
                            report::disabled_intent_reason(),
                        ));
                    }
                }
                StyleProperty::FontFamily { value } => {
                    if options.adopt_font_family {
                        font = value.clone();
                    } else {
                        issues.push(override_intent(
                            intent,
                            base.east_asia_font.clone(),
                            report::disabled_intent_reason(),
                        ));
                    }
                }
                StyleProperty::FontSize { points } => {
                    if options.adopt_font_size {
                        size = *points as f32;
                    } else {
                        issues.push(override_intent(
                            intent,
                            format!("{}pt", base.size_pt),
                            report::disabled_intent_reason(),
                        ));
                    }
                }
                StyleProperty::Bold { value } => {
                    if options.adopt_bold {
                        bold = *value;
                    } else {
                        issues.push(override_intent(
                            intent,
                            if base.bold { "加粗" } else { "常规" },
                            report::disabled_intent_reason(),
                        ));
                    }
                }
                StyleProperty::Italic { value } => {
                    if options.adopt_italic {
                        italic = *value;
                    } else {
                        issues.push(override_intent(
                            intent,
                            "常规",
                            report::disabled_intent_reason(),
                        ));
                    }
                }
            }
        }

        let properties = RunProperties::default()
            .font_ascii(&base.ascii_font)
            .font_hansi(&base.ascii_font)
            .font_east_asia(font)
            .font_cs(&base.ascii_font)
            .size_pt(size)
            .bold(bold)
            .italic(italic)
            .color(color);
        runs.push(Run::text(inline.text.clone()).properties(properties));
    }
    runs
}

fn override_intent(
    intent: &StyleIntent,
    adopted: impl ToString,
    reason: impl Into<String>,
) -> Issue {
    report::style_override(intent, adopted, reason)
}

pub(crate) fn styled_paragraph(
    style_id: &str,
    inlines: &[Inline],
    style: &RunConfig,
    options: &DocOptions,
    styles: &DocStyles,
    issues: &mut Vec<Issue>,
) -> Paragraph {
    let mut paragraph = Paragraph::styled(style_id);
    paragraph.runs = build_runs(inlines, style, options, styles, issues);
    paragraph
}

pub(crate) fn styled_run(text: &str, style: &RunConfig) -> Run {
    Run::text(text).properties(
        RunProperties::default()
            .font_ascii(&style.ascii_font)
            .font_hansi(&style.ascii_font)
            .font_east_asia(&style.east_asia_font)
            .font_cs(&style.ascii_font)
            .size_pt(style.size_pt)
            .bold(style.bold)
            .color(style.color),
    )
}

pub(crate) fn plain_paragraph(text: &str, style_id: &str, style: &RunConfig) -> Paragraph {
    let mut paragraph = Paragraph::styled(style_id);
    paragraph.runs.push(styled_run(text, style));
    paragraph
}

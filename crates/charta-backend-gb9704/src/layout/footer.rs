//! Odd/even page-number footers.

use charta_docx_kit::{Footer, Footers, Indent, Paragraph, Run, RunProperties};

use crate::styles::{Gb9704Styles, RunConfig, STYLE_PAGE_NUMBER};

pub(crate) fn build(styles: &Gb9704Styles) -> Footers {
    let config = styles.footer();
    let page_number = styles.run(STYLE_PAGE_NUMBER);
    let props = footer_run_properties(&config.ascii_font, page_number);
    let mut odd = Paragraph::styled(STYLE_PAGE_NUMBER);
    odd.formatting.alignment = Some(config.odd.alignment);
    odd.formatting.indent = Indent {
        right_chars: Some(config.odd.indent_chars),
        ..Indent::default()
    };
    odd.runs = vec![
        Run::text(config.prefix.as_str()).properties(props.clone()),
        Run::page_number().properties(props.clone()),
        Run::text(config.suffix.as_str()).properties(props),
    ];

    let even_props = footer_run_properties(&config.ascii_font, page_number);
    let mut even = Paragraph::styled(STYLE_PAGE_NUMBER);
    even.formatting.alignment = Some(config.even.alignment);
    even.formatting.indent = Indent {
        left_chars: Some(config.even.indent_chars),
        ..Indent::default()
    };
    even.runs = vec![
        Run::text(config.prefix.as_str()).properties(even_props.clone()),
        Run::page_number().properties(even_props.clone()),
        Run::text(config.suffix.as_str()).properties(even_props),
    ];

    Footers {
        default: Some(Footer::new(vec![odd])),
        even: Some(Footer::new(vec![even])),
        first: None,
    }
}

fn footer_run_properties(ascii_font: &str, style: &RunConfig) -> RunProperties {
    RunProperties::default()
        .font_ascii(ascii_font)
        .font_hansi(ascii_font)
        .font_east_asia(&style.east_asia_font)
        .size_pt(style.size_pt)
        .color(style.color)
}

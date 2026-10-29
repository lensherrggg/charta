//! 版头: copy number, security marks, organization mark, document number (and
//! the signer line of an upward document) and the red separator.

use charta::Metadata;
use charta_docx_kit::{Docx, Paragraph};

use crate::layout::string_field;
use crate::layout::text::{plain_paragraph, styled_run};
use crate::styles::{
    DocStyles, STYLE_COPY_NUMBER, STYLE_DOC_NUMBER, STYLE_DOC_NUMBER_UP, STYLE_MARK,
    STYLE_ORG_MARK, STYLE_RED_LINE, STYLE_SIGNER,
};

pub(crate) fn render(docx: &mut Docx, metadata: &Metadata, styles: &DocStyles) {
    if let Some(copy_number) = string_field(metadata, "copy_number") {
        docx.add_paragraph(plain_paragraph(
            copy_number,
            STYLE_COPY_NUMBER,
            styles.run(STYLE_COPY_NUMBER),
        ));
    }
    for key in ["security_level", "urgency"] {
        if let Some(value) = string_field(metadata, key) {
            docx.add_paragraph(plain_paragraph(value, STYLE_MARK, styles.run(STYLE_MARK)));
        }
    }

    let organization = string_field(metadata, "organization")
        .or_else(|| string_field(metadata, "issuer"))
        .unwrap_or_default();
    let doc_number = string_field(metadata, "doc_number").unwrap_or_default();

    let mut organization_paragraph =
        plain_paragraph(organization, STYLE_ORG_MARK, styles.run(STYLE_ORG_MARK));
    if let Some(scale) = organization_scale(organization, styles) {
        for run in &mut organization_paragraph.runs {
            run.properties.character_scale = Some(scale);
        }
    }
    docx.add_paragraph(organization_paragraph);
    docx.add_paragraph(document_number_paragraph(
        doc_number,
        string_field(metadata, "signer"),
        styles,
    ));
    docx.add_paragraph(Paragraph::styled(STYLE_RED_LINE));
}

/// Downward documents centre the document number; upward ones put it flush
/// left with one character of indent and place "签发人：姓名" on the same line,
/// one character from the right edge (GB/T 9704-2012 7.2.5, 7.2.6).
fn document_number_paragraph(
    doc_number: &str,
    signer: Option<&str>,
    styles: &DocStyles,
) -> Paragraph {
    let Some(signer) = signer else {
        return plain_paragraph(doc_number, STYLE_DOC_NUMBER, styles.run(STYLE_DOC_NUMBER));
    };
    let number = styles.run(STYLE_DOC_NUMBER_UP);
    let mut paragraph = plain_paragraph(doc_number, STYLE_DOC_NUMBER_UP, number);
    paragraph.runs.push(styled_run("\t", number));
    paragraph.runs.push(styled_run("签发人：", number));
    paragraph
        .runs
        .push(styled_run(signer, styles.run(STYLE_SIGNER)));
    paragraph
}

/// The red header keeps its 36 pt height; names wider than the text column are
/// narrowed horizontally (Word's 字符缩放) so they stay on one line.
fn organization_scale(text: &str, styles: &DocStyles) -> Option<u32> {
    let size_pt = styles.run(STYLE_ORG_MARK).size_pt;
    let text_width_mm = styles.page().setup().text_width_mm();
    let em_mm = size_pt * 25.4 / 72.0;
    let natural_mm: f32 = text
        .chars()
        .map(|ch| if ch.is_ascii() { em_mm * 0.55 } else { em_mm })
        .sum();
    if natural_mm <= text_width_mm || natural_mm <= 0.0 {
        return None;
    }
    let percent = (text_width_mm / natural_mm * 100.0).floor();
    Some((percent as u32).clamp(1, 100))
}

//! 版头: copy number, security marks, organization mark, document number and
//! the red separator.

use charta::Metadata;
use charta_docx_kit::{Docx, Paragraph};

use crate::layout::string_field;
use crate::layout::text::plain_paragraph;
use crate::styles::{
    DocStyles, STYLE_COPY_NUMBER, STYLE_DOC_NUMBER, STYLE_MARK, STYLE_ORG_MARK, STYLE_RED_LINE,
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

    docx.add_paragraph(plain_paragraph(
        organization,
        STYLE_ORG_MARK,
        styles.run(STYLE_ORG_MARK),
    ));
    docx.add_paragraph(plain_paragraph(
        doc_number,
        STYLE_DOC_NUMBER,
        styles.run(STYLE_DOC_NUMBER),
    ));
    docx.add_paragraph(Paragraph::styled(STYLE_RED_LINE));
}

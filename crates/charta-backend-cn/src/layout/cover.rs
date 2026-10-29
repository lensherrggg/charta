//! Cover page for the compilation template.

use charta::Metadata;
use charta_docx_kit::{Docx, Paragraph};

use crate::layout::text::styled_run;
use crate::layout::{chinese_date, chinese_date_numerals, string_field};
use crate::styles::{DocStyles, STYLE_COVER_DATE, STYLE_COVER_SPACER, STYLE_COVER_TITLE};

const BLANK_LINES_BEFORE: usize = 5;
const BLANK_LINES_AFTER: usize = 7;

pub(crate) fn render(docx: &mut Docx, metadata: &Metadata, styles: &DocStyles) {
    for _ in 0..BLANK_LINES_BEFORE {
        docx.add_paragraph(Paragraph::styled(STYLE_COVER_SPACER));
    }

    let title = string_field(metadata, "title").unwrap_or_default();
    let mut paragraph = Paragraph::styled(STYLE_COVER_TITLE);
    paragraph
        .runs
        .push(styled_run(title, styles.run(STYLE_COVER_TITLE)));
    docx.add_paragraph(paragraph);

    for _ in 0..BLANK_LINES_AFTER {
        docx.add_paragraph(Paragraph::styled(STYLE_COVER_SPACER));
    }

    let raw_date = string_field(metadata, "date").unwrap_or_default();
    let date = match string_field(metadata, "date_style") {
        Some("chinese") => chinese_date_numerals(raw_date),
        _ => chinese_date(raw_date),
    };
    let mut paragraph = Paragraph::styled(STYLE_COVER_DATE);
    paragraph
        .runs
        .push(styled_run(&date, styles.run(STYLE_COVER_DATE)));
    docx.add_paragraph(paragraph);
}

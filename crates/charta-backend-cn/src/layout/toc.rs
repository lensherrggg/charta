//! Table of contents page for the compilation template.

use charta_docx_kit::{Docx, Paragraph, Run};

use crate::layout::text::styled_run;
use crate::styles::{DocStyles, STYLE_TOC1, STYLE_TOC_SPACER, STYLE_TOC_TITLE};

const TOC_TITLE: &str = "目\u{3000}\u{3000}录";

/// Depths beyond this are reserved for later releases.
const DEPTH: u8 = 1;

pub(crate) fn render(docx: &mut Docx, styles: &DocStyles) {
    let mut title = Paragraph::styled(STYLE_TOC_TITLE);
    title.formatting.page_break_before = true;
    title
        .runs
        .push(styled_run(TOC_TITLE, styles.run(STYLE_TOC_TITLE)));
    docx.add_paragraph(title);
    docx.add_paragraph(Paragraph::styled(STYLE_TOC_SPACER));

    let mut field = Paragraph::styled(STYLE_TOC1);
    field.runs.push(Run::field_begin());
    field.runs.push(Run::instruction(format!(
        " TOC \\o \"1-{DEPTH}\" \\h \\z \\u "
    )));
    field.runs.push(Run::field_separate());
    field.runs.push(Run::field_end());
    docx.add_paragraph(field);
}

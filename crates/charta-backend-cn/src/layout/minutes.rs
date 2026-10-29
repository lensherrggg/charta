//! 纪要格式 (GB/T 9704-2012 10.3): the red meeting mark and the
//! attendance/absence/observer lists.

use charta::Metadata;
use charta_docx_kit::{Docx, Paragraph};

use crate::layout::header::organization_scale;
use crate::layout::list_field;
use crate::layout::text::{plain_paragraph, styled_run};
use crate::styles::{
    DocStyles, STYLE_ATTENDANCE, STYLE_BODY, STYLE_MARK, STYLE_ORG_MARK, STYLE_RED_LINE,
};

/// The mark is the meeting name plus "纪要", centred in red across the text
/// area. It reuses the 发文机关标志 metrics.
pub(crate) fn render_mark(docx: &mut Docx, metadata: &Metadata, styles: &DocStyles) {
    let meeting = crate::layout::string_field(metadata, "meeting").unwrap_or_default();
    let mark = format!("{meeting}纪要");
    let mut paragraph = plain_paragraph(&mark, STYLE_ORG_MARK, styles.run(STYLE_ORG_MARK));
    if let Some(scale) = organization_scale(&mark, styles) {
        for run in &mut paragraph.runs {
            run.properties.character_scale = Some(scale);
        }
    }
    docx.add_paragraph(paragraph);
    // 10.3 does not require a separator, but the common 纪要 house style puts a
    // red rule under the mark as the bottom of the letterhead.
    docx.add_paragraph(Paragraph::styled(STYLE_RED_LINE));
}

/// "出席：张三、李四" one line each, two characters from the left edge with a
/// three-character hanging indent so wrapped names line up after the colon.
pub(crate) fn render_attendance(docx: &mut Docx, metadata: &Metadata, styles: &DocStyles) {
    let mut first = true;
    for (label, key) in [
        ("出席", "attendees"),
        ("请假", "absentees"),
        ("列席", "observers"),
    ] {
        let Some(names) = list_field(metadata, key).filter(|names| !names.is_empty()) else {
            continue;
        };
        let mut paragraph = Paragraph::styled(STYLE_ATTENDANCE);
        if first {
            paragraph.formatting.spacing.before_pt = Some(28.0);
            first = false;
        }
        paragraph
            .runs
            .push(styled_run(&format!("{label}："), styles.run(STYLE_MARK)));
        paragraph
            .runs
            .push(styled_run(&names.join("、"), styles.run(STYLE_BODY)));
        docx.add_paragraph(paragraph);
    }
}

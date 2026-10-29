//! 版记: the dedicated last section and the copy/issuing record table.

use charta::Metadata;
use charta_docx_kit::{BodyItem, Docx, SectionBreak, Table, TableCell, TableRow};

use crate::layout::text::plain_paragraph;
use crate::layout::{chinese_date, list_field, string_field};
use crate::styles::{DocStyles, STYLE_VERSION};

pub(crate) fn render(docx: &mut Docx, metadata: &Metadata, styles: &DocStyles) {
    docx.push(BodyItem::SectionBreak(SectionBreak {
        page: docx.page,
        continuous: false,
        footer: true,
    }));
    docx.page.vertical_alignment = styles.page().version_record_vertical_alignment;
    // The record sits alone on the last page: it must start on an even page
    // (the final printed side) and neither it nor a preceding blank page
    // carries a page number.
    docx.final_section_even_page = true;
    docx.final_section_footer = false;

    let issuer = string_field(metadata, "issuer").unwrap_or_default();
    let date = string_field(metadata, "date").unwrap_or_default();
    let cc = list_field(metadata, "cc").unwrap_or_default();
    let office = string_field(metadata, "issuing_office")
        .unwrap_or(issuer)
        .to_owned();
    let issuing_date = string_field(metadata, "issuing_date")
        .unwrap_or(date)
        .to_owned();
    let config = styles.version_record();
    let line = config.border;
    let style = styles.run(STYLE_VERSION);
    let has_cc = !cc.is_empty();
    let mut rows = Vec::new();

    if has_cc {
        rows.push(TableRow {
            cells: vec![TableCell::new(vec![plain_paragraph(
                &format!("抄送：{}", cc.join("、")),
                STYLE_VERSION,
                style,
            )])
            .grid_span(2)],
            header: false,
            cant_split: true,
            height_mm: None,
        });
    }

    let left = plain_paragraph(&office, STYLE_VERSION, style);
    let mut right = plain_paragraph(
        &format!("{}印发", chinese_date(&issuing_date)),
        STYLE_VERSION,
        style,
    );
    right.formatting.alignment = Some(config.date_alignment);
    rows.push(TableRow {
        cells: vec![TableCell::new(vec![left]), TableCell::new(vec![right])],
        header: false,
        cant_split: true,
        height_mm: None,
    });

    let mut table = Table::new(config.columns_mm.clone(), rows);
    table.borders.top = Some(line);
    table.borders.bottom = Some(line);
    if has_cc {
        table.borders.inside_horizontal = Some(line);
    }
    table.cell_margin_mm = config.cell_margin_mm;
    table.width_mm = Some(config.width_mm);
    docx.add_table(table);
}

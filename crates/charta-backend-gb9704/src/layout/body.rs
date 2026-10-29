//! 主体: title, recipient, IR blocks, attachments, signature, date and note.

use std::path::{Path, PathBuf};

use charta::{
    Alignment, Block, BlockKind, Determinism, ImageBlock, Issue, ListBlock, Metadata, SourceSpan,
    TableBlock, TableCell as IrTableCell,
};
use charta_docx_kit::{
    Alignment as DocxAlignment, BodyItem, Docx, Image, NumberingDefinition, NumberingRef,
    PageSetup, Paragraph, Run, SectionBreak, Table, TableBorders, TableCell, TableRow, VMerge,
};

use crate::layout::text::{build_runs, plain_paragraph, styled_paragraph, styled_run};
use crate::layout::{chinese_date, list_field, string_field, with_colon, with_parentheses};
use crate::numbering::{figure_label, list_marker, max_list_depth, NumberingState};
use crate::report;
use crate::styles::{
    Gb9704Styles, RunConfig, STYLE_ATTACHMENT, STYLE_BODY, STYLE_BYLINE, STYLE_DATE,
    STYLE_HEADING1, STYLE_HEADING2, STYLE_HEADING3, STYLE_IMAGE, STYLE_IMAGE_CAPTION, STYLE_ISSUER,
    STYLE_NOTE, STYLE_QUOTE, STYLE_RECIPIENT, STYLE_TABLE_CAPTION, STYLE_TABLE_HEADER,
    STYLE_TABLE_TEXT, STYLE_TITLE,
};
use crate::Gb9704Options;
use crate::Gb9704Template;

pub(crate) fn render_main(
    docx: &mut Docx,
    metadata: &Metadata,
    blocks: &[Block],
    options: &Gb9704Options,
    styles: &Gb9704Styles,
    issues: &mut Vec<Issue>,
) {
    let title = string_field(metadata, "title").unwrap_or_default();
    docx.add_paragraph(plain_paragraph(title, STYLE_TITLE, styles.run(STYLE_TITLE)));
    if let Some(recipient) = string_field(metadata, "recipient") {
        docx.add_paragraph(plain_paragraph(
            &with_colon(recipient),
            STYLE_RECIPIENT,
            styles.run(STYLE_RECIPIENT),
        ));
    }

    let mut numbers = NumberingState::default();
    render_blocks(
        blocks,
        docx,
        options,
        styles,
        issues,
        &mut numbers,
        false,
        Gb9704Template::Notice,
    );

    if let Some(attachments) = list_field(metadata, "attachments") {
        let mut text = String::from("附件：");
        for (index, name) in attachments.iter().enumerate() {
            if index == 0 {
                text.push_str(&format!("1. {name}"));
            } else {
                text.push('\n');
                text.push_str(&format!("{}. {name}", index + 1));
            }
        }
        docx.add_paragraph(plain_paragraph(
            &text,
            STYLE_ATTACHMENT,
            styles.run(STYLE_ATTACHMENT),
        ));
    }

    let issuer = string_field(metadata, "issuer").unwrap_or_default();
    let date = string_field(metadata, "date").unwrap_or_default();
    docx.add_paragraph(Paragraph::styled(STYLE_BODY));
    docx.add_paragraph(plain_paragraph(
        issuer,
        STYLE_ISSUER,
        styles.run(STYLE_ISSUER),
    ));
    docx.add_paragraph(plain_paragraph(
        &chinese_date(date),
        STYLE_DATE,
        styles.run(STYLE_DATE),
    ));

    if let Some(note) = string_field(metadata, "note") {
        docx.add_paragraph(plain_paragraph(
            &with_parentheses(note),
            STYLE_NOTE,
            styles.run(STYLE_NOTE),
        ));
    }
}

pub(crate) fn render_general(
    docx: &mut Docx,
    metadata: &Metadata,
    blocks: &[Block],
    options: &Gb9704Options,
    styles: &Gb9704Styles,
    issues: &mut Vec<Issue>,
) {
    let mut numbers = NumberingState::default();
    let title_index = blocks
        .iter()
        .position(|block| matches!(&block.kind, BlockKind::Heading { level: 1, .. }));
    match title_index {
        Some(index) => {
            render_blocks(
                &blocks[..index],
                docx,
                options,
                styles,
                issues,
                &mut numbers,
                false,
                Gb9704Template::General,
            );
            render_blocks(
                &blocks[index..=index],
                docx,
                options,
                styles,
                issues,
                &mut numbers,
                false,
                Gb9704Template::General,
            );
            add_general_preface(docx, metadata, styles);
            render_blocks(
                &blocks[index + 1..],
                docx,
                options,
                styles,
                issues,
                &mut numbers,
                false,
                Gb9704Template::General,
            );
        }
        None => {
            add_general_preface(docx, metadata, styles);
            render_blocks(
                blocks,
                docx,
                options,
                styles,
                issues,
                &mut numbers,
                false,
                Gb9704Template::General,
            );
        }
    }
}

pub(crate) fn render_compilation_content(
    docx: &mut Docx,
    blocks: &[Block],
    options: &Gb9704Options,
    styles: &Gb9704Styles,
    issues: &mut Vec<Issue>,
) {
    // The cover and the TOC form their own section without a footer, so their
    // pages carry no page numbers; the body starts at page 1.
    docx.body.push(BodyItem::SectionBreak(section_break(
        docx.page, None, false,
    )));
    docx.page.page_number_start = Some(1);

    let mut numbers = NumberingState::default();
    let mut started = false;
    for block in blocks {
        // Chapters are separated by a plain page break; the body stays in one
        // section so page numbers run continuously.
        if started && matches!(&block.kind, BlockKind::Heading { level: 1, .. }) {
            docx.body.push(BodyItem::PageBreak);
        }
        render_blocks(
            std::slice::from_ref(block),
            docx,
            options,
            styles,
            issues,
            &mut numbers,
            false,
            Gb9704Template::Compilation,
        );
        started = true;
    }
}

fn section_break(page: PageSetup, page_number_start: Option<u32>, footer: bool) -> SectionBreak {
    SectionBreak {
        page: PageSetup {
            page_number_start,
            ..page
        },
        continuous: false,
        footer,
    }
}

fn add_general_preface(docx: &mut Docx, metadata: &Metadata, styles: &Gb9704Styles) {
    let author = string_field(metadata, "author").unwrap_or_default();
    let date = chinese_date(string_field(metadata, "date").unwrap_or_default());
    let author = author.trim();
    let byline = match (author.is_empty(), date.trim().is_empty()) {
        (false, false) => Some(format!("{}\n（{date}）", spaced_name(author))),
        (false, true) => Some(spaced_name(author)),
        (true, false) => Some(format!("（{date}）")),
        (true, true) => None,
    };
    if let Some(byline) = byline {
        docx.add_paragraph(plain_paragraph(
            &byline,
            STYLE_BYLINE,
            styles.run(STYLE_BYLINE),
        ));
    }

    if let Some(recipient) =
        string_field(metadata, "recipient").filter(|value| !value.trim().is_empty())
    {
        docx.add_paragraph(plain_paragraph(
            &with_colon(recipient),
            STYLE_RECIPIENT,
            styles.run(STYLE_RECIPIENT),
        ));
    }
}

pub(crate) fn first_title(blocks: &[Block]) -> Option<String> {
    blocks.iter().find_map(|block| match &block.kind {
        BlockKind::Heading {
            level: 1, content, ..
        } => {
            let text: String = content.iter().map(|run| run.text.as_str()).collect();
            (!text.trim().is_empty()).then_some(text)
        }
        _ => None,
    })
}

pub(crate) fn spaced_name(name: &str) -> String {
    let name = name.trim();
    let chars: Vec<char> = name.chars().collect();
    if chars.len() == 2 && chars.iter().all(|ch| !ch.is_whitespace()) {
        format!("{}\u{3000}{}", chars[0], chars[1])
    } else {
        name.to_owned()
    }
}

#[allow(clippy::too_many_arguments)]
fn render_blocks(
    blocks: &[Block],
    docx: &mut Docx,
    options: &Gb9704Options,
    styles: &Gb9704Styles,
    issues: &mut Vec<Issue>,
    numbers: &mut NumberingState,
    quote: bool,
    template: Gb9704Template,
) {
    for block in blocks {
        if let Determinism::NonDeterministic { reason, impact } = &block.determinism {
            issues.push(report::degraded_content(reason, impact, &block.source));
        }
        match &block.kind {
            BlockKind::Heading {
                level,
                content,
                auto_number,
            } => {
                let (style_id, style) = if quote {
                    (STYLE_QUOTE, styles.run(STYLE_QUOTE))
                } else {
                    heading_style(*level, styles, template)
                };
                if matches!(
                    template,
                    Gb9704Template::General | Gb9704Template::Compilation
                ) && *level == 1
                {
                    numbers.heading_num_id = None;
                    numbers.headings.reset();
                }
                let mut paragraph =
                    styled_paragraph(style_id, content, style, options, styles, issues);
                if !quote
                    && matches!(
                        template,
                        Gb9704Template::General | Gb9704Template::Compilation
                    )
                {
                    paragraph.formatting.outline_level = Some(level.saturating_sub(1).min(8));
                }
                let numbering_requested = match auto_number {
                    Some(value) => *value,
                    None => matches!(template, Gb9704Template::Compilation),
                };
                if numbering_requested {
                    apply_heading_numbering(
                        *level,
                        template,
                        &block.source,
                        &mut paragraph,
                        style,
                        docx,
                        styles,
                        issues,
                        numbers,
                    );
                }
                docx.add_paragraph(paragraph);
            }
            BlockKind::Paragraph { content } => {
                let (style_id, style) = if quote {
                    (STYLE_QUOTE, styles.run(STYLE_QUOTE))
                } else {
                    (STYLE_BODY, styles.run(STYLE_BODY))
                };
                let paragraph = styled_paragraph(style_id, content, style, options, styles, issues);
                docx.add_paragraph(paragraph);
            }
            BlockKind::List(list) => {
                render_list(list, 0, docx, options, styles, issues, &block.source)
            }
            BlockKind::Table(table) => {
                render_table(table, docx, options, styles, issues, &block.source)
            }
            BlockKind::Image(image) => {
                render_image(
                    image,
                    &block.source,
                    docx,
                    options,
                    styles,
                    issues,
                    &mut numbers.figure,
                );
            }
            BlockKind::Quote { blocks } => render_blocks(
                blocks, docx, options, styles, issues, numbers, true, template,
            ),
            BlockKind::Extended { blocks, .. } => render_blocks(
                blocks, docx, options, styles, issues, numbers, quote, template,
            ),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_heading_numbering(
    level: u8,
    template: Gb9704Template,
    source: &SourceSpan,
    paragraph: &mut Paragraph,
    style: &RunConfig,
    docx: &mut Docx,
    styles: &Gb9704Styles,
    issues: &mut Vec<Issue>,
    numbers: &mut NumberingState,
) {
    let offset = match template {
        Gb9704Template::Notice => 0,
        Gb9704Template::General | Gb9704Template::Compilation => 1,
    };
    if level <= offset {
        return;
    }
    let numbering_level = level - offset;
    if !(1..=4).contains(&numbering_level) {
        if styles.heading_numbering().is_some() {
            if !numbers.heading_flattened_reported {
                numbers.heading_flattened_reported = true;
                issues.push(report::heading_numbering_flattened(source));
            }
        } else {
            let marker = numbers.headings.marker(numbering_level);
            if marker.report_static {
                issues.push(report::heading_numbering_static(source));
            }
            if marker.report_flattened {
                issues.push(report::heading_numbering_flattened(source));
            }
        }
        return;
    }

    if let Some(definition) = styles.heading_numbering() {
        let num_id = match numbers.heading_num_id {
            Some(num_id) => num_id,
            None => {
                let instance = heading_instance(definition, numbers.heading_instances);
                let num_id = instance.num_id;
                numbers.heading_instances += 1;
                numbers.heading_num_id = Some(num_id);
                docx.add_numbering(instance);
                num_id
            }
        };
        paragraph.formatting.numbering = Some(NumberingRef {
            num_id,
            level: (numbering_level - 1) as u32,
        });
        if !numbers.heading_word_reported {
            numbers.heading_word_reported = true;
            issues.push(report::heading_numbering_word(source));
        }
    } else {
        let marker = numbers.headings.marker(numbering_level);
        if marker.report_static {
            issues.push(report::heading_numbering_static(source));
        }
        if let Some(text) = marker.text {
            paragraph.runs.insert(0, styled_run(&text, style));
        }
    }
}

fn render_list(
    list: &ListBlock,
    depth: u32,
    docx: &mut Docx,
    options: &Gb9704Options,
    styles: &Gb9704Styles,
    issues: &mut Vec<Issue>,
    source: &SourceSpan,
) {
    if depth == 0 && max_list_depth(list, 0) >= 4 {
        issues.push(report::list_nesting_flattened(source));
    }
    let style = styles.run(STYLE_BODY);
    let start = if list.ordered { list.start } else { 1 };
    for (offset, item) in list.items.iter().enumerate() {
        let number = start.saturating_add(offset as u32);
        if depth == 1 && number > 50 {
            issues.push(report::list_number_format_fallback(number, source));
        }
        let mut paragraph = match list_marker(depth, number) {
            Some(marker) => plain_paragraph(&marker, STYLE_BODY, style),
            None => Paragraph::styled(STYLE_BODY),
        };
        paragraph
            .runs
            .extend(build_runs(&item.content, style, options, styles, issues));
        docx.add_paragraph(paragraph);
        for child in &item.children {
            render_list(child, depth + 1, docx, options, styles, issues, source);
        }
    }
}

fn heading_style(
    level: u8,
    styles: &Gb9704Styles,
    template: Gb9704Template,
) -> (&'static str, &RunConfig) {
    let style_id = match (template, level) {
        (Gb9704Template::General | Gb9704Template::Compilation, 1) => STYLE_TITLE,
        (Gb9704Template::General | Gb9704Template::Compilation, 2) => STYLE_HEADING1,
        (Gb9704Template::General | Gb9704Template::Compilation, 3) => STYLE_HEADING2,
        (Gb9704Template::General | Gb9704Template::Compilation, _) => STYLE_HEADING3,
        (Gb9704Template::Notice, 1) => STYLE_HEADING1,
        (Gb9704Template::Notice, 2) => STYLE_HEADING2,
        (Gb9704Template::Notice, _) => STYLE_HEADING3,
    };
    (style_id, styles.run(style_id))
}

fn heading_instance(base: &NumberingDefinition, index: u32) -> NumberingDefinition {
    let mut instance = base.clone();
    if index > 0 {
        instance.abstract_id += index;
        instance.num_id += index;
    }
    instance
}

fn render_image(
    image: &ImageBlock,
    source: &SourceSpan,
    docx: &mut Docx,
    options: &Gb9704Options,
    styles: &Gb9704Styles,
    issues: &mut Vec<Issue>,
    figure_number: &mut u32,
) {
    let path = resolve_image_path(&image.path, source);
    let display = path.display().to_string();
    let data = match std::fs::read(&path) {
        Ok(data) => data,
        Err(error) => {
            issues.push(report::image_read_failed(
                &display,
                &error.to_string(),
                source,
            ));
            return;
        }
    };
    let embedded = match Image::from_bytes(data) {
        Ok(image) => image,
        Err(error) => {
            issues.push(report::image_decode_failed(
                &display,
                &error.to_string(),
                source,
            ));
            return;
        }
    };
    let config = styles.image();
    let caption = image.caption.as_ref().filter(|caption| !caption.is_empty());
    let alt = image
        .alt
        .clone()
        .or_else(|| image.title.clone())
        .unwrap_or_default();
    let embedded = embedded
        .with_alt(alt)
        .fit_within(config.max_width_mm, config.max_height_mm);
    let index = docx.add_image(embedded);
    let mut paragraph = Paragraph::styled(STYLE_IMAGE);
    paragraph.runs.push(Run::image(index));
    paragraph.formatting.keep_next = caption.is_some();
    docx.add_paragraph(paragraph);

    if let Some(caption) = caption {
        *figure_number += 1;
        if *figure_number == 1 {
            issues.push(report::image_caption_static(source));
        }
        let style = styles.run(STYLE_IMAGE_CAPTION);
        let mut paragraph =
            styled_paragraph(STYLE_IMAGE_CAPTION, caption, style, options, styles, issues);
        paragraph
            .runs
            .insert(0, styled_run(&figure_label(*figure_number), style));
        docx.add_paragraph(paragraph);
    }
}

fn resolve_image_path(path: &str, source: &SourceSpan) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    match Path::new(&source.file).parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(path),
        _ => path.to_path_buf(),
    }
}

fn render_table(
    table: &TableBlock,
    docx: &mut Docx,
    options: &Gb9704Options,
    styles: &Gb9704Styles,
    issues: &mut Vec<Issue>,
    source: &SourceSpan,
) {
    let columns = table.column_count();
    if columns == 0 {
        issues.push(report::empty_table(source));
        return;
    }
    if let Some(caption) = &table.caption {
        docx.add_paragraph(styled_paragraph(
            STYLE_TABLE_CAPTION,
            caption,
            styles.run(STYLE_TABLE_CAPTION),
            options,
            styles,
            issues,
        ));
    }
    let config = styles.table();
    let width = config.width_mm / columns as f32;
    let widths = vec![width; columns];
    let rows = layout_table_rows(table, columns, options, styles, issues, source);
    let mut table = Table::new(widths, rows);
    table.borders = TableBorders::all(config.border);
    table.alignment = Some(config.alignment);
    table.cell_margin_mm = config.cell_margin_mm;
    docx.add_table(table);
}

#[derive(Debug, Clone, Copy)]
struct ActiveMerge {
    start: usize,
    colspan: usize,
    remaining: usize,
}

fn layout_table_rows(
    table: &TableBlock,
    columns: usize,
    options: &Gb9704Options,
    styles: &Gb9704Styles,
    issues: &mut Vec<Issue>,
    source: &SourceSpan,
) -> Vec<TableRow> {
    let mut active: Vec<Option<ActiveMerge>> = vec![None; columns];
    let mut rows = Vec::new();
    let source_rows = table
        .header
        .iter()
        .map(|row| (row, true))
        .chain(table.rows.iter().map(|row| (row, false)));

    for (row_index, (row, header)) in source_rows.enumerate() {
        let active_before = active.clone();
        let mut cells = Vec::new();
        let mut input = row.cells.iter();
        let mut column = 0usize;
        while column < columns {
            if let Some(merge) = active[column] {
                if merge.start == column {
                    let span = merge.colspan.min(columns - column);
                    cells.push(empty_table_cell(
                        header,
                        span as u32,
                        VMerge::Continue,
                        styles,
                    ));
                    column += span;
                } else {
                    column += 1;
                }
                continue;
            }
            let Some(cell) = input.next() else {
                cells.push(empty_table_cell(header, 1, VMerge::None, styles));
                column += 1;
                continue;
            };
            let span = (cell.colspan.max(1) as usize).min(columns - column);
            let rowspan = cell.rowspan.max(1) as usize;
            let mut built =
                content_table_cell(cell, table, column, header, options, styles, issues);
            built.grid_span = span as u32;
            if rowspan > 1 {
                built.v_merge = VMerge::Restart;
                let merge = ActiveMerge {
                    start: column,
                    colspan: span,
                    remaining: rowspan - 1,
                };
                for slot in active.iter_mut().skip(column).take(span) {
                    *slot = Some(merge);
                }
            }
            cells.push(built);
            column += span;
        }
        if input.next().is_some() {
            issues.push(report::table_cells_overflow(row_index, source));
        }
        rows.push(TableRow {
            cells,
            header,
            cant_split: true,
            height_mm: None,
        });
        release_merges(&mut active, &active_before);
    }

    if active.iter().any(Option::is_some) {
        issues.push(report::table_rowspan_truncated(source));
    }
    rows
}

fn release_merges(active: &mut [Option<ActiveMerge>], before: &[Option<ActiveMerge>]) {
    for (start, merge) in before.iter().enumerate() {
        let Some(merge) = merge else {
            continue;
        };
        if merge.start != start {
            continue;
        }
        if merge.remaining <= 1 {
            for slot in active.iter_mut().skip(start).take(merge.colspan) {
                *slot = None;
            }
        } else {
            for current in active.iter_mut().skip(start).take(merge.colspan).flatten() {
                if current.start == start {
                    current.remaining = merge.remaining - 1;
                }
            }
        }
    }
}

fn content_table_cell(
    cell: &IrTableCell,
    table: &TableBlock,
    column: usize,
    header: bool,
    options: &Gb9704Options,
    styles: &Gb9704Styles,
    issues: &mut Vec<Issue>,
) -> TableCell {
    let style_id = if header {
        STYLE_TABLE_HEADER
    } else {
        STYLE_TABLE_TEXT
    };
    let mut paragraph = styled_paragraph(
        style_id,
        &cell.content,
        styles.run(style_id),
        options,
        styles,
        issues,
    );
    let alignment = cell
        .alignment
        .or_else(|| table.column_alignments.get(column).copied());
    if let Some(alignment) = alignment {
        paragraph.formatting.alignment = Some(map_alignment(alignment));
    }
    let mut built = TableCell::new(vec![paragraph]);
    built.vertical_alignment = styles.table().cell_vertical_alignment;
    built
}

fn empty_table_cell(header: bool, span: u32, merge: VMerge, styles: &Gb9704Styles) -> TableCell {
    let style_id = if header {
        STYLE_TABLE_HEADER
    } else {
        STYLE_TABLE_TEXT
    };
    let mut cell = TableCell::new(vec![Paragraph::styled(style_id)])
        .grid_span(span)
        .v_merge(merge);
    cell.vertical_alignment = styles.table().cell_vertical_alignment;
    cell
}

fn map_alignment(alignment: Alignment) -> DocxAlignment {
    match alignment {
        Alignment::Left => DocxAlignment::Left,
        Alignment::Center => DocxAlignment::Center,
        Alignment::Right => DocxAlignment::Right,
        Alignment::Justify => DocxAlignment::Justify,
    }
}

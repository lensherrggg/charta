use crate::model::{
    BodyItem, CoreProperties, Docx, Footer, NumberingDefinition, NumberingLevel, PageSetup,
    Paragraph, Run, RunContent, StyleDefinition, StyleKind, Table, TableCell,
};
use crate::xml::{
    border_xml, color_hex, escape_attr, escape_text, mm_to_twips, paragraph_properties_xml,
    run_properties_xml, XML_DECL,
};
use crate::Result;
use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_PKG_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

const REL_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
const REL_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
const REL_FOOTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
const REL_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

struct ImageRef {
    id: String,
    width_emu: i64,
    height_emu: i64,
    alt: String,
}

pub(crate) fn write(docx: &Docx) -> Result<Vec<u8>> {
    crate::model::validate(docx)?;

    let mut relationships: Vec<(String, String, String)> = vec![(
        "rId1".to_owned(),
        REL_STYLES.to_owned(),
        "styles.xml".to_owned(),
    )];
    let mut next_rel = 2;

    let numbering = if docx.numbering.is_empty() {
        None
    } else {
        let id = format!("rId{next_rel}");
        next_rel += 1;
        relationships.push((
            id.clone(),
            REL_NUMBERING.to_owned(),
            "numbering.xml".to_owned(),
        ));
        Some(numbering_xml(&docx.numbering))
    };

    let mut image_refs = Vec::new();
    let mut media_parts: Vec<(String, Vec<u8>)> = Vec::new();
    let mut image_defaults: BTreeMap<&'static str, &'static str> = BTreeMap::new();
    for (index, image) in docx.images.iter().enumerate() {
        let id = format!("rId{next_rel}");
        next_rel += 1;
        let file = format!("image{}.{}", index + 1, image.format.extension());
        relationships.push((id.clone(), REL_IMAGE.to_owned(), format!("media/{file}")));
        image_defaults.insert(image.format.extension(), image.format.content_type());
        image_refs.push(ImageRef {
            id,
            width_emu: image.width_emu(),
            height_emu: image.height_emu(),
            alt: image.alt.clone(),
        });
        media_parts.push((format!("word/media/{file}"), image.data.clone()));
    }

    let mut footer_parts: Vec<(String, String)> = Vec::new();
    let mut refs = FooterRefs::default();
    if let Some(footer) = &docx.footers.default {
        add_footer(
            footer,
            "default",
            "footer1.xml",
            &mut relationships,
            &mut next_rel,
            &mut footer_parts,
            &mut refs,
            &image_refs,
        );
    }
    if let Some(footer) = &docx.footers.even {
        add_footer(
            footer,
            "even",
            "footer2.xml",
            &mut relationships,
            &mut next_rel,
            &mut footer_parts,
            &mut refs,
            &image_refs,
        );
    }
    if let Some(footer) = &docx.footers.first {
        add_footer(
            footer,
            "first",
            "footer3.xml",
            &mut relationships,
            &mut next_rel,
            &mut footer_parts,
            &mut refs,
            &image_refs,
        );
    }

    let settings_id = format!("rId{next_rel}");
    relationships.push((
        settings_id.clone(),
        REL_SETTINGS.to_owned(),
        "settings.xml".to_owned(),
    ));

    let document = document_xml(docx, &refs, &image_refs);
    let styles = styles_xml(docx);
    let settings = settings_xml(docx.footers.even.is_some());

    let mut content_type_overrides: Vec<(String, String)> = vec![
        (
            "/word/document.xml".to_owned(),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"
                .to_owned(),
        ),
        (
            "/word/styles.xml".to_owned(),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml".to_owned(),
        ),
        (
            "/word/settings.xml".to_owned(),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"
                .to_owned(),
        ),
        (
            "/docProps/core.xml".to_owned(),
            "application/vnd.openxmlformats-package.core-properties+xml".to_owned(),
        ),
        (
            "/docProps/app.xml".to_owned(),
            "application/vnd.openxmlformats-officedocument.extended-properties+xml".to_owned(),
        ),
    ];
    if numbering.is_some() {
        content_type_overrides.push((
            "/word/numbering.xml".to_owned(),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"
                .to_owned(),
        ));
    }
    for (file, _) in &footer_parts {
        content_type_overrides.push((
            format!("/word/{file}"),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml".to_owned(),
        ));
    }

    let image_defaults: Vec<(String, String)> = image_defaults
        .into_iter()
        .map(|(extension, content_type)| (extension.to_owned(), content_type.to_owned()))
        .collect();
    let content_types = content_types_xml(&content_type_overrides, &image_defaults);
    let root_rels = root_rels_xml();
    let document_rels = document_rels_xml(&relationships);

    let mut files: Vec<(String, Vec<u8>)> = vec![
        ("[Content_Types].xml".to_owned(), content_types.into_bytes()),
        ("_rels/.rels".to_owned(), root_rels.into_bytes()),
        ("word/document.xml".to_owned(), document.into_bytes()),
        (
            "word/_rels/document.xml.rels".to_owned(),
            document_rels.into_bytes(),
        ),
        ("word/styles.xml".to_owned(), styles.into_bytes()),
        ("word/settings.xml".to_owned(), settings.into_bytes()),
        (
            "docProps/core.xml".to_owned(),
            core_xml(&docx.core).into_bytes(),
        ),
        ("docProps/app.xml".to_owned(), app_xml().into_bytes()),
    ];
    if let Some(numbering) = numbering {
        files.push(("word/numbering.xml".to_owned(), numbering.into_bytes()));
    }
    for (file, xml) in footer_parts {
        files.push((format!("word/{file}"), xml.into_bytes()));
    }
    files.extend(media_parts);

    zip_package(files)
}

#[derive(Default)]
struct FooterRefs {
    default: Option<String>,
    even: Option<String>,
    first: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn add_footer(
    footer: &Footer,
    kind: &str,
    file: &str,
    relationships: &mut Vec<(String, String, String)>,
    next_rel: &mut usize,
    footer_parts: &mut Vec<(String, String)>,
    refs: &mut FooterRefs,
    image_refs: &[ImageRef],
) {
    let id = format!("rId{next_rel}");
    *next_rel += 1;
    relationships.push((id.clone(), REL_FOOTER.to_owned(), file.to_owned()));
    footer_parts.push((file.to_owned(), footer_xml(footer, image_refs)));
    match kind {
        "default" => refs.default = Some(id),
        "even" => refs.even = Some(id),
        "first" => refs.first = Some(id),
        _ => {}
    }
}

fn document_xml(docx: &Docx, refs: &FooterRefs, image_refs: &[ImageRef]) -> String {
    let mut drawing_id = 0u32;
    let mut body = String::new();
    for item in &docx.body {
        match item {
            BodyItem::Paragraph(paragraph) => {
                body.push_str(&paragraph_xml(paragraph, image_refs, &mut drawing_id));
            }
            BodyItem::Table(table) => {
                body.push_str(&table_xml(table, image_refs, &mut drawing_id));
            }
            BodyItem::PageBreak => {
                body.push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>");
            }
            BodyItem::SectionBreak(section) => {
                let sect = sect_pr_xml(
                    &section.page,
                    refs,
                    Some(if section.continuous {
                        "continuous"
                    } else {
                        "nextPage"
                    }),
                );
                body.push_str(&format!("<w:p><w:pPr>{sect}</w:pPr></w:p>"));
            }
        }
    }
    let final_sect = sect_pr_xml(&docx.page, refs, None);
    format!(
        "{XML_DECL}<w:document xmlns:w=\"{NS_W}\" xmlns:r=\"{NS_R}\"><w:body>{body}{final_sect}</w:body></w:document>"
    )
}

fn sect_pr_xml(page: &PageSetup, refs: &FooterRefs, section_type: Option<&str>) -> String {
    let mut inner = String::new();
    if let Some(id) = &refs.default {
        inner.push_str(&format!(
            "<w:footerReference w:type=\"default\" r:id=\"{}\"/>",
            escape_attr(id)
        ));
    }
    if let Some(id) = &refs.even {
        inner.push_str(&format!(
            "<w:footerReference w:type=\"even\" r:id=\"{}\"/>",
            escape_attr(id)
        ));
    }
    if let Some(id) = &refs.first {
        inner.push_str(&format!(
            "<w:footerReference w:type=\"first\" r:id=\"{}\"/>",
            escape_attr(id)
        ));
    }
    if let Some(section_type) = section_type {
        inner.push_str(&format!(
            "<w:type w:val=\"{}\"/>",
            escape_attr(section_type)
        ));
    }
    inner.push_str(&format!(
        "<w:pgSz w:w=\"{}\" w:h=\"{}\"/>",
        mm_to_twips(page.width_mm),
        mm_to_twips(page.height_mm)
    ));
    inner.push_str(&format!(
        "<w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"{}\" w:footer=\"{}\" w:gutter=\"0\"/>",
        mm_to_twips(page.margin_top_mm),
        mm_to_twips(page.margin_right_mm),
        mm_to_twips(page.margin_bottom_mm),
        mm_to_twips(page.margin_left_mm),
        mm_to_twips(page.header_mm),
        mm_to_twips(page.footer_mm)
    ));
    inner.push_str("<w:cols w:space=\"425\"/>");
    if let Some(alignment) = page.vertical_alignment {
        inner.push_str(&format!("<w:vAlign w:val=\"{}\"/>", alignment.xml_value()));
    }
    if refs.first.is_some() {
        inner.push_str("<w:titlePg/>");
    }
    format!("<w:sectPr>{inner}</w:sectPr>")
}

fn paragraph_xml(paragraph: &Paragraph, image_refs: &[ImageRef], drawing_id: &mut u32) -> String {
    let ppr = paragraph_properties_xml(&paragraph.formatting, true);
    let mut xml = String::from("<w:p>");
    if !ppr.is_empty() {
        xml.push_str(&ppr);
    }
    for run in &paragraph.runs {
        xml.push_str(&run_xml(run, image_refs, drawing_id));
    }
    xml.push_str("</w:p>");
    xml
}

fn run_xml(run: &Run, image_refs: &[ImageRef], drawing_id: &mut u32) -> String {
    let rpr = run_properties_xml(&run.properties);
    match &run.content {
        RunContent::Text(text) => {
            let mut inner = String::new();
            let mut buffer = String::new();
            let flush = |buffer: &mut String, inner: &mut String| {
                if !buffer.is_empty() {
                    inner.push_str(&format!(
                        "<w:t xml:space=\"preserve\">{}</w:t>",
                        escape_text(buffer)
                    ));
                    buffer.clear();
                }
            };
            for ch in text.chars() {
                match ch {
                    '\n' => {
                        flush(&mut buffer, &mut inner);
                        inner.push_str("<w:br/>");
                    }
                    '\t' => {
                        flush(&mut buffer, &mut inner);
                        inner.push_str("<w:tab/>");
                    }
                    _ => buffer.push(ch),
                }
            }
            flush(&mut buffer, &mut inner);
            format!("<w:r>{rpr}{inner}</w:r>")
        }
        RunContent::PageNumber => {
            let field = |content: &str| format!("<w:r>{}{content}</w:r>", rpr);
            let mut xml = String::new();
            xml.push_str(&field("<w:fldChar w:fldCharType=\"begin\"/>"));
            xml.push_str(&field(
                "<w:instrText xml:space=\"preserve\"> PAGE </w:instrText>",
            ));
            xml.push_str(&field("<w:fldChar w:fldCharType=\"separate\"/>"));
            xml.push_str(&field("<w:t>1</w:t>"));
            xml.push_str(&field("<w:fldChar w:fldCharType=\"end\"/>"));
            xml
        }
        RunContent::Image(index) => {
            let Some(image) = image_refs.get(*index) else {
                return String::new();
            };
            *drawing_id += 1;
            format!("<w:r>{rpr}{}</w:r>", image_drawing_xml(image, *drawing_id))
        }
    }
}

fn image_drawing_xml(image: &ImageRef, drawing_id: u32) -> String {
    let name = format!("Picture {drawing_id}");
    let name = escape_attr(&name);
    let alt = escape_attr(&image.alt);
    format!(
        "<w:drawing><wp:inline xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{width}\" cy=\"{height}\"/><wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/><wp:docPr id=\"{id}\" name=\"{name}\" descr=\"{alt}\"/><wp:cNvGraphicFramePr><a:graphicFrameLocks xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" noChangeAspect=\"1\"/></wp:cNvGraphicFramePr><a:graphic xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:nvPicPr><pic:cNvPr id=\"{id}\" name=\"{name}\" descr=\"{alt}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"{rel}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{width}\" cy=\"{height}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>",
        width = image.width_emu,
        height = image.height_emu,
        id = drawing_id,
        rel = escape_attr(&image.id),
    )
}

fn table_xml(table: &Table, image_refs: &[ImageRef], drawing_id: &mut u32) -> String {
    let widths: Vec<i32> = table
        .columns
        .iter()
        .map(|width| mm_to_twips(*width))
        .collect();
    let total_width = table
        .width_mm
        .map(mm_to_twips)
        .unwrap_or_else(|| widths.iter().sum());

    let mut xml = String::from("<w:tbl>");
    xml.push_str("<w:tblPr>");
    xml.push_str(&format!("<w:tblW w:w=\"{total_width}\" w:type=\"dxa\"/>"));
    if let Some(alignment) = table.alignment {
        xml.push_str(&format!("<w:jc w:val=\"{}\"/>", alignment.xml_value()));
    }
    let borders = table_borders_xml(table);
    if !borders.is_empty() {
        xml.push_str(&borders);
    }
    xml.push_str("<w:tblLayout w:type=\"fixed\"/>");
    let margin = mm_to_twips(table.cell_margin_mm);
    xml.push_str(&format!(
        "<w:tblCellMar><w:top w:w=\"{margin}\" w:type=\"dxa\"/><w:left w:w=\"{margin}\" w:type=\"dxa\"/><w:bottom w:w=\"{margin}\" w:type=\"dxa\"/><w:right w:w=\"{margin}\" w:type=\"dxa\"/></w:tblCellMar>"
    ));
    xml.push_str("</w:tblPr><w:tblGrid>");
    for width in &widths {
        xml.push_str(&format!("<w:gridCol w:w=\"{width}\"/>"));
    }
    xml.push_str("</w:tblGrid>");

    for row in &table.rows {
        xml.push_str("<w:tr><w:trPr>");
        if row.cant_split {
            xml.push_str("<w:cantSplit/>");
        }
        if let Some(height) = row.height_mm {
            xml.push_str(&format!(
                "<w:trHeight w:val=\"{}\" w:hRule=\"atLeast\"/>",
                mm_to_twips(height)
            ));
        }
        if row.header {
            xml.push_str("<w:tblHeader/>");
        }
        xml.push_str("</w:trPr>");
        let mut grid_index = 0usize;
        for cell in &row.cells {
            let span = cell.grid_span.max(1) as usize;
            let start = grid_index.min(widths.len());
            let end = (grid_index + span).min(widths.len());
            let width: i32 = widths[start..end].iter().sum();
            xml.push_str(&table_cell_xml(cell, width, image_refs, drawing_id));
            grid_index += span;
        }
        xml.push_str("</w:tr>");
    }
    xml.push_str("</w:tbl>");
    xml
}

fn table_borders_xml(table: &Table) -> String {
    let mut inner = String::new();
    if let Some(border) = &table.borders.top {
        inner.push_str(&border_xml("top", border));
    }
    if let Some(border) = &table.borders.left {
        inner.push_str(&border_xml("left", border));
    }
    if let Some(border) = &table.borders.bottom {
        inner.push_str(&border_xml("bottom", border));
    }
    if let Some(border) = &table.borders.right {
        inner.push_str(&border_xml("right", border));
    }
    if let Some(border) = &table.borders.inside_horizontal {
        inner.push_str(&border_xml("insideH", border));
    }
    if let Some(border) = &table.borders.inside_vertical {
        inner.push_str(&border_xml("insideV", border));
    }
    if inner.is_empty() {
        String::new()
    } else {
        format!("<w:tblBorders>{inner}</w:tblBorders>")
    }
}

fn table_cell_xml(
    cell: &TableCell,
    width: i32,
    image_refs: &[ImageRef],
    drawing_id: &mut u32,
) -> String {
    let mut xml = String::from("<w:tc><w:tcPr>");
    xml.push_str(&format!("<w:tcW w:w=\"{width}\" w:type=\"dxa\"/>"));
    if cell.grid_span > 1 {
        xml.push_str(&format!(
            "<w:gridSpan w:val=\"{}\"/>",
            cell.grid_span.max(1)
        ));
    }
    if let Some(value) = cell.v_merge.xml_value() {
        xml.push_str(&format!("<w:vMerge w:val=\"{value}\"/>"));
    }
    if let Some(shading) = cell.shading {
        xml.push_str(&format!(
            "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>",
            color_hex(shading)
        ));
    }
    xml.push_str(&format!(
        "<w:vAlign w:val=\"{}\"/>",
        cell.vertical_alignment.xml_value()
    ));
    xml.push_str("</w:tcPr>");
    if cell.paragraphs.is_empty() {
        xml.push_str("<w:p></w:p>");
    } else {
        for paragraph in &cell.paragraphs {
            xml.push_str(&paragraph_xml(paragraph, image_refs, drawing_id));
        }
    }
    xml.push_str("</w:tc>");
    xml
}

fn styles_xml(docx: &Docx) -> String {
    let mut xml = format!("{XML_DECL}<w:styles xmlns:w=\"{NS_W}\">");
    let stylesheet = &docx.styles;
    if stylesheet.default_font.is_some()
        || stylesheet.default_east_asia_font.is_some()
        || stylesheet.default_size_half_points.is_some()
    {
        xml.push_str("<w:docDefaults><w:rPrDefault><w:rPr>");
        let mut fonts = String::new();
        if let Some(font) = &stylesheet.default_font {
            fonts.push_str(&format!(
                " w:ascii=\"{}\" w:hAnsi=\"{}\"",
                escape_attr(font),
                escape_attr(font)
            ));
        }
        if let Some(font) = &stylesheet.default_east_asia_font {
            fonts.push_str(&format!(" w:eastAsia=\"{}\"", escape_attr(font)));
        }
        if !fonts.is_empty() {
            xml.push_str(&format!("<w:rFonts{fonts}/>"));
        }
        if let Some(size) = stylesheet.default_size_half_points {
            xml.push_str(&format!(
                "<w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/>"
            ));
        }
        xml.push_str("</w:rPr></w:rPrDefault>");
        xml.push_str("<w:pPrDefault><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault>");
        xml.push_str("</w:docDefaults>");
    }

    let normal = stylesheet.styles.iter().find(|style| style.id == "Normal");
    if let Some(style) = normal {
        xml.push_str(&style_xml(style, true));
    } else {
        xml.push_str("<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:qFormat/></w:style>");
    }
    for style in &stylesheet.styles {
        if style.id == "Normal" {
            continue;
        }
        xml.push_str(&style_xml(style, false));
    }
    xml.push_str("</w:styles>");
    xml
}

fn style_xml(style: &StyleDefinition, default: bool) -> String {
    let mut xml = format!(
        "<w:style w:type=\"{}\" w:styleId=\"{}\"{}>",
        style.kind.xml_value(),
        escape_attr(&style.id),
        if default { " w:default=\"1\"" } else { "" }
    );
    xml.push_str(&format!("<w:name w:val=\"{}\"/>", escape_attr(&style.name)));
    if let Some(based_on) = &style.based_on {
        xml.push_str(&format!("<w:basedOn w:val=\"{}\"/>", escape_attr(based_on)));
    }
    if let Some(next) = &style.next {
        xml.push_str(&format!("<w:next w:val=\"{}\"/>", escape_attr(next)));
    }
    xml.push_str("<w:qFormat/>");
    if style.kind == StyleKind::Paragraph {
        xml.push_str(&paragraph_properties_xml(&style.formatting, false));
    }
    xml.push_str(&run_properties_xml(&style.run));
    xml.push_str("</w:style>");
    xml
}

fn numbering_indent(level: &NumberingLevel) -> String {
    let mut attrs = format!("w:left=\"{}\"", level.indent_left_twips);
    if let Some(first_line) = level.first_line_twips {
        attrs.push_str(&format!(" w:firstLine=\"{first_line}\""));
    } else {
        attrs.push_str(&format!(" w:hanging=\"{}\"", level.hanging_twips));
    }
    attrs
}

fn numbering_xml(definitions: &[NumberingDefinition]) -> String {
    let mut xml = format!("{XML_DECL}<w:numbering xmlns:w=\"{NS_W}\">");
    for definition in definitions {
        xml.push_str(&format!(
            "<w:abstractNum w:abstractNumId=\"{}\"><w:multiLevelType w:val=\"{}\"/>",
            definition.abstract_id,
            definition.multi_level_type.xml_value()
        ));
        for level in &definition.levels {
            xml.push_str(&format!(
                "<w:lvl w:ilvl=\"{}\"><w:start w:val=\"{}\"/><w:numFmt w:val=\"{}\"/><w:suff w:val=\"{}\"/><w:lvlText w:val=\"{}\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind {}/></w:pPr>",
                level.level,
                level.start,
                level.format.xml_value(),
                level.suffix.xml_value(),
                escape_attr(&level.text),
                numbering_indent(level)
            ));
            if let Some(font) = &level.font {
                xml.push_str(&format!(
                    "<w:rPr><w:rFonts w:ascii=\"{}\" w:hAnsi=\"{}\" w:eastAsia=\"{}\"/></w:rPr>",
                    escape_attr(font),
                    escape_attr(font),
                    escape_attr(font)
                ));
            }
            xml.push_str("</w:lvl>");
        }
        xml.push_str("</w:abstractNum>");
    }
    for definition in definitions {
        xml.push_str(&format!(
            "<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"{}\"/></w:num>",
            definition.num_id, definition.abstract_id
        ));
    }
    xml.push_str("</w:numbering>");
    xml
}

fn footer_xml(footer: &Footer, image_refs: &[ImageRef]) -> String {
    let mut drawing_id = 0u32;
    let mut xml = format!("{XML_DECL}<w:ftr xmlns:w=\"{NS_W}\" xmlns:r=\"{NS_R}\">");
    if footer.paragraphs.is_empty() {
        xml.push_str("<w:p></w:p>");
    } else {
        for paragraph in &footer.paragraphs {
            xml.push_str(&paragraph_xml(paragraph, image_refs, &mut drawing_id));
        }
    }
    xml.push_str("</w:ftr>");
    xml
}

fn settings_xml(even_and_odd: bool) -> String {
    let mut xml = format!("{XML_DECL}<w:settings xmlns:w=\"{NS_W}\">");
    xml.push_str("<w:zoom w:percent=\"100\"/>");
    if even_and_odd {
        xml.push_str("<w:evenAndOddHeaders/>");
    }
    xml.push_str(
        "<w:defaultTabStop w:val=\"420\"/><w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/></w:compat><w:themeFontLang w:val=\"en-US\" w:eastAsia=\"zh-CN\"/></w:settings>",
    );
    xml
}

fn core_xml(core: &CoreProperties) -> String {
    let mut xml = format!(
        "{XML_DECL}<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">"
    );
    if let Some(title) = &core.title {
        xml.push_str(&format!("<dc:title>{}</dc:title>", escape_text(title)));
    }
    if let Some(creator) = &core.creator {
        xml.push_str(&format!(
            "<dc:creator>{}</dc:creator>",
            escape_text(creator)
        ));
        xml.push_str(&format!(
            "<cp:lastModifiedBy>{}</cp:lastModifiedBy>",
            escape_text(creator)
        ));
    }
    if let Some(subject) = &core.subject {
        xml.push_str(&format!(
            "<dc:subject>{}</dc:subject>",
            escape_text(subject)
        ));
    }
    if let Some(created) = &core.created {
        xml.push_str(&format!(
            "<dcterms:created xsi:type=\"dcterms:W3CDTF\">{}</dcterms:created>",
            escape_text(created)
        ));
    }
    xml.push_str("</cp:coreProperties>");
    xml
}

fn app_xml() -> String {
    format!(
        "{XML_DECL}<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\" xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\"><Application>Charta</Application><AppVersion>{}</AppVersion><DocSecurity>0</DocSecurity><ScaleCrop>false</ScaleCrop></Properties>",
        env!("CARGO_PKG_VERSION")
    )
}

fn content_types_xml(overrides: &[(String, String)], defaults: &[(String, String)]) -> String {
    let mut xml = format!(
        "{XML_DECL}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/>"
    );
    for (extension, content_type) in defaults {
        xml.push_str(&format!(
            "<Default Extension=\"{}\" ContentType=\"{}\"/>",
            escape_attr(extension),
            escape_attr(content_type)
        ));
    }
    for (part, content_type) in overrides {
        xml.push_str(&format!(
            "<Override PartName=\"{}\" ContentType=\"{}\"/>",
            escape_attr(part),
            escape_attr(content_type)
        ));
    }
    xml.push_str("</Types>");
    xml
}

fn root_rels_xml() -> String {
    format!(
        "{XML_DECL}<Relationships xmlns=\"{NS_PKG_REL}\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/><Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties\" Target=\"docProps/app.xml\"/></Relationships>"
    )
}

fn document_rels_xml(relationships: &[(String, String, String)]) -> String {
    let mut xml = format!("{XML_DECL}<Relationships xmlns=\"{NS_PKG_REL}\">");
    for (id, kind, target) in relationships {
        xml.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"/>",
            escape_attr(id),
            escape_attr(kind),
            escape_attr(target)
        ));
    }
    xml.push_str("</Relationships>");
    xml
}

fn zip_package(files: Vec<(String, Vec<u8>)>) -> Result<Vec<u8>> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    for (name, contents) in files {
        writer.start_file(name, options)?;
        writer.write_all(&contents)?;
    }
    let cursor = writer.finish()?;
    Ok(cursor.into_inner())
}

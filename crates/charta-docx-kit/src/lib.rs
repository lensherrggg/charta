//! Mechanical OOXML helpers for Charta backends.
//!
//! The kit intentionally contains no document-standard semantics. It builds
//! `.docx` packages from zero and lets a backend bypass any helper.
//!
//! ```
//! use charta_docx_kit::{Docx, Paragraph, Run, RunProperties};
//!
//! let mut docx = Docx::new();
//! docx.add_paragraph(
//!     Paragraph::text("Hello")
//!         .run(Run::text(" world").properties(RunProperties::default().bold(true))),
//! );
//! let bytes = docx.to_bytes().unwrap();
//! assert!(!bytes.is_empty());
//! ```

mod image;
mod model;
mod writer;
mod xml;

pub use image::{Image, ImageFormat};
pub use model::*;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("ZIP 打包错误: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("docx 结构不合法: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Docx {
    /// Build the complete `.docx` package in memory.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        writer::write(self)
    }

    pub fn write(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        std::fs::write(path, self.to_bytes()?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn read_part(bytes: &[u8], name: &str) -> String {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut file = archive.by_name(name).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        contents
    }

    fn names(bytes: &[u8]) -> Vec<String> {
        let archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        archive.file_names().map(str::to_owned).collect()
    }

    #[test]
    fn writes_native_multilevel_heading_numbering() {
        let mut docx = Docx::new();
        docx.add_numbering(
            NumberingDefinition::new(
                1000,
                1000,
                vec![NumberingLevel {
                    level: 0,
                    format: NumberFormat::ChineseCounting,
                    text: "%1、".to_owned(),
                    start: 1,
                    indent_left_twips: 0,
                    hanging_twips: 0,
                    first_line_twips: Some(640),
                    suffix: NumberingSuffix::Nothing,
                    font: None,
                }],
            )
            .multi_level_type(MultiLevelType::Multilevel),
        );
        let mut paragraph = Paragraph::text("标题");
        paragraph.formatting.numbering = Some(NumberingRef {
            num_id: 1000,
            level: 0,
        });
        docx.add_paragraph(paragraph);

        let bytes = docx.to_bytes().unwrap();
        let numbering = read_part(&bytes, "word/numbering.xml");
        assert!(numbering.contains("w:multiLevelType w:val=\"multilevel\""));
        assert!(numbering.contains("w:numFmt w:val=\"chineseCounting\""));
        assert!(numbering.contains("w:suff w:val=\"nothing\""));
        assert!(numbering.contains("w:lvlText w:val=\"%1、\""));
        assert!(numbering.contains("w:firstLine=\"640\""));
        assert!(!numbering.contains("w:hanging="));

        let body = read_part(&bytes, "word/document.xml");
        assert!(body.contains("<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"1000\"/></w:numPr>"));
    }

    #[test]
    fn writes_a_complete_package() {
        let mut docx = Docx::new();
        docx.styles.default_font = Some("Times New Roman".to_owned());
        docx.styles.default_east_asia_font = Some("仿宋_GB2312".to_owned());
        docx.styles.default_size_half_points = Some(32);
        docx.add_style(
            StyleDefinition::paragraph("ChartaBody", "Charta Body")
                .run(
                    RunProperties::default()
                        .font_east_asia("仿宋_GB2312")
                        .size_pt(16.0),
                )
                .formatting(ParagraphFormatting {
                    spacing: Spacing::exact(28.0),
                    indent: Indent {
                        first_line_chars: Some(2.0),
                        first_line_twips: Some(640),
                        ..Indent::default()
                    },
                    ..ParagraphFormatting::default()
                }),
        );
        docx.add_paragraph(
            Paragraph::new().style("ChartaBody").run(
                Run::text("测试正文").properties(
                    RunProperties::default()
                        .font_east_asia("仿宋_GB2312")
                        .size_pt(16.0),
                ),
            ),
        );
        docx.add_table(Table::new(
            vec![40.0, 40.0],
            vec![TableRow::new(vec![
                TableCell::text("甲"),
                TableCell::text("乙"),
            ])],
        ));
        docx.footers.default = Some(Footer::new(vec![Paragraph::text("— ")
            .align(Alignment::Center)
            .run(Run::page_number())
            .run(Run::text(" —"))]));
        docx.footers.even = Some(Footer::new(vec![Paragraph::text("— ")
            .align(Alignment::Center)
            .run(Run::page_number())
            .run(Run::text(" —"))]));

        let bytes = docx.to_bytes().unwrap();
        let files = names(&bytes);
        assert!(files.contains(&"[Content_Types].xml".to_owned()));
        assert!(files.contains(&"word/document.xml".to_owned()));
        assert!(files.contains(&"word/styles.xml".to_owned()));
        assert!(files.contains(&"word/footer1.xml".to_owned()));
        assert!(files.contains(&"word/footer2.xml".to_owned()));

        let document = read_part(&bytes, "word/document.xml");
        assert!(document.contains("测试正文"));
        assert!(document.contains("<w:tbl>"));
        assert!(!document.contains("w:tblHeader"));

        let footer = read_part(&bytes, "word/footer1.xml");
        assert!(footer.contains("PAGE"));
        assert!(footer.contains("fldCharType=\"begin\""));

        let settings = read_part(&bytes, "word/settings.xml");
        assert!(settings.contains("evenAndOddHeaders"));

        let styles = read_part(&bytes, "word/styles.xml");
        assert!(styles.contains("ChartaBody"));
        assert!(styles.contains("仿宋_GB2312"));
        assert!(styles.contains("w:line=\"560\""));
        assert!(styles.contains("w:firstLineChars=\"200\""));
    }

    #[test]
    fn rejects_tables_without_columns() {
        let mut docx = Docx::new();
        docx.add_table(Table::new(Vec::new(), Vec::new()));
        let error = docx.to_bytes().unwrap_err();
        assert!(error.to_string().contains("列宽"));
    }

    #[test]
    fn rejects_rows_wider_than_columns() {
        let mut docx = Docx::new();
        docx.add_table(Table::new(
            vec![40.0, 40.0],
            vec![TableRow::new(vec![TableCell::text("宽").grid_span(3)])],
        ));
        let error = docx.to_bytes().unwrap_err();
        assert!(error.to_string().contains("网格跨度"));
    }

    #[test]
    fn writes_merged_cells() {
        let mut docx = Docx::new();
        let table = Table::new(
            vec![40.0, 40.0, 40.0],
            vec![
                TableRow::new(vec![
                    TableCell::text("合并")
                        .grid_span(2)
                        .v_merge(VMerge::Restart),
                    TableCell::text("右"),
                ]),
                TableRow::new(vec![
                    TableCell::new(Vec::new())
                        .grid_span(2)
                        .v_merge(VMerge::Continue),
                    TableCell::text("尾"),
                ]),
            ],
        );
        docx.add_table(table);

        let document = read_part(&docx.to_bytes().unwrap(), "word/document.xml");
        assert!(document.contains("<w:gridSpan w:val=\"2\"/>"));
        assert!(document.contains("<w:vMerge w:val=\"restart\"/>"));
        assert!(document.contains("<w:vMerge w:val=\"continue\"/>"));
    }

    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0xFC,
        0xCF, 0xC0, 0x50, 0x0F, 0x00, 0x05, 0x83, 0x02, 0x7F, 0x94, 0xAD, 0xD0, 0xEB, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn writes_image_parts_and_drawing() {
        let image = Image::from_bytes(PNG_1X1.to_vec())
            .unwrap()
            .with_alt("示意图");
        assert_eq!(image.format(), ImageFormat::Png);
        let mut docx = Docx::new();
        let index = docx.add_image(image.fit_within(100.0, 100.0));
        let mut paragraph = Paragraph::new();
        paragraph.formatting.alignment = Some(Alignment::Center);
        paragraph.runs.push(Run::image(index));
        docx.add_paragraph(paragraph);

        let bytes = docx.to_bytes().unwrap();
        let files = names(&bytes);
        assert!(files.contains(&"word/media/image1.png".to_owned()));
        let content_types = read_part(&bytes, "[Content_Types].xml");
        assert!(content_types.contains("Extension=\"png\""));
        assert!(content_types.contains("image/png"));
        let rels = read_part(&bytes, "word/_rels/document.xml.rels");
        assert!(rels.contains("relationships/image"));
        assert!(rels.contains("media/image1.png"));
        let document = read_part(&bytes, "word/document.xml");
        assert!(document.contains("<w:drawing>"));
        assert!(document.contains("r:embed="));
        assert!(document.contains("descr=\"示意图\""));
    }

    #[test]
    fn rejects_unknown_image_reference() {
        let mut docx = Docx::new();
        let mut paragraph = Paragraph::new();
        paragraph.runs.push(Run::image(0));
        docx.add_paragraph(paragraph);
        let error = docx.to_bytes().unwrap_err();
        assert!(error.to_string().contains("图片引用"));
    }

    #[test]
    fn writes_red_separator_and_page_setup() {
        let mut docx = Docx::new();
        docx.page.margin_top_mm = 37.0;
        docx.page.vertical_alignment = Some(PageVerticalAlignment::Bottom);
        let mut paragraph = Paragraph::new();
        paragraph.formatting.borders.bottom = Some(Border::single(12, [0xFF, 0x00, 0x00]));
        docx.add_paragraph(paragraph);
        let bytes = docx.to_bytes().unwrap();
        let document = read_part(&bytes, "word/document.xml");
        assert!(document.contains("w:color=\"FF0000\""));
        assert!(document.contains("w:top=\"2098\""));
        assert!(document.contains("<w:vAlign w:val=\"bottom\"/>"));
    }
}

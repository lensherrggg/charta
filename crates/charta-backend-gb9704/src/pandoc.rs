//! Build a pandoc `reference.docx` from the backend style data.
//!
//! Pandoc looks styles up by Word's built-in style IDs (`BodyText`,
//! `Heading1`, ...). Charta's style IDs (`GbBody`, `GbHeading1`, ...) carry
//! document-standard meaning, so the reference document renames them while
//! keeping every value from the style data. Styles with no pandoc counterpart
//! are kept as custom styles.

use std::collections::HashMap;

use charta_docx_kit::{Docx, Paragraph, Stylesheet};

use crate::styles::Gb9704Styles;

/// Existing styles that map onto a pandoc paragraph role.
const MAPPED: &[(&str, &str, &str)] = &[
    ("GbTitle", "Title", "Title"),
    ("GbByline", "Author", "Author"),
    ("GbDate", "Date", "Date"),
    ("GbBody", "BodyText", "Body Text"),
    ("GbHeading1", "Heading1", "heading 1"),
    ("GbHeading2", "Heading2", "heading 2"),
    ("GbHeading3", "Heading3", "heading 3"),
    ("GbQuote", "BlockText", "Block Text"),
    ("GbTableCaption", "TableCaption", "Table Caption"),
    ("GbImageCaption", "ImageCaption", "Image Caption"),
    ("TocTitle", "TOCHeading", "TOC Heading"),
];

/// Pandoc roles that need a style even when the backend has none. Values are
/// copied from the source style listed first.
const DERIVED: &[(&str, &str, &str)] = &[
    ("BodyText", "FirstParagraph", "First Paragraph"),
    ("BodyText", "Compact", "Compact"),
    ("Author", "Date", "Date"),
    ("Heading3", "Heading4", "heading 4"),
    ("Heading3", "Heading5", "heading 5"),
    ("Heading3", "Heading6", "heading 6"),
    ("Heading3", "Heading7", "heading 7"),
    ("Heading3", "Heading8", "heading 8"),
    ("Heading3", "Heading9", "heading 9"),
];

impl Gb9704Styles {
    /// Build a pandoc `reference.docx` from these styles.
    pub fn pandoc_reference_docx(&self) -> charta::Result<Vec<u8>> {
        let mut docx = Docx::new();
        docx.page = self.page().setup();
        docx.styles = self.pandoc_stylesheet();
        docx.core.title = Some("Charta pandoc reference".to_owned());
        docx.add_paragraph(Paragraph::new());
        docx.to_bytes()
            .map_err(|error| charta::Error::InvalidArgument(error.to_string()))
    }

    /// Names of the styles renamed for pandoc, as `source -> target`.
    pub fn pandoc_style_mapping(&self) -> Vec<(String, String)> {
        let sheet = self.stylesheet();
        MAPPED
            .iter()
            .filter(|(source, _, _)| sheet.styles.iter().any(|style| style.id == *source))
            .map(|(source, target, _)| ((*source).to_owned(), (*target).to_owned()))
            .collect()
    }

    fn pandoc_stylesheet(&self) -> Stylesheet {
        let mut sheet = self.stylesheet();
        let mut renamed = HashMap::new();
        for style in &mut sheet.styles {
            let Some((_, target, name)) = MAPPED.iter().find(|(source, _, _)| style.id == *source)
            else {
                continue;
            };
            renamed.insert(style.id.clone(), (*target).to_owned());
            style.id = (*target).to_owned();
            style.name = (*name).to_owned();
        }
        for style in &mut sheet.styles {
            if let Some(base) = &style.based_on {
                if let Some(target) = renamed.get(base) {
                    style.based_on = Some(target.clone());
                }
            }
            if let Some(next) = &style.next {
                if let Some(target) = renamed.get(next) {
                    style.next = Some(target.clone());
                }
            }
        }

        for (source, target, name) in DERIVED {
            if sheet.styles.iter().any(|style| style.id == *target) {
                continue;
            }
            let Some(mut style) = sheet
                .styles
                .iter()
                .find(|style| style.id == *source)
                .cloned()
            else {
                continue;
            };
            style.id = (*target).to_owned();
            style.name = (*name).to_owned();
            style.based_on = Some((*source).to_owned());
            style.next = None;
            if *target == "Compact" {
                style.formatting.indent = charta_docx_kit::Indent::default();
            }
            sheet.styles.push(style);
        }
        sheet
    }
}

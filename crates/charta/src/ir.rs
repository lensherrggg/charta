//! Semantic intermediate representation.
//!
//! The IR is the only contract between a frontend and a backend. It carries
//! structure and *style intents*, never document-standard semantics.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Version of the IR wire format. Bumped independently from the crate version.
pub const IR_VERSION: &str = "1.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontendId {
    Markdown,
    Typst,
}

impl FrontendId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Typst => "typst",
        }
    }
}

/// A position in a source file. Lines and columns are 1-based; columns count
/// Unicode scalar values, while byte offsets index the UTF-8 source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub byte_start: usize,
    pub byte_end: usize,
}

impl SourceSpan {
    pub fn new(
        file: impl Into<String>,
        line: usize,
        column: usize,
        end_line: usize,
        end_column: usize,
        byte_start: usize,
        byte_end: usize,
    ) -> Self {
        Self {
            file: file.into(),
            line,
            column,
            end_line,
            end_column,
            byte_start,
            byte_end,
        }
    }

    pub fn point(file: impl Into<String>, line: usize, column: usize, byte: usize) -> Self {
        Self::new(file, line, column, line, column, byte, byte)
    }

    pub fn document(file: impl Into<String>) -> Self {
        Self::point(file, 1, 1, 0)
    }

    pub fn display(&self) -> String {
        format!("{}:{}:{}", self.file, self.line, self.column)
    }
}

/// Static determinism of an IR node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Determinism {
    Deterministic,
    NonDeterministic { reason: String, impact: String },
}

impl Determinism {
    pub fn is_deterministic(&self) -> bool {
        matches!(self, Self::Deterministic)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetaValue {
    #[default]
    Null,
    Bool(bool),
    Integer(i64),
    Number(f64),
    String(String),
    List(Vec<MetaValue>),
    Map(BTreeMap<String, MetaValue>),
}

impl MetaValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::Integer(_) => "integer",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::List(_) => "list",
            Self::Map(_) => "map",
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[MetaValue]> {
        match self {
            Self::List(values) => Some(values),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&BTreeMap<String, MetaValue>> {
        match self {
            Self::Map(values) => Some(values),
            _ => None,
        }
    }
}

impl From<String> for MetaValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for MetaValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

/// Document-level metadata as parsed from the frontend. Field semantics belong
/// exclusively to the backend schema; the engine only stores and forwards it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    pub fields: BTreeMap<String, MetaValue>,
}

impl Metadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&MetaValue> {
        self.fields.get(key)
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<MetaValue>) {
        self.fields.insert(key.into(), value.into());
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &MetaValue)> {
        self.fields.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub ir_version: String,
    pub frontend: FrontendId,
    pub metadata: Metadata,
    pub blocks: Vec<Block>,
    /// Extended structure roles declared at document level.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declared_roles: Vec<RoleDeclaration>,
}

impl Document {
    pub fn new(frontend: FrontendId, metadata: Metadata) -> Self {
        Self {
            ir_version: IR_VERSION.to_owned(),
            frontend,
            metadata,
            blocks: Vec::new(),
            declared_roles: Vec::new(),
        }
    }

    pub fn with_blocks(mut self, blocks: Vec<Block>) -> Self {
        self.blocks = blocks;
        self
    }

    pub fn has_non_deterministic_content(&self) -> bool {
        self.blocks.iter().any(Block::has_non_deterministic_content)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoleDeclaration {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    #[serde(flatten)]
    pub kind: BlockKind,
    pub source: SourceSpan,
    pub determinism: Determinism,
}

impl Block {
    fn new(kind: BlockKind, source: SourceSpan) -> Self {
        Self {
            kind,
            source,
            determinism: Determinism::Deterministic,
        }
    }

    pub fn heading(level: u8, content: Vec<Inline>, source: SourceSpan) -> Self {
        Self::new(
            BlockKind::Heading {
                level,
                content,
                auto_number: None,
            },
            source,
        )
    }

    /// Sets the frontend's auto-numbering suggestion for a heading. No-op for
    /// other block kinds.
    pub fn with_auto_number(mut self, auto_number: bool) -> Self {
        if let BlockKind::Heading {
            auto_number: suggestion,
            ..
        } = &mut self.kind
        {
            *suggestion = Some(auto_number);
        }
        self
    }

    pub fn paragraph(content: Vec<Inline>, source: SourceSpan) -> Self {
        Self::new(BlockKind::Paragraph { content }, source)
    }

    pub fn list(list: ListBlock, source: SourceSpan) -> Self {
        Self::new(BlockKind::List(list), source)
    }

    pub fn table(table: TableBlock, source: SourceSpan) -> Self {
        Self::new(BlockKind::Table(table), source)
    }

    pub fn image(image: ImageBlock, source: SourceSpan) -> Self {
        Self::new(BlockKind::Image(image), source)
    }

    pub fn quote(blocks: Vec<Block>, source: SourceSpan) -> Self {
        Self::new(BlockKind::Quote { blocks }, source)
    }

    pub fn extended(role: impl Into<String>, blocks: Vec<Block>, source: SourceSpan) -> Self {
        Self::new(
            BlockKind::Extended {
                role: role.into(),
                blocks,
            },
            source,
        )
    }

    pub fn with_determinism(mut self, determinism: Determinism) -> Self {
        self.determinism = determinism;
        self
    }

    pub fn has_non_deterministic_content(&self) -> bool {
        if !self.determinism.is_deterministic() {
            return true;
        }
        match &self.kind {
            BlockKind::Quote { blocks } | BlockKind::Extended { blocks, .. } => {
                blocks.iter().any(Self::has_non_deterministic_content)
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockKind {
    Heading {
        level: u8,
        content: Vec<Inline>,
        /// Frontend suggestion: generate numbering for this heading. `None`
        /// means no suggestion; the backend decides whether to number.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        auto_number: Option<bool>,
    },
    Paragraph {
        content: Vec<Inline>,
    },
    List(ListBlock),
    Table(TableBlock),
    Image(ImageBlock),
    Quote {
        blocks: Vec<Block>,
    },
    /// A role declared in document metadata. Backends that do not know the role
    /// should render the contained blocks transparently.
    Extended {
        role: String,
        blocks: Vec<Block>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListBlock {
    pub ordered: bool,
    #[serde(default = "default_list_start")]
    pub start: u32,
    pub items: Vec<ListItem>,
}

fn default_list_start() -> u32 {
    1
}

impl ListBlock {
    pub fn unordered(items: Vec<ListItem>) -> Self {
        Self {
            ordered: false,
            start: 1,
            items,
        }
    }

    pub fn ordered(start: u32, items: Vec<ListItem>) -> Self {
        Self {
            ordered: true,
            start,
            items,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListItem {
    pub content: Vec<Inline>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ListBlock>,
}

impl ListItem {
    pub fn new(content: Vec<Inline>) -> Self {
        Self {
            content,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableBlock {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub header: Vec<TableRow>,
    #[serde(default)]
    pub rows: Vec<TableRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_alignments: Vec<Alignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<Vec<Inline>>,
}

impl TableBlock {
    pub fn new(header: Vec<TableRow>, rows: Vec<TableRow>) -> Self {
        Self {
            header,
            rows,
            column_alignments: Vec::new(),
            caption: None,
        }
    }

    pub fn column_count(&self) -> usize {
        self.header
            .iter()
            .chain(self.rows.iter())
            .map(TableRow::column_count)
            .max()
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
}

impl TableRow {
    pub fn new(cells: Vec<TableCell>) -> Self {
        Self { cells }
    }

    pub fn column_count(&self) -> usize {
        self.cells
            .iter()
            .map(|cell| cell.colspan.max(1) as usize)
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    pub content: Vec<Inline>,
    #[serde(default = "default_span")]
    pub colspan: u32,
    #[serde(default = "default_span")]
    pub rowspan: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<Alignment>,
}

fn default_span() -> u32 {
    1
}

impl TableCell {
    pub fn new(content: Vec<Inline>) -> Self {
        Self {
            content,
            colspan: 1,
            rowspan: 1,
            alignment: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageBlock {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<Vec<Inline>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inline {
    pub text: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub strong: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub emphasis: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intents: Vec<StyleIntent>,
    pub source: SourceSpan,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl Inline {
    pub fn text(text: impl Into<String>, source: SourceSpan) -> Self {
        Self {
            text: text.into(),
            strong: false,
            emphasis: false,
            intents: Vec::new(),
            source,
        }
    }

    pub fn strong(mut self) -> Self {
        self.strong = true;
        self
    }

    pub fn emphasis(mut self) -> Self {
        self.emphasis = true;
        self
    }

    pub fn with_intent(mut self, intent: StyleIntent) -> Self {
        self.intents.push(intent);
        self
    }
}

/// An explicit style from the source. It is a *suggestion*; a backend decides
/// whether to adopt it and must report every intent it overrides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleIntent {
    #[serde(flatten)]
    pub property: StyleProperty,
    pub frontend: FrontendId,
    pub source: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "property", rename_all = "snake_case")]
pub enum StyleProperty {
    FontFamily { value: String },
    FontSize { points: f64 },
    Color { hex: String },
    Bold { value: bool },
    Italic { value: bool },
}

impl StyleProperty {
    pub fn name(&self) -> &'static str {
        match self {
            Self::FontFamily { .. } => "font-family",
            Self::FontSize { .. } => "font-size",
            Self::Color { .. } => "color",
            Self::Bold { .. } => "bold",
            Self::Italic { .. } => "italic",
        }
    }

    pub fn value_display(&self) -> String {
        match self {
            Self::FontFamily { value } => value.clone(),
            Self::FontSize { points } => format_number(*points, "pt"),
            Self::Color { hex } => hex.clone(),
            Self::Bold { value } => value.to_string(),
            Self::Italic { value } => value.to_string(),
        }
    }

    pub fn property_label(&self) -> &'static str {
        match self {
            Self::FontFamily { .. } => "font-family",
            Self::FontSize { .. } => "font-size",
            Self::Color { .. } => "color",
            Self::Bold { .. } => "bold",
            Self::Italic { .. } => "italic",
        }
    }
}

pub fn format_number(value: f64, unit: &str) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{}{}", value as i64, unit)
    } else {
        format!("{}{}", value, unit)
    }
}

pub fn normalize_hex_color(input: &str) -> Option<String> {
    let raw = input.trim().trim_start_matches('#');
    if !raw.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    match raw.len() {
        3 => {
            let expanded: String = raw.chars().flat_map(|ch| [ch, ch]).collect();
            Some(format!("#{}", expanded.to_ascii_uppercase()))
        }
        6 => Some(format!("#{}", raw.to_ascii_uppercase())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> SourceSpan {
        SourceSpan::point("test.md", 1, 1, 0)
    }

    #[test]
    fn ir_round_trips_through_json() {
        let document = Document::new(FrontendId::Markdown, Metadata::new()).with_blocks(vec![
            Block::heading(1, vec![Inline::text("标题", span())], span()),
            Block::paragraph(
                vec![Inline::text("正文", span()).with_intent(StyleIntent {
                    property: StyleProperty::Color {
                        hex: "#FF0000".into(),
                    },
                    frontend: FrontendId::Typst,
                    source: span(),
                })],
                span(),
            ),
        ]);

        let json = serde_json::to_string_pretty(&document).unwrap();
        let restored: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(document, restored);
        assert!(json.contains("\"ir_version\": \"1.1\""));
        assert!(json.contains("\"property\": \"color\""));
        assert!(!json.contains("auto_number"));
    }

    #[test]
    fn heading_numbering_suggestion_round_trips() {
        let document =
            Document::new(FrontendId::Markdown, Metadata::new()).with_blocks(vec![Block::heading(
                1,
                vec![Inline::text("标题", span())],
                span(),
            )
            .with_auto_number(true)]);

        let json = serde_json::to_string(&document).unwrap();
        let restored: Document = serde_json::from_str(&json).unwrap();

        assert_eq!(document, restored);
        assert!(json.contains("\"auto_number\":true"));
    }

    #[test]
    fn hex_colors_are_normalized() {
        assert_eq!(normalize_hex_color("f00").as_deref(), Some("#FF0000"));
        assert_eq!(normalize_hex_color("#00ff00").as_deref(), Some("#00FF00"));
        assert_eq!(normalize_hex_color("red"), None);
    }
}

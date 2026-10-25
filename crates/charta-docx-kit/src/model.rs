//! Mechanical document model. No document-standard semantics live here.

use super::{Image, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Docx {
    pub page: PageSetup,
    pub styles: Stylesheet,
    pub numbering: Vec<NumberingDefinition>,
    pub body: Vec<BodyItem>,
    pub footers: Footers,
    pub core: CoreProperties,
    pub images: Vec<Image>,
}

impl Default for Docx {
    fn default() -> Self {
        Self::new()
    }
}

impl Docx {
    pub fn new() -> Self {
        Self {
            page: PageSetup::a4(),
            styles: Stylesheet::default(),
            numbering: Vec::new(),
            body: Vec::new(),
            footers: Footers::default(),
            core: CoreProperties::default(),
            images: Vec::new(),
        }
    }

    pub fn add_image(&mut self, image: Image) -> usize {
        self.images.push(image);
        self.images.len() - 1
    }

    pub fn push(&mut self, item: impl Into<BodyItem>) {
        self.body.push(item.into());
    }

    pub fn add_paragraph(&mut self, paragraph: Paragraph) {
        self.body.push(BodyItem::Paragraph(paragraph));
    }

    pub fn add_table(&mut self, table: Table) {
        self.body.push(BodyItem::Table(table));
    }

    pub fn add_style(&mut self, style: StyleDefinition) {
        self.styles.styles.push(style);
    }

    pub fn add_numbering(&mut self, numbering: NumberingDefinition) {
        self.numbering.push(numbering);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSetup {
    pub width_mm: f32,
    pub height_mm: f32,
    pub margin_top_mm: f32,
    pub margin_bottom_mm: f32,
    pub margin_left_mm: f32,
    pub margin_right_mm: f32,
    pub header_mm: f32,
    pub footer_mm: f32,
    pub vertical_alignment: Option<PageVerticalAlignment>,
}

impl PageSetup {
    pub fn a4() -> Self {
        Self {
            width_mm: 210.0,
            height_mm: 297.0,
            margin_top_mm: 25.4,
            margin_bottom_mm: 25.4,
            margin_left_mm: 25.4,
            margin_right_mm: 25.4,
            header_mm: 12.7,
            footer_mm: 17.5,
            vertical_alignment: None,
        }
    }

    pub fn text_width_mm(&self) -> f32 {
        self.width_mm - self.margin_left_mm - self.margin_right_mm
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageVerticalAlignment {
    Top,
    Center,
    Bottom,
    Both,
}

impl PageVerticalAlignment {
    pub(crate) fn xml_value(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Center => "center",
            Self::Bottom => "bottom",
            Self::Both => "both",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BodyItem {
    Paragraph(Paragraph),
    Table(Table),
    PageBreak,
    SectionBreak(SectionBreak),
}

impl From<Paragraph> for BodyItem {
    fn from(value: Paragraph) -> Self {
        Self::Paragraph(value)
    }
}

impl From<Table> for BodyItem {
    fn from(value: Table) -> Self {
        Self::Table(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SectionBreak {
    pub page: PageSetup,
    pub continuous: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Paragraph {
    pub formatting: ParagraphFormatting,
    pub runs: Vec<Run>,
}

impl Paragraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn styled(style: impl Into<String>) -> Self {
        Self {
            formatting: ParagraphFormatting {
                style: Some(style.into()),
                ..ParagraphFormatting::default()
            },
            runs: Vec::new(),
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        let mut paragraph = Self::new();
        paragraph.runs.push(Run::text(text));
        paragraph
    }

    pub fn with_formatting(formatting: ParagraphFormatting) -> Self {
        Self {
            formatting,
            runs: Vec::new(),
        }
    }

    pub fn run(mut self, run: Run) -> Self {
        self.runs.push(run);
        self
    }

    pub fn push_run(&mut self, run: Run) {
        self.runs.push(run);
    }

    pub fn push_text(&mut self, text: impl Into<String>) {
        self.runs.push(Run::text(text));
    }

    pub fn style(mut self, style: impl Into<String>) -> Self {
        self.formatting.style = Some(style.into());
        self
    }

    pub fn align(mut self, alignment: Alignment) -> Self {
        self.formatting.alignment = Some(alignment);
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParagraphFormatting {
    pub style: Option<String>,
    pub alignment: Option<Alignment>,
    pub spacing: Spacing,
    pub indent: Indent,
    pub borders: ParagraphBorders,
    pub shading: Option<[u8; 3]>,
    pub numbering: Option<NumberingRef>,
    pub keep_next: bool,
    pub keep_lines: bool,
    pub page_break_before: bool,
    /// Zero-based Word outline level (0 = Heading 1).
    pub outline_level: Option<u8>,
}

impl ParagraphFormatting {
    pub fn styled(style: impl Into<String>) -> Self {
        Self {
            style: Some(style.into()),
            ..Self::default()
        }
    }

    pub fn align(mut self, alignment: Alignment) -> Self {
        self.alignment = Some(alignment);
        self
    }

    pub fn first_line_chars(mut self, chars: f32) -> Self {
        self.indent.first_line_chars = Some(chars);
        self
    }

    pub fn outline_level(mut self, level: u8) -> Self {
        self.outline_level = Some(level);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Center,
    Right,
    Justify,
    Distribute,
}

impl Alignment {
    pub(crate) fn xml_value(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::Justify => "both",
            Self::Distribute => "distribute",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Spacing {
    pub before_pt: Option<f32>,
    pub after_pt: Option<f32>,
    pub line: Option<LineSpacing>,
}

impl Spacing {
    pub fn exact(points: f32) -> Self {
        Self {
            line: Some(LineSpacing::Exact { points }),
            ..Self::default()
        }
    }

    pub fn before_after(before_pt: f32, after_pt: f32) -> Self {
        Self {
            before_pt: Some(before_pt),
            after_pt: Some(after_pt),
            line: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineSpacing {
    Single,
    Exact { points: f32 },
    AtLeast { points: f32 },
    Multiple(f32),
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Indent {
    pub left_chars: Option<f32>,
    pub right_chars: Option<f32>,
    pub first_line_chars: Option<f32>,
    pub first_line_twips: Option<i32>,
    pub hanging_chars: Option<f32>,
    pub hanging_twips: Option<i32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ParagraphBorders {
    pub top: Option<Border>,
    pub bottom: Option<Border>,
    pub left: Option<Border>,
    pub right: Option<Border>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Border {
    /// Line width in eighths of a point.
    pub size_eighths_pt: u8,
    pub color: [u8; 3],
    pub space_pt: u8,
}

impl Border {
    pub fn single(size_eighths_pt: u8, color: [u8; 3]) -> Self {
        Self {
            size_eighths_pt,
            color,
            space_pt: 1,
        }
    }

    pub fn with_space(mut self, space_pt: u8) -> Self {
        self.space_pt = space_pt;
        self
    }

    pub(crate) fn color_hex(&self) -> String {
        format!(
            "{:02X}{:02X}{:02X}",
            self.color[0], self.color[1], self.color[2]
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Run {
    pub content: RunContent,
    pub properties: RunProperties,
}

impl Default for RunContent {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

impl Run {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: RunContent::Text(text.into()),
            properties: RunProperties::default(),
        }
    }

    pub fn page_number() -> Self {
        Self {
            content: RunContent::PageNumber,
            properties: RunProperties::default(),
        }
    }

    /// References an image previously registered with [`Docx::add_image`].
    pub fn image(index: usize) -> Self {
        Self {
            content: RunContent::Image(index),
            properties: RunProperties::default(),
        }
    }

    pub fn properties(mut self, properties: RunProperties) -> Self {
        self.properties = properties;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RunContent {
    Text(String),
    PageNumber,
    Image(usize),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunProperties {
    pub font_ascii: Option<String>,
    pub font_hansi: Option<String>,
    pub font_east_asia: Option<String>,
    pub font_cs: Option<String>,
    pub size_half_points: Option<u32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: bool,
    pub color: Option<[u8; 3]>,
    pub spacing_twips: Option<i32>,
}

impl RunProperties {
    pub fn font_east_asia(mut self, font: impl Into<String>) -> Self {
        self.font_east_asia = Some(font.into());
        self
    }

    pub fn font_ascii(mut self, font: impl Into<String>) -> Self {
        self.font_ascii = Some(font.into());
        self
    }

    pub fn font_hansi(mut self, font: impl Into<String>) -> Self {
        self.font_hansi = Some(font.into());
        self
    }

    pub fn font_cs(mut self, font: impl Into<String>) -> Self {
        self.font_cs = Some(font.into());
        self
    }

    pub fn size_pt(mut self, points: f32) -> Self {
        self.size_half_points = Some((points * 2.0).round().max(1.0) as u32);
        self
    }

    pub fn bold(mut self, value: bool) -> Self {
        self.bold = Some(value);
        self
    }

    pub fn italic(mut self, value: bool) -> Self {
        self.italic = Some(value);
        self
    }

    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    pub fn color(mut self, color: [u8; 3]) -> Self {
        self.color = Some(color);
        self
    }

    pub fn spacing_twips(mut self, twips: i32) -> Self {
        self.spacing_twips = Some(twips);
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stylesheet {
    pub default_font: Option<String>,
    pub default_east_asia_font: Option<String>,
    pub default_size_half_points: Option<u32>,
    pub styles: Vec<StyleDefinition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyleDefinition {
    pub id: String,
    pub name: String,
    pub kind: StyleKind,
    pub based_on: Option<String>,
    pub next: Option<String>,
    pub formatting: ParagraphFormatting,
    pub run: RunProperties,
}

impl StyleDefinition {
    pub fn paragraph(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: StyleKind::Paragraph,
            based_on: None,
            next: None,
            formatting: ParagraphFormatting::default(),
            run: RunProperties::default(),
        }
    }

    pub fn character(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            kind: StyleKind::Character,
            ..Self::paragraph(id, name)
        }
    }

    pub fn based_on(mut self, style: impl Into<String>) -> Self {
        self.based_on = Some(style.into());
        self
    }

    pub fn next(mut self, style: impl Into<String>) -> Self {
        self.next = Some(style.into());
        self
    }

    pub fn formatting(mut self, formatting: ParagraphFormatting) -> Self {
        self.formatting = formatting;
        self
    }

    pub fn run(mut self, run: RunProperties) -> Self {
        self.run = run;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleKind {
    Paragraph,
    Character,
}

impl StyleKind {
    pub(crate) fn xml_value(self) -> &'static str {
        match self {
            Self::Paragraph => "paragraph",
            Self::Character => "character",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NumberingDefinition {
    pub abstract_id: u32,
    pub num_id: u32,
    pub multi_level_type: MultiLevelType,
    pub levels: Vec<NumberingLevel>,
}

impl NumberingDefinition {
    pub fn new(abstract_id: u32, num_id: u32, levels: Vec<NumberingLevel>) -> Self {
        Self {
            abstract_id,
            num_id,
            multi_level_type: MultiLevelType::HybridMultilevel,
            levels,
        }
    }

    pub fn multi_level_type(mut self, value: MultiLevelType) -> Self {
        self.multi_level_type = value;
        self
    }

    pub fn bullet(abstract_id: u32, num_id: u32) -> Self {
        let levels = (0..3)
            .map(|level| NumberingLevel {
                level,
                format: NumberFormat::Bullet,
                text: "•".to_owned(),
                start: 1,
                indent_left_twips: 420 + level as i32 * 420,
                hanging_twips: 420,
                first_line_twips: None,
                suffix: NumberingSuffix::Tab,
                font: None,
            })
            .collect();
        Self::new(abstract_id, num_id, levels)
    }

    pub fn decimal(abstract_id: u32, num_id: u32) -> Self {
        let levels = (0..3)
            .map(|level| NumberingLevel {
                level,
                format: NumberFormat::Decimal,
                text: format!("%{}.", level + 1),
                start: 1,
                indent_left_twips: 420 + level as i32 * 420,
                hanging_twips: 420,
                first_line_twips: None,
                suffix: NumberingSuffix::Tab,
                font: None,
            })
            .collect();
        Self::new(abstract_id, num_id, levels)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiLevelType {
    SingleLevel,
    Multilevel,
    HybridMultilevel,
}

impl MultiLevelType {
    pub(crate) fn xml_value(self) -> &'static str {
        match self {
            Self::SingleLevel => "singleLevel",
            Self::Multilevel => "multilevel",
            Self::HybridMultilevel => "hybridMultilevel",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NumberingLevel {
    pub level: u32,
    pub format: NumberFormat,
    pub text: String,
    pub start: u32,
    pub indent_left_twips: i32,
    pub hanging_twips: i32,
    /// When set, the first line is indented by this amount and wrapped lines
    /// align to `indent_left_twips`; mutually exclusive with `hanging_twips`
    /// in the emitted XML.
    pub first_line_twips: Option<i32>,
    pub suffix: NumberingSuffix,
    pub font: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberingSuffix {
    Tab,
    Space,
    Nothing,
}

impl NumberingSuffix {
    pub(crate) fn xml_value(self) -> &'static str {
        match self {
            Self::Tab => "tab",
            Self::Space => "space",
            Self::Nothing => "nothing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberFormat {
    Bullet,
    Decimal,
    ChineseCounting,
}

impl NumberFormat {
    pub(crate) fn xml_value(self) -> &'static str {
        match self {
            Self::Bullet => "bullet",
            Self::Decimal => "decimal",
            Self::ChineseCounting => "chineseCounting",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberingRef {
    pub num_id: u32,
    pub level: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub columns: Vec<f32>,
    pub rows: Vec<TableRow>,
    pub borders: TableBorders,
    pub alignment: Option<Alignment>,
    pub width_mm: Option<f32>,
    pub cell_margin_mm: f32,
}

impl Table {
    pub fn new(columns: Vec<f32>, rows: Vec<TableRow>) -> Self {
        Self {
            columns,
            rows,
            borders: TableBorders::default(),
            alignment: None,
            width_mm: None,
            cell_margin_mm: 1.0,
        }
    }

    pub fn total_width_mm(&self) -> f32 {
        self.width_mm.unwrap_or_else(|| self.columns.iter().sum())
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    pub header: bool,
    pub cant_split: bool,
    pub height_mm: Option<f32>,
}

impl TableRow {
    pub fn new(cells: Vec<TableCell>) -> Self {
        Self {
            cells,
            header: false,
            cant_split: false,
            height_mm: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableCell {
    pub paragraphs: Vec<Paragraph>,
    pub shading: Option<[u8; 3]>,
    pub vertical_alignment: VerticalAlignment,
    pub grid_span: u32,
    pub v_merge: VMerge,
}

impl Default for TableCell {
    fn default() -> Self {
        Self {
            paragraphs: Vec::new(),
            shading: None,
            vertical_alignment: VerticalAlignment::Top,
            grid_span: 1,
            v_merge: VMerge::None,
        }
    }
}

impl TableCell {
    pub fn new(paragraphs: Vec<Paragraph>) -> Self {
        Self {
            paragraphs,
            ..Self::default()
        }
    }

    pub fn paragraph(paragraph: Paragraph) -> Self {
        Self::new(vec![paragraph])
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::paragraph(Paragraph::text(text))
    }

    pub fn shading(mut self, color: [u8; 3]) -> Self {
        self.shading = Some(color);
        self
    }

    pub fn grid_span(mut self, span: u32) -> Self {
        self.grid_span = span.max(1);
        self
    }

    pub fn v_merge(mut self, merge: VMerge) -> Self {
        self.v_merge = merge;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VMerge {
    None,
    Restart,
    Continue,
}

impl VMerge {
    pub(crate) fn xml_value(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Restart => Some("restart"),
            Self::Continue => Some("continue"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignment {
    Top,
    Center,
    Bottom,
}

impl VerticalAlignment {
    pub(crate) fn xml_value(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Center => "center",
            Self::Bottom => "bottom",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableBorders {
    pub top: Option<Border>,
    pub bottom: Option<Border>,
    pub left: Option<Border>,
    pub right: Option<Border>,
    pub inside_horizontal: Option<Border>,
    pub inside_vertical: Option<Border>,
}

impl TableBorders {
    pub fn all(border: Border) -> Self {
        Self {
            top: Some(border),
            bottom: Some(border),
            left: Some(border),
            right: Some(border),
            inside_horizontal: Some(border),
            inside_vertical: Some(border),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Footers {
    pub default: Option<Footer>,
    pub even: Option<Footer>,
    pub first: Option<Footer>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Footer {
    pub paragraphs: Vec<Paragraph>,
}

impl Footer {
    pub fn new(paragraphs: Vec<Paragraph>) -> Self {
        Self { paragraphs }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CoreProperties {
    pub title: Option<String>,
    pub creator: Option<String>,
    pub subject: Option<String>,
    pub created: Option<String>,
}

pub(crate) fn validate(docx: &Docx) -> Result<()> {
    for (index, table) in docx
        .body
        .iter()
        .filter_map(|item| match item {
            BodyItem::Table(table) => Some(table),
            _ => None,
        })
        .enumerate()
    {
        if table.columns.is_empty() {
            return Err(super::Error::Invalid(format!(
                "第 {} 个表格没有列宽定义",
                index + 1
            )));
        }
        if table.columns.iter().any(|width| *width <= 0.0) {
            return Err(super::Error::Invalid(format!(
                "第 {} 个表格存在非正列宽",
                index + 1
            )));
        }
        for (row_index, row) in table.rows.iter().enumerate() {
            let span: u32 = row.cells.iter().map(|cell| cell.grid_span.max(1)).sum();
            if span > table.columns.len() as u32 {
                return Err(super::Error::Invalid(format!(
                    "第 {} 个表格第 {} 行的网格跨度超过列定义",
                    index + 1,
                    row_index + 1
                )));
            }
        }
    }

    for paragraph in docx.body.iter().flat_map(body_paragraphs) {
        validate_image_refs(paragraph, docx.images.len())?;
    }
    for footer in [
        docx.footers.default.as_ref(),
        docx.footers.even.as_ref(),
        docx.footers.first.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        for paragraph in &footer.paragraphs {
            validate_image_refs(paragraph, docx.images.len())?;
        }
    }
    Ok(())
}

fn body_paragraphs(item: &BodyItem) -> Vec<&Paragraph> {
    match item {
        BodyItem::Paragraph(paragraph) => vec![paragraph],
        BodyItem::Table(table) => table
            .rows
            .iter()
            .flat_map(|row| row.cells.iter())
            .flat_map(|cell| cell.paragraphs.iter())
            .collect(),
        BodyItem::PageBreak | BodyItem::SectionBreak(_) => Vec::new(),
    }
}

fn validate_image_refs(paragraph: &Paragraph, image_count: usize) -> Result<()> {
    for run in &paragraph.runs {
        if let RunContent::Image(index) = &run.content {
            if *index >= image_count {
                return Err(super::Error::Invalid(format!(
                    "图片引用 {} 超出已注册图片数量 {}",
                    index, image_count
                )));
            }
        }
    }
    Ok(())
}

//! Style data for the official-document templates: loading, validation and
//! conversion to docx-kit values.
//!
//! The JSON file is the backend's style source. Rendering code only selects
//! styles and applies them; it does not carry font, size, color, margin,
//! spacing, border or column-width literals.

use std::collections::HashMap;
use std::path::Path;

use charta_docx_kit::{
    Alignment, Border, Indent, LineSpacing, MultiLevelType, NumberFormat, NumberingDefinition,
    NumberingLevel, NumberingSuffix, PageSetup, PageVerticalAlignment, ParagraphBorders,
    ParagraphFormatting, RunProperties, Spacing, StyleDefinition, Stylesheet, TabAlignment,
    TabLeader, TabStop, VerticalAlignment,
};
use serde::Deserialize;
use serde_json::{Map, Value};

pub(crate) const STYLE_COPY_NUMBER: &str = "GbCopyNumber";
pub(crate) const STYLE_MARK: &str = "GbMark";
pub(crate) const STYLE_ORG_MARK: &str = "GbOrgMark";
pub(crate) const STYLE_DOC_NUMBER: &str = "GbDocNumber";
pub(crate) const STYLE_DOC_NUMBER_UP: &str = "GbDocNumberUp";
pub(crate) const STYLE_SIGNER: &str = "GbSigner";
pub(crate) const STYLE_RED_LINE: &str = "GbRedLine";
pub(crate) const STYLE_TITLE: &str = "GbTitle";
pub(crate) const STYLE_RECIPIENT: &str = "GbRecipient";
pub(crate) const STYLE_BODY: &str = "GbBody";
pub(crate) const STYLE_HEADING1: &str = "GbHeading1";
pub(crate) const STYLE_HEADING2: &str = "GbHeading2";
pub(crate) const STYLE_HEADING3: &str = "GbHeading3";
pub(crate) const STYLE_QUOTE: &str = "GbQuote";
pub(crate) const STYLE_ATTACHMENT: &str = "GbAttachment";
pub(crate) const STYLE_ISSUER: &str = "GbIssuer";
pub(crate) const STYLE_DATE: &str = "GbDate";
pub(crate) const STYLE_NOTE: &str = "GbNote";
pub(crate) const STYLE_VERSION: &str = "GbVersionRecord";
pub(crate) const STYLE_TABLE_TEXT: &str = "GbTableText";
pub(crate) const STYLE_TABLE_HEADER: &str = "GbTableHeader";
pub(crate) const STYLE_TABLE_CAPTION: &str = "GbTableCaption";
pub(crate) const STYLE_IMAGE: &str = "GbImage";
pub(crate) const STYLE_IMAGE_CAPTION: &str = "GbImageCaption";
pub(crate) const STYLE_BYLINE: &str = "GbByline";
pub(crate) const STYLE_STRONG: &str = "GbStrong";
pub(crate) const STYLE_EMPHASIS: &str = "GbEmphasis";
pub(crate) const STYLE_PAGE_NUMBER: &str = "GbPageNumber";
pub(crate) const STYLE_TOC_TITLE: &str = "TocTitle";
pub(crate) const STYLE_TOC_SPACER: &str = "TOCSpacer";
pub(crate) const STYLE_TOC1: &str = "TOC1";
pub(crate) const STYLE_COVER_SPACER: &str = "CoverSpacer";
pub(crate) const STYLE_COVER_TITLE: &str = "CoverTitle";
pub(crate) const STYLE_COVER_DATE: &str = "CoverDate";

const REQUIRED_STYLES: &[&str] = &[
    "Normal",
    STYLE_COPY_NUMBER,
    STYLE_MARK,
    STYLE_ORG_MARK,
    STYLE_DOC_NUMBER,
    STYLE_DOC_NUMBER_UP,
    STYLE_SIGNER,
    STYLE_RED_LINE,
    STYLE_TITLE,
    STYLE_RECIPIENT,
    STYLE_BODY,
    STYLE_HEADING1,
    STYLE_HEADING2,
    STYLE_HEADING3,
    STYLE_QUOTE,
    STYLE_ATTACHMENT,
    STYLE_ISSUER,
    STYLE_DATE,
    STYLE_NOTE,
    STYLE_VERSION,
    STYLE_TABLE_TEXT,
    STYLE_TABLE_HEADER,
    STYLE_TABLE_CAPTION,
    STYLE_IMAGE,
    STYLE_IMAGE_CAPTION,
    STYLE_PAGE_NUMBER,
];

const GENERAL_REQUIRED_STYLES: &[&str] = &[
    "Normal",
    STYLE_TITLE,
    STYLE_BYLINE,
    STYLE_RECIPIENT,
    STYLE_BODY,
    STYLE_HEADING1,
    STYLE_HEADING2,
    STYLE_HEADING3,
    STYLE_QUOTE,
    STYLE_TABLE_TEXT,
    STYLE_TABLE_HEADER,
    STYLE_TABLE_CAPTION,
    STYLE_IMAGE,
    STYLE_IMAGE_CAPTION,
    STYLE_STRONG,
    STYLE_EMPHASIS,
    STYLE_PAGE_NUMBER,
    STYLE_TOC_TITLE,
    STYLE_TOC_SPACER,
    STYLE_TOC1,
    STYLE_COVER_SPACER,
    STYLE_COVER_TITLE,
    STYLE_COVER_DATE,
];

const SCHEMA_VERSION: u32 = 1;

const FONT_SIZE_LADDER: [f32; 16] = [
    42.0, 36.0, 26.0, 24.0, 22.0, 18.0, 16.0, 15.0, 14.0, 12.0, 10.5, 9.0, 7.5, 6.5, 5.5, 5.0,
];

const FONT_SIZE_NAMES: [&str; 16] = [
    "初号", "小初", "一号", "小一", "二号", "小二", "三号", "小三", "四号", "小四", "五号", "小五",
    "六号", "小六", "七号", "八号",
];

/// Which required-style set a style file must satisfy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleSet {
    /// GB/T 9704 notice layout.
    Notice,
    /// General-material layout.
    General,
}

impl StyleSet {
    fn backend_id(self) -> &'static str {
        match self {
            Self::Notice => "gb9704",
            Self::General => "general",
        }
    }

    fn required(self) -> &'static [&'static str] {
        match self {
            Self::Notice => REQUIRED_STYLES,
            Self::General => GENERAL_REQUIRED_STYLES,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DocStyles {
    defaults: Defaults,
    page: PageConfig,
    styles: HashMap<String, StyleConfig>,
    order: Vec<String>,
    heading_numbering: Option<NumberingDefinition>,
    table: TableConfig,
    image: ImageConfig,
    version_record: VersionRecordConfig,
    footer: FooterConfig,
}

impl DocStyles {
    /// Parse the notice style data embedded in the binary.
    pub fn embedded() -> charta::Result<Self> {
        Self::embedded_for(StyleSet::Notice)
    }

    /// Parse the style data embedded for the general-material backend.
    pub fn embedded_general() -> charta::Result<Self> {
        Self::embedded_for(StyleSet::General)
    }

    /// Parse the style data embedded for `set`.
    pub fn embedded_for(set: StyleSet) -> charta::Result<Self> {
        Self::from_json_for(embedded_json(set), set)
    }

    /// Parse notice style data from a JSON string. Invalid or incomplete data
    /// is an error; there is no partial fallback.
    pub fn from_json(json: &str) -> charta::Result<Self> {
        Self::from_json_for(json, StyleSet::Notice)
    }

    /// Parse style data from a JSON string, validating it against `set`.
    pub fn from_json_for(json: &str, set: StyleSet) -> charta::Result<Self> {
        let raw: RawFile = serde_json::from_str(json).map_err(|error| {
            remap_backend(invalid(format!("无法解析 JSON：{error}")), set.backend_id())
        })?;
        Self::from_raw(raw, set).map_err(|error| remap_backend(error, set.backend_id()))
    }

    /// Load notice style data from an external JSON file.
    pub fn from_path(path: impl AsRef<Path>) -> charta::Result<Self> {
        Self::from_path_for(path, StyleSet::Notice)
    }

    /// Load complete style data from an external JSON file, validating it
    /// against `set`.
    pub fn from_path_for(path: impl AsRef<Path>, set: StyleSet) -> charta::Result<Self> {
        Self::from_json_for(&read_style_text(path.as_ref(), set)?, set)
    }

    /// Loads a complete or partial style file. A file whose top-level `styles`
    /// member is an array is treated as complete style data; otherwise it is a
    /// patch merged over the embedded data for `set`.
    pub fn from_external_for(path: impl AsRef<Path>, set: StyleSet) -> charta::Result<Self> {
        let json = read_style_text(path.as_ref(), set)?;
        let value: Value = serde_json::from_str(&json).map_err(|error| {
            remap_backend(
                invalid(format!("无法解析样式 JSON：{error}")),
                set.backend_id(),
            )
        })?;
        let complete = value.get("styles").map(Value::is_array).unwrap_or(false);
        if complete {
            Self::from_json_for(&json, set)
        } else {
            Self::from_json_with_patch(embedded_json(set), &json, set)
        }
    }

    /// Applies a partial style patch to complete style data.
    pub fn from_json_with_patch(base: &str, patch: &str, set: StyleSet) -> charta::Result<Self> {
        let mut base: Value = serde_json::from_str(base).map_err(|error| {
            remap_backend(
                invalid(format!("无法解析基础样式 JSON：{error}")),
                set.backend_id(),
            )
        })?;
        let patch: Value = serde_json::from_str(patch).map_err(|error| {
            remap_backend(
                invalid(format!("无法解析样式补丁 JSON：{error}")),
                set.backend_id(),
            )
        })?;
        apply_patch(&mut base, patch).map_err(|error| remap_backend(error, set.backend_id()))?;
        let json = serde_json::to_string(&base).map_err(|error| {
            remap_backend(
                invalid(format!("无法序列化合并后的样式：{error}")),
                set.backend_id(),
            )
        })?;
        Self::from_json_for(&json, set)
    }

    /// Checks that this style data satisfies `set`'s required styles.
    pub fn validate(&self, set: StyleSet) -> charta::Result<()> {
        check_required(&self.styles, set.required())
            .map_err(|error| remap_backend(error, set.backend_id()))
    }

    fn from_raw(raw: RawFile, set: StyleSet) -> charta::Result<Self> {
        if raw.schema != SCHEMA_VERSION {
            return Err(invalid(format!(
                "不支持的样式数据版本 {}，期望 {}",
                raw.schema, SCHEMA_VERSION
            )));
        }

        let defaults = raw.defaults.resolve()?;
        let colors = resolve_colors(raw.colors)?;
        let page = raw.page.resolve()?;
        let heading_numbering = resolve_heading_numbering(raw.numbering)?;

        let mut styles = HashMap::new();
        let mut order = Vec::new();
        for style in raw.styles {
            if styles.contains_key(&style.id) {
                return Err(invalid(format!("样式 `{}` 重复定义", style.id)));
            }
            let style = style.resolve(&defaults, &colors)?;
            order.push(style.id.clone());
            styles.insert(style.id.clone(), style);
        }
        for id in &order {
            if let Some(base) = &styles[id].based_on {
                if !styles.contains_key(base) {
                    return Err(invalid(format!(
                        "样式 `{id}` 的 based_on 指向未定义样式 `{base}`"
                    )));
                }
            }
        }
        check_required(&styles, set.required())
            .map_err(|error| remap_backend(error, set.backend_id()))?;

        Ok(Self {
            defaults,
            page,
            styles,
            order,
            heading_numbering,
            table: raw.table.resolve(&colors)?,
            image: raw.image.resolve()?,
            version_record: raw.version_record.resolve(&colors)?,
            footer: raw.footer.resolve()?,
        })
    }

    pub(crate) fn page(&self) -> &PageConfig {
        &self.page
    }

    pub(crate) fn heading_numbering(&self) -> Option<&NumberingDefinition> {
        self.heading_numbering.as_ref()
    }

    pub(crate) fn table(&self) -> &TableConfig {
        &self.table
    }

    pub(crate) fn image(&self) -> &ImageConfig {
        &self.image
    }

    pub(crate) fn version_record(&self) -> &VersionRecordConfig {
        &self.version_record
    }

    pub(crate) fn footer(&self) -> &FooterConfig {
        &self.footer
    }

    pub(crate) fn run(&self, id: &str) -> &RunConfig {
        self.styles
            .get(id)
            .unwrap_or_else(|| panic!("样式 `{id}` 未在样式数据中定义"))
            .run
            .as_ref()
            .unwrap_or_else(|| panic!("样式 `{id}` 未定义 run 字符格式"))
    }

    pub(crate) fn try_run(&self, id: &str) -> Option<&RunConfig> {
        self.styles.get(id).and_then(|style| style.run.as_ref())
    }

    pub(crate) fn indent(&self, id: &str) -> Indent {
        self.styles
            .get(id)
            .map(|style| style.paragraph.indent)
            .unwrap_or_default()
    }

    pub(crate) fn stylesheet(&self) -> Stylesheet {
        let mut sheet = Stylesheet {
            default_font: Some(self.defaults.ascii_font.clone()),
            default_east_asia_font: Some(self.defaults.east_asia_font.clone()),
            default_size_half_points: Some(pt_to_half_points(self.defaults.size_pt)),
            styles: Vec::new(),
        };
        for id in &self.order {
            let style = &self.styles[id];
            let mut definition = StyleDefinition::paragraph(style.id.as_str(), style.name.as_str());
            if let Some(based_on) = &style.based_on {
                definition = definition.based_on(based_on.as_str());
            }
            definition = definition.formatting(style.paragraph.to_formatting());
            if let Some(run) = &style.run {
                definition = definition.run(run.properties());
            }
            sheet.styles.push(definition);
        }
        sheet
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Defaults {
    ascii_font: String,
    east_asia_font: String,
    size_pt: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct PageConfig {
    pub(crate) width_mm: f32,
    pub(crate) height_mm: f32,
    pub(crate) margin_top_mm: f32,
    pub(crate) margin_bottom_mm: f32,
    pub(crate) margin_left_mm: f32,
    pub(crate) margin_right_mm: f32,
    pub(crate) header_mm: f32,
    pub(crate) footer_mm: f32,
    pub(crate) vertical_alignment: Option<PageVerticalAlignment>,
    pub(crate) version_record_vertical_alignment: Option<PageVerticalAlignment>,
}

impl PageConfig {
    pub(crate) fn setup(&self) -> PageSetup {
        PageSetup {
            width_mm: self.width_mm,
            height_mm: self.height_mm,
            margin_top_mm: self.margin_top_mm,
            margin_bottom_mm: self.margin_bottom_mm,
            margin_left_mm: self.margin_left_mm,
            margin_right_mm: self.margin_right_mm,
            header_mm: self.header_mm,
            footer_mm: self.footer_mm,
            vertical_alignment: self.vertical_alignment,
            page_number_start: None,
        }
    }
}

#[derive(Debug, Clone)]
struct StyleConfig {
    id: String,
    name: String,
    based_on: Option<String>,
    paragraph: ParagraphConfig,
    run: Option<RunConfig>,
}

#[derive(Debug, Clone, Default)]
struct ParagraphConfig {
    alignment: Option<Alignment>,
    line_spacing_pt: Option<f32>,
    line_spacing: Option<LineSpacing>,
    space_before_pt: Option<f32>,
    space_after_pt: Option<f32>,
    keep_next: bool,
    indent: Indent,
    borders: ParagraphBorders,
    tabs: Vec<TabStop>,
}

impl ParagraphConfig {
    fn to_formatting(&self) -> ParagraphFormatting {
        ParagraphFormatting {
            alignment: self.alignment,
            spacing: Spacing {
                before_pt: self.space_before_pt,
                after_pt: self.space_after_pt,
                line: self.line_spacing.or_else(|| {
                    self.line_spacing_pt
                        .map(|points| LineSpacing::Exact { points })
                }),
            },
            indent: self.indent,
            borders: self.borders,
            keep_next: self.keep_next,
            tabs: self.tabs.clone(),
            ..ParagraphFormatting::default()
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RunConfig {
    pub(crate) ascii_font: String,
    pub(crate) east_asia_font: String,
    pub(crate) size_pt: f32,
    pub(crate) bold: bool,
    pub(crate) color: [u8; 3],
}

impl RunConfig {
    fn properties(&self) -> RunProperties {
        RunProperties::default()
            .font_ascii(&self.ascii_font)
            .font_hansi(&self.ascii_font)
            .font_east_asia(&self.east_asia_font)
            .font_cs(&self.ascii_font)
            .size_pt(self.size_pt)
            .bold(self.bold)
            .color(self.color)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TableConfig {
    pub(crate) width_mm: f32,
    pub(crate) cell_margin_mm: f32,
    pub(crate) alignment: Alignment,
    pub(crate) cell_vertical_alignment: VerticalAlignment,
    pub(crate) border: Border,
}

#[derive(Debug, Clone)]
pub(crate) struct ImageConfig {
    pub(crate) max_width_mm: f32,
    pub(crate) max_height_mm: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct VersionRecordConfig {
    pub(crate) width_mm: f32,
    pub(crate) columns_mm: Vec<f32>,
    pub(crate) cell_margin_mm: f32,
    pub(crate) date_alignment: Alignment,
    pub(crate) border: Border,
}

#[derive(Debug, Clone)]
pub(crate) struct FooterConfig {
    pub(crate) ascii_font: String,
    pub(crate) prefix: String,
    pub(crate) suffix: String,
    pub(crate) odd: FooterSide,
    pub(crate) even: FooterSide,
}

#[derive(Debug, Clone)]
pub(crate) struct FooterSide {
    pub(crate) alignment: Alignment,
    pub(crate) indent_chars: f32,
}

fn resolve_colors(raw: HashMap<String, String>) -> charta::Result<HashMap<String, [u8; 3]>> {
    let mut colors = HashMap::new();
    for (name, value) in raw {
        let color = parse_color(&value)
            .ok_or_else(|| invalid(format!("颜色 `{name}` 的值 `{value}` 不是合法的 #RRGGBB")))?;
        colors.insert(name, color);
    }
    if colors.is_empty() {
        return Err(invalid("colors 不能为空"));
    }
    Ok(colors)
}

fn resolve_color(colors: &HashMap<String, [u8; 3]>, name: &str) -> charta::Result<[u8; 3]> {
    colors
        .get(name)
        .copied()
        .ok_or_else(|| invalid(format!("引用了未定义的颜色 `{name}`")))
}

fn parse_alignment(value: &str, context: &str) -> charta::Result<Alignment> {
    match value {
        "left" => Ok(Alignment::Left),
        "center" => Ok(Alignment::Center),
        "right" => Ok(Alignment::Right),
        "justify" => Ok(Alignment::Justify),
        other => Err(invalid(format!("{context} 的对齐方式 `{other}` 无效"))),
    }
}

fn parse_line_spacing(value: &str, style_id: &str) -> charta::Result<LineSpacing> {
    match value {
        "single" => Ok(LineSpacing::Single),
        "1.5" => Ok(LineSpacing::Multiple(1.5)),
        "double" => Ok(LineSpacing::Multiple(2.0)),
        other => Err(invalid(format!(
            "样式 `{style_id}` 的 line_spacing `{other}` 无效；可用：single、1.5、double"
        ))),
    }
}

impl RawTabStop {
    fn resolve(self, style_id: &str) -> charta::Result<TabStop> {
        if !positive(self.position_mm) {
            return Err(invalid(format!(
                "样式 `{style_id}` 的制表位 position_mm 必须大于 0"
            )));
        }
        let alignment = match self.alignment.as_str() {
            "left" => TabAlignment::Left,
            "center" => TabAlignment::Center,
            "right" => TabAlignment::Right,
            other => {
                return Err(invalid(format!(
                    "样式 `{style_id}` 的制表位对齐 `{other}` 无效；可用：left、center、right"
                )))
            }
        };
        let leader = match self.leader.as_str() {
            "none" => TabLeader::None,
            "dot" => TabLeader::Dot,
            other => {
                return Err(invalid(format!(
                    "样式 `{style_id}` 的制表位前导符 `{other}` 无效；可用：none、dot"
                )))
            }
        };
        Ok(TabStop {
            position_twips: (self.position_mm * 1440.0 / 25.4).round() as i32,
            alignment,
            leader,
        })
    }
}

fn parse_page_alignment(
    value: &str,
    context: &str,
) -> charta::Result<Option<PageVerticalAlignment>> {
    match value {
        "top" => Ok(None),
        "center" => Ok(Some(PageVerticalAlignment::Center)),
        "bottom" => Ok(Some(PageVerticalAlignment::Bottom)),
        "both" => Ok(Some(PageVerticalAlignment::Both)),
        other => Err(invalid(format!("{context} 的垂直对齐 `{other}` 无效"))),
    }
}

fn parse_vertical_alignment(value: &str, context: &str) -> charta::Result<VerticalAlignment> {
    match value {
        "top" => Ok(VerticalAlignment::Top),
        "center" => Ok(VerticalAlignment::Center),
        "bottom" => Ok(VerticalAlignment::Bottom),
        other => Err(invalid(format!("{context} 的垂直对齐 `{other}` 无效"))),
    }
}

fn pt_to_half_points(points: f32) -> u32 {
    (points * 2.0).round().max(1.0) as u32
}

pub(crate) fn color_hex(color: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2])
}

pub(crate) fn parse_color(hex: &str) -> Option<[u8; 3]> {
    let raw = hex.trim().trim_start_matches('#');
    if !raw.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    let expanded = match raw.len() {
        3 => raw.chars().flat_map(|ch| [ch, ch]).collect::<String>(),
        6 => raw.to_owned(),
        _ => return None,
    };
    let r = u8::from_str_radix(&expanded[0..2], 16).ok()?;
    let g = u8::from_str_radix(&expanded[2..4], 16).ok()?;
    let b = u8::from_str_radix(&expanded[4..6], 16).ok()?;
    Some([r, g, b])
}

fn positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn non_negative(value: f32) -> bool {
    value.is_finite() && value >= 0.0
}

fn embedded_json(set: StyleSet) -> &'static str {
    match set {
        StyleSet::Notice => include_str!("../data/styles.json"),
        StyleSet::General => include_str!("../data/general-styles.json"),
    }
}

fn read_style_text(path: &Path, set: StyleSet) -> charta::Result<String> {
    std::fs::read_to_string(path).map_err(|error| {
        charta::Error::backend(
            set.backend_id(),
            format!("无法读取样式数据 `{}`：{error}", path.display()),
        )
    })
}

const PATCH_SECTIONS: &[&str] = &[
    "defaults",
    "colors",
    "styles",
    "numbering",
    "page",
    "table",
    "image",
    "version_record",
    "footer",
];

fn apply_patch(base: &mut Value, patch: Value) -> charta::Result<()> {
    let patch = match patch {
        Value::Object(map) => map,
        _ => return Err(invalid("样式补丁必须是 JSON 对象")),
    };
    for key in patch.keys() {
        if !PATCH_SECTIONS.contains(&key.as_str()) {
            return Err(invalid(format!(
                "样式补丁包含未知字段 `{key}`；可用字段：{}",
                PATCH_SECTIONS.join("、")
            )));
        }
    }
    for (key, value) in patch {
        match key.as_str() {
            "styles" => apply_style_patch(base, value)?,
            "colors" => apply_colors(base, value)?,
            _ => {
                let target = base
                    .get_mut(&key)
                    .ok_or_else(|| invalid(format!("基础样式缺少 `{key}` 段")))?;
                merge_json(target, value, &key)?;
            }
        }
    }
    Ok(())
}

fn apply_colors(base: &mut Value, patch: Value) -> charta::Result<()> {
    let patch = match patch {
        Value::Object(map) => map,
        _ => return Err(invalid("样式补丁的 `colors` 必须是对象")),
    };
    let target = base
        .get_mut("colors")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| invalid("基础样式缺少 colors 对象"))?;
    for (name, color) in patch {
        if !color.is_string() {
            return Err(invalid(format!("颜色 `{name}` 的值必须是字符串")));
        }
        target.insert(name, color);
    }
    Ok(())
}

fn apply_style_patch(base: &mut Value, patch: Value) -> charta::Result<()> {
    let patch =
        match patch {
            Value::Object(map) => map,
            _ => return Err(invalid(
                "样式补丁的 `styles` 必须是按样式 ID 索引的对象；完整样式文件的 `styles` 是数组",
            )),
        };
    for (id, style_patch) in patch {
        let styles = base
            .get_mut("styles")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("基础样式缺少 styles 数组"))?;
        let style = styles
            .iter_mut()
            .find(|style| style.get("id").and_then(Value::as_str) == Some(id.as_str()))
            .ok_or_else(|| invalid(format!("样式补丁引用了未定义样式 `{id}`")))?;
        let mut style_patch = match style_patch {
            Value::Object(map) => map,
            _ => return Err(invalid(format!("样式 `{id}` 的补丁必须是对象"))),
        };
        if let Some(run_patch) = style_patch.get_mut("run") {
            apply_run_patch(&id, style, run_patch)?;
        }
        merge_json_object(style, style_patch, &format!("styles.{id}"))?;
    }
    Ok(())
}

fn apply_run_patch(id: &str, style: &Value, run_patch: &mut Value) -> charta::Result<()> {
    let run_patch = run_patch
        .as_object_mut()
        .ok_or_else(|| invalid(format!("样式 `{id}` 的 run 补丁必须是对象")))?;
    let step = match run_patch.get("size_step") {
        Some(value) => value
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| invalid(format!("样式 `{id}` 的 size_step 必须是整数")))?,
        None => return Ok(()),
    };
    if run_patch.contains_key("size_pt") {
        return Err(invalid(format!(
            "样式 `{id}` 的 run 不能同时指定 size_pt 和 size_step"
        )));
    }
    let base_size = style
        .get("run")
        .and_then(|run| run.get("size_pt"))
        .and_then(Value::as_f64)
        .ok_or_else(|| invalid(format!("样式 `{id}` 没有可调整的 run.size_pt")))?;
    let size = shift_font_size(base_size as f32, step, &format!("样式 `{id}`"))?;
    run_patch.remove("size_step");
    run_patch.insert("size_pt".to_owned(), serde_json::json!(size));
    Ok(())
}

fn merge_json_object(
    target: &mut Value,
    patch: Map<String, Value>,
    path: &str,
) -> charta::Result<()> {
    let target = target
        .as_object_mut()
        .ok_or_else(|| invalid(format!("`{path}` 不是对象，无法合并")))?;
    for (key, value) in patch {
        match target.get_mut(&key) {
            Some(existing) => merge_json(existing, value, &format!("{path}.{key}"))?,
            None => {
                target.insert(key, value);
            }
        }
    }
    Ok(())
}

fn merge_json(target: &mut Value, patch: Value, path: &str) -> charta::Result<()> {
    match patch {
        Value::Object(patch) if target.is_object() => merge_json_object(target, patch, path),
        patch => {
            *target = patch;
            Ok(())
        }
    }
}

fn resolve_heading_numbering(
    raw: Option<RawNumbering>,
) -> charta::Result<Option<NumberingDefinition>> {
    let Some(raw) = raw.and_then(|numbering| numbering.heading) else {
        return Ok(None);
    };
    if raw.levels.is_empty() {
        return Err(invalid("numbering.heading.levels 不能为空"));
    }
    let restart_setting = raw.restart.unwrap_or_default();
    if let Some(never_from) = restart_setting.never_from {
        if !restart_setting.enabled {
            return Err(invalid(
                "numbering.heading.restart.never_from 只在 restart.enabled 为 true 时有效",
            ));
        }
        if !(2..=4).contains(&never_from) {
            return Err(invalid(
                "numbering.heading.restart.never_from 必须在 2 到 4 之间",
            ));
        }
    }
    let multi_level_type = parse_multi_level_type(&raw.multi_level_type)?;
    let mut levels = Vec::with_capacity(raw.levels.len());
    for raw_level in raw.levels {
        let context = format!("numbering.heading.levels[{}]", raw_level.level);
        if raw_level.level > 8 {
            return Err(invalid(format!("{context}.level 不能超过 8")));
        }
        if raw_level.text.trim().is_empty() {
            return Err(invalid(format!("{context}.text 不能为空")));
        }
        let placeholder = format!("%{}", raw_level.level + 1);
        if !raw_level.text.contains(&placeholder) {
            return Err(invalid(format!(
                "{context}.text 必须包含本层级占位符 `{placeholder}`"
            )));
        }
        if raw_level.start == 0 {
            return Err(invalid(format!("{context}.start 必须大于 0")));
        }
        if raw_level.indent_left_twips < 0 {
            return Err(invalid(format!("{context}.indent_left_twips 不能为负数")));
        }
        if let Some(first_line) = raw_level.first_line_twips {
            if first_line < 0 {
                return Err(invalid(format!("{context}.first_line_twips 不能为负数")));
            }
            if raw_level.hanging_twips.is_some() {
                return Err(invalid(format!(
                    "{context} 不能同时指定 first_line_twips 和 hanging_twips"
                )));
            }
        }
        let hanging_twips = raw_level.hanging_twips.unwrap_or(0);
        if hanging_twips < 0 {
            return Err(invalid(format!("{context}.hanging_twips 不能为负数")));
        }
        let font = match raw_level.font {
            Some(font) if font.trim().is_empty() => {
                return Err(invalid(format!("{context}.font 不能为空")));
            }
            font => font,
        };
        let one_based = raw_level.level + 1;
        let resets = restart_setting.enabled
            && match restart_setting.never_from {
                Some(from) => one_based < u32::from(from),
                None => true,
            };
        let restart_override = (one_based >= 2 && !resets).then_some(0);
        levels.push(NumberingLevel {
            level: raw_level.level,
            format: parse_number_format(&raw_level.format)?,
            text: raw_level.text,
            start: raw_level.start,
            indent_left_twips: raw_level.indent_left_twips,
            hanging_twips,
            first_line_twips: raw_level.first_line_twips,
            suffix: parse_numbering_suffix(&raw_level.suffix)?,
            font,
            restart: restart_override,
        });
    }
    levels.sort_by_key(|level| level.level);
    for pair in levels.windows(2) {
        if pair[0].level == pair[1].level {
            return Err(invalid(format!(
                "numbering.heading 层级 {} 重复定义",
                pair[0].level
            )));
        }
    }
    for expected in 0..4 {
        if !levels.iter().any(|level| level.level == expected) {
            return Err(invalid(format!(
                "numbering.heading 缺少第 {} 级（level {expected}）定义",
                expected + 1
            )));
        }
    }
    Ok(Some(
        NumberingDefinition::new(raw.abstract_id, raw.num_id, levels)
            .multi_level_type(multi_level_type),
    ))
}

fn parse_multi_level_type(value: &str) -> charta::Result<MultiLevelType> {
    match value {
        "singleLevel" => Ok(MultiLevelType::SingleLevel),
        "multilevel" => Ok(MultiLevelType::Multilevel),
        "hybridMultilevel" => Ok(MultiLevelType::HybridMultilevel),
        other => Err(invalid(format!(
            "numbering.heading.multi_level_type `{other}` 无效；可用：singleLevel、multilevel、hybridMultilevel"
        ))),
    }
}

fn parse_number_format(value: &str) -> charta::Result<NumberFormat> {
    match value {
        "bullet" => Ok(NumberFormat::Bullet),
        "decimal" => Ok(NumberFormat::Decimal),
        "chineseCounting" => Ok(NumberFormat::ChineseCounting),
        other => Err(invalid(format!(
            "编号格式 `{other}` 无效；可用：bullet、decimal、chineseCounting"
        ))),
    }
}

fn parse_numbering_suffix(value: &str) -> charta::Result<NumberingSuffix> {
    match value {
        "tab" => Ok(NumberingSuffix::Tab),
        "space" => Ok(NumberingSuffix::Space),
        "nothing" => Ok(NumberingSuffix::Nothing),
        other => Err(invalid(format!(
            "编号后缀 `{other}` 无效；可用：tab、space、nothing"
        ))),
    }
}

fn check_required(styles: &HashMap<String, StyleConfig>, required: &[&str]) -> charta::Result<()> {
    for id in required {
        let style = styles
            .get(*id)
            .ok_or_else(|| invalid(format!("缺少必需样式 `{id}`")))?;
        if *id != STYLE_RED_LINE && style.run.is_none() {
            return Err(invalid(format!("样式 `{id}` 缺少 run 字符格式")));
        }
    }
    Ok(())
}

fn shift_font_size(points: f32, step: i32, context: &str) -> charta::Result<f32> {
    let index = FONT_SIZE_LADDER
        .iter()
        .position(|value| (value - points).abs() < 0.01)
        .ok_or_else(|| {
            invalid(format!(
                "{context} 的字号 {points}pt 不在中文字号阶梯上，无法按号调整"
            ))
        })?;
    let target = index as i32 - step;
    if target < 0 || target as usize >= FONT_SIZE_LADDER.len() {
        return Err(invalid(format!(
            "{context} 的字号 {points}pt（{}）升降 {step} 级后超出中文字号阶梯",
            FONT_SIZE_NAMES[index]
        )));
    }
    Ok(FONT_SIZE_LADDER[target as usize])
}

fn remap_backend(error: charta::Error, backend: &'static str) -> charta::Error {
    match error {
        charta::Error::Backend { message, .. } => charta::Error::backend(backend, message),
        other => other,
    }
}

fn invalid(message: impl Into<String>) -> charta::Error {
    charta::Error::backend("gb9704", format!("样式数据无效：{}", message.into()))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    schema: u32,
    defaults: RawDefaults,
    colors: HashMap<String, String>,
    page: RawPage,
    styles: Vec<RawStyle>,
    #[serde(default)]
    numbering: Option<RawNumbering>,
    table: RawTable,
    image: RawImage,
    version_record: RawVersionRecord,
    footer: RawFooter,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNumbering {
    heading: Option<RawHeadingNumbering>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHeadingNumbering {
    abstract_id: u32,
    num_id: u32,
    #[serde(default = "default_heading_multi_level_type")]
    multi_level_type: String,
    #[serde(default)]
    restart: Option<RawHeadingRestart>,
    levels: Vec<RawNumberingLevel>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawHeadingRestart {
    #[serde(default = "default_restart_enabled")]
    enabled: bool,
    never_from: Option<u8>,
}

impl Default for RawHeadingRestart {
    fn default() -> Self {
        Self {
            enabled: true,
            never_from: None,
        }
    }
}

fn default_restart_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNumberingLevel {
    level: u32,
    format: String,
    text: String,
    #[serde(default = "default_numbering_start")]
    start: u32,
    #[serde(default)]
    indent_left_twips: i32,
    #[serde(default)]
    hanging_twips: Option<i32>,
    #[serde(default)]
    first_line_twips: Option<i32>,
    #[serde(default = "default_numbering_suffix")]
    suffix: String,
    font: Option<String>,
}

fn default_heading_multi_level_type() -> String {
    "multilevel".to_owned()
}

fn default_numbering_start() -> u32 {
    1
}

fn default_numbering_suffix() -> String {
    "tab".to_owned()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDefaults {
    ascii_font: String,
    east_asia_font: String,
    size_pt: f32,
}

impl RawDefaults {
    fn resolve(self) -> charta::Result<Defaults> {
        if self.ascii_font.trim().is_empty() {
            return Err(invalid("defaults.ascii_font 不能为空"));
        }
        if self.east_asia_font.trim().is_empty() {
            return Err(invalid("defaults.east_asia_font 不能为空"));
        }
        if !positive(self.size_pt) {
            return Err(invalid("defaults.size_pt 必须大于 0"));
        }
        Ok(Defaults {
            ascii_font: self.ascii_font,
            east_asia_font: self.east_asia_font,
            size_pt: self.size_pt,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPage {
    width_mm: f32,
    height_mm: f32,
    margin_top_mm: f32,
    margin_bottom_mm: f32,
    margin_left_mm: f32,
    margin_right_mm: f32,
    header_mm: f32,
    footer_mm: f32,
    vertical_alignment: String,
    version_record_vertical_alignment: String,
}

impl RawPage {
    fn resolve(self) -> charta::Result<PageConfig> {
        if !positive(self.width_mm) || !positive(self.height_mm) {
            return Err(invalid("page 的宽高必须大于 0"));
        }
        for (name, value) in [
            ("margin_top_mm", self.margin_top_mm),
            ("margin_bottom_mm", self.margin_bottom_mm),
            ("margin_left_mm", self.margin_left_mm),
            ("margin_right_mm", self.margin_right_mm),
            ("header_mm", self.header_mm),
            ("footer_mm", self.footer_mm),
        ] {
            if !non_negative(value) {
                return Err(invalid(format!("page.{name} 不能为负数")));
            }
        }
        Ok(PageConfig {
            width_mm: self.width_mm,
            height_mm: self.height_mm,
            margin_top_mm: self.margin_top_mm,
            margin_bottom_mm: self.margin_bottom_mm,
            margin_left_mm: self.margin_left_mm,
            margin_right_mm: self.margin_right_mm,
            header_mm: self.header_mm,
            footer_mm: self.footer_mm,
            vertical_alignment: parse_page_alignment(
                &self.vertical_alignment,
                "page.vertical_alignment",
            )?,
            version_record_vertical_alignment: parse_page_alignment(
                &self.version_record_vertical_alignment,
                "page.version_record_vertical_alignment",
            )?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStyle {
    id: String,
    name: String,
    based_on: Option<String>,
    #[serde(default)]
    paragraph: RawParagraph,
    run: Option<RawRun>,
}

impl RawStyle {
    fn resolve(
        self,
        defaults: &Defaults,
        colors: &HashMap<String, [u8; 3]>,
    ) -> charta::Result<StyleConfig> {
        if self.id.trim().is_empty() {
            return Err(invalid("样式 id 不能为空"));
        }
        if self.name.trim().is_empty() {
            return Err(invalid(format!("样式 `{}` 的 name 不能为空", self.id)));
        }
        let paragraph = self.paragraph.resolve(&self.id, colors)?;
        let run = self
            .run
            .map(|run| run.resolve(&self.id, defaults, colors))
            .transpose()?;
        Ok(StyleConfig {
            id: self.id,
            name: self.name,
            based_on: self.based_on,
            paragraph,
            run,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawParagraph {
    alignment: Option<String>,
    line_spacing_pt: Option<f32>,
    line_spacing: Option<String>,
    space_before_pt: Option<f32>,
    space_after_pt: Option<f32>,
    #[serde(default)]
    keep_next: bool,
    indent: Option<RawIndent>,
    #[serde(default)]
    tabs: Vec<RawTabStop>,
    top_border: Option<RawBorder>,
    bottom_border: Option<RawBorder>,
    left_border: Option<RawBorder>,
    right_border: Option<RawBorder>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTabStop {
    position_mm: f32,
    #[serde(default = "default_tab_alignment")]
    alignment: String,
    #[serde(default = "default_tab_leader")]
    leader: String,
}

fn default_tab_alignment() -> String {
    "right".to_owned()
}

fn default_tab_leader() -> String {
    "none".to_owned()
}

impl RawParagraph {
    fn resolve(
        self,
        style_id: &str,
        colors: &HashMap<String, [u8; 3]>,
    ) -> charta::Result<ParagraphConfig> {
        if let Some(points) = self.line_spacing_pt {
            if !positive(points) {
                return Err(invalid(format!(
                    "样式 `{style_id}` 的 line_spacing_pt 必须大于 0"
                )));
            }
        }
        if self.line_spacing.is_some() && self.line_spacing_pt.is_some() {
            return Err(invalid(format!(
                "样式 `{style_id}` 不能同时指定 line_spacing 和 line_spacing_pt"
            )));
        }
        let line_spacing = self
            .line_spacing
            .as_deref()
            .map(|value| parse_line_spacing(value, style_id))
            .transpose()?;
        for (name, value) in [
            ("space_before_pt", self.space_before_pt),
            ("space_after_pt", self.space_after_pt),
        ] {
            if let Some(value) = value {
                if !non_negative(value) {
                    return Err(invalid(format!("样式 `{style_id}` 的 {name} 不能为负数")));
                }
            }
        }
        let indent = self.indent.map(RawIndent::resolve).unwrap_or_default();
        let tabs = self
            .tabs
            .into_iter()
            .map(|tab| tab.resolve(style_id))
            .collect::<charta::Result<Vec<_>>>()?;
        Ok(ParagraphConfig {
            alignment: self
                .alignment
                .as_deref()
                .map(|value| parse_alignment(value, &format!("样式 `{style_id}`")))
                .transpose()?,
            line_spacing_pt: self.line_spacing_pt,
            line_spacing,
            space_before_pt: self.space_before_pt,
            space_after_pt: self.space_after_pt,
            keep_next: self.keep_next,
            indent,
            tabs,
            borders: ParagraphBorders {
                top: self
                    .top_border
                    .as_ref()
                    .map(|border| border.resolve(colors))
                    .transpose()?,
                bottom: self
                    .bottom_border
                    .as_ref()
                    .map(|border| border.resolve(colors))
                    .transpose()?,
                left: self
                    .left_border
                    .as_ref()
                    .map(|border| border.resolve(colors))
                    .transpose()?,
                right: self
                    .right_border
                    .as_ref()
                    .map(|border| border.resolve(colors))
                    .transpose()?,
            },
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIndent {
    left_chars: Option<f32>,
    right_chars: Option<f32>,
    first_line_chars: Option<f32>,
    first_line_twips: Option<i32>,
}

impl RawIndent {
    fn resolve(self) -> Indent {
        Indent {
            left_chars: self.left_chars,
            right_chars: self.right_chars,
            first_line_chars: self.first_line_chars,
            first_line_twips: self.first_line_twips,
            ..Indent::default()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBorder {
    width_eighths_pt: u8,
    color: String,
    space_pt: u8,
}

impl RawBorder {
    fn resolve(&self, colors: &HashMap<String, [u8; 3]>) -> charta::Result<Border> {
        if self.width_eighths_pt == 0 {
            return Err(invalid("边框宽度必须大于 0"));
        }
        Ok(Border {
            size_eighths_pt: self.width_eighths_pt,
            color: resolve_color(colors, &self.color)?,
            space_pt: self.space_pt,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRun {
    ascii_font: Option<String>,
    east_asia_font: String,
    size_pt: f32,
    #[serde(default)]
    bold: bool,
    color: String,
}

impl RawRun {
    fn resolve(
        self,
        style_id: &str,
        defaults: &Defaults,
        colors: &HashMap<String, [u8; 3]>,
    ) -> charta::Result<RunConfig> {
        if self.east_asia_font.trim().is_empty() {
            return Err(invalid(format!(
                "样式 `{style_id}` 的 east_asia_font 不能为空"
            )));
        }
        if !positive(self.size_pt) {
            return Err(invalid(format!("样式 `{style_id}` 的 size_pt 必须大于 0")));
        }
        let ascii_font = match self.ascii_font {
            Some(font) if !font.trim().is_empty() => font,
            Some(_) => return Err(invalid(format!("样式 `{style_id}` 的 ascii_font 不能为空"))),
            None => defaults.ascii_font.clone(),
        };
        Ok(RunConfig {
            ascii_font,
            east_asia_font: self.east_asia_font,
            size_pt: self.size_pt,
            bold: self.bold,
            color: resolve_color(colors, &self.color)?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTable {
    width_mm: f32,
    cell_margin_mm: f32,
    alignment: String,
    cell_vertical_alignment: String,
    border: RawBorder,
}

impl RawTable {
    fn resolve(self, colors: &HashMap<String, [u8; 3]>) -> charta::Result<TableConfig> {
        if !positive(self.width_mm) {
            return Err(invalid("table.width_mm 必须大于 0"));
        }
        if !non_negative(self.cell_margin_mm) {
            return Err(invalid("table.cell_margin_mm 不能为负数"));
        }
        Ok(TableConfig {
            width_mm: self.width_mm,
            cell_margin_mm: self.cell_margin_mm,
            alignment: parse_alignment(&self.alignment, "table")?,
            cell_vertical_alignment: parse_vertical_alignment(
                &self.cell_vertical_alignment,
                "table",
            )?,
            border: self.border.resolve(colors)?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawImage {
    max_width_mm: f32,
    max_height_mm: f32,
}

impl RawImage {
    fn resolve(self) -> charta::Result<ImageConfig> {
        if !positive(self.max_width_mm) || !positive(self.max_height_mm) {
            return Err(invalid("image 的最大宽高必须大于 0"));
        }
        Ok(ImageConfig {
            max_width_mm: self.max_width_mm,
            max_height_mm: self.max_height_mm,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVersionRecord {
    width_mm: f32,
    columns_mm: Vec<f32>,
    cell_margin_mm: f32,
    date_alignment: String,
    border: RawBorder,
}

impl RawVersionRecord {
    fn resolve(self, colors: &HashMap<String, [u8; 3]>) -> charta::Result<VersionRecordConfig> {
        if !positive(self.width_mm) {
            return Err(invalid("version_record.width_mm 必须大于 0"));
        }
        if self.columns_mm.len() != 2 {
            return Err(invalid("version_record.columns_mm 必须恰好包含两列"));
        }
        if self.columns_mm.iter().any(|width| !positive(*width)) {
            return Err(invalid("version_record.columns_mm 的列宽必须大于 0"));
        }
        let sum: f32 = self.columns_mm.iter().sum();
        if (sum - self.width_mm).abs() > 0.01 {
            return Err(invalid(format!(
                "version_record 列宽合计 {sum} 与 width_mm {} 不一致",
                self.width_mm
            )));
        }
        if !non_negative(self.cell_margin_mm) {
            return Err(invalid("version_record.cell_margin_mm 不能为负数"));
        }
        Ok(VersionRecordConfig {
            width_mm: self.width_mm,
            columns_mm: self.columns_mm,
            cell_margin_mm: self.cell_margin_mm,
            date_alignment: parse_alignment(&self.date_alignment, "version_record")?,
            border: self.border.resolve(colors)?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFooter {
    ascii_font: String,
    prefix: String,
    suffix: String,
    odd: RawFooterSide,
    even: RawFooterSide,
}

impl RawFooter {
    fn resolve(self) -> charta::Result<FooterConfig> {
        if self.ascii_font.trim().is_empty() {
            return Err(invalid("footer.ascii_font 不能为空"));
        }
        Ok(FooterConfig {
            ascii_font: self.ascii_font,
            prefix: self.prefix,
            suffix: self.suffix,
            odd: self.odd.resolve("footer.odd")?,
            even: self.even.resolve("footer.even")?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFooterSide {
    alignment: String,
    indent_chars: f32,
}

impl RawFooterSide {
    fn resolve(self, context: &str) -> charta::Result<FooterSide> {
        if !non_negative(self.indent_chars) {
            return Err(invalid(format!("{context}.indent_chars 不能为负数")));
        }
        Ok(FooterSide {
            alignment: parse_alignment(&self.alignment, context)?,
            indent_chars: self.indent_chars,
        })
    }
}

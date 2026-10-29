//! Chinese official-document backends.
//!
//! The crate hosts the GB/T 9704 notice, general materials and file
//! compilation templates, which share layout, styles and numbering. It is
//! split by responsibility:
//!
//! - `schema`: metadata contracts
//! - `styles` + `data/styles.json` / `data/general-styles.json`: style values
//! - `layout`: 版头/主体/版记/cover/TOC flow, run construction and page furniture
//! - `numbering`: list ordinals and heading numbering
//! - `pandoc`: pandoc `reference.docx` generation from the style data
//! - `report`: backend-owned report messages and codes
//! - `lib`: the `Backend` implementations and render orchestration
//!
//! No external `.docx` template is required or referenced.

use charta::{
    Backend, Issue, MetaValue, Metadata, MetadataSchema, RenderOutput, RenderRequest, Result,
};
use charta_docx_kit::{CoreProperties, Docx};

mod layout;
mod numbering;
mod pandoc;
mod report;
mod schema;
mod styles;

pub use styles::{DocStyles, StyleSet};

use schema::{build_compilation_schema, build_general_schema, build_schema};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocTemplate {
    Notice,
    General,
    Compilation,
}

#[derive(Debug, Clone)]
pub struct DocOptions {
    pub template: DocTemplate,
    /// Render 版头: copy number, security marks, organization mark, document
    /// number and the red separator.
    pub header: bool,
    /// Render 版记: the copy/issuing record and its dedicated last section.
    pub record: bool,
    /// Explicitly adopt the source color intent. The active template is still
    /// the visual authority for every other property.
    pub adopt_color: bool,
    pub adopt_font_family: bool,
    pub adopt_font_size: bool,
    pub adopt_bold: bool,
    pub adopt_italic: bool,
}

impl Default for DocOptions {
    fn default() -> Self {
        Self {
            template: DocTemplate::Notice,
            header: true,
            record: true,
            adopt_color: true,
            adopt_font_family: false,
            adopt_font_size: false,
            adopt_bold: false,
            adopt_italic: false,
        }
    }
}

impl DocOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn general() -> Self {
        Self {
            template: DocTemplate::General,
            header: false,
            record: false,
            ..Self::default()
        }
    }

    pub(crate) fn compilation() -> Self {
        Self {
            template: DocTemplate::Compilation,
            header: false,
            record: false,
            ..Self::default()
        }
    }

    pub fn without_header(mut self) -> Self {
        self.header = false;
        self
    }

    pub fn without_record(mut self) -> Self {
        self.record = false;
        self
    }

    pub fn adopt_all(mut self) -> Self {
        self.adopt_font_family = true;
        self.adopt_font_size = true;
        self.adopt_bold = true;
        self.adopt_italic = true;
        self
    }
}

#[derive(Debug, Clone)]
pub struct Gb9704Backend {
    schema: MetadataSchema,
    options: DocOptions,
    styles: DocStyles,
}

impl Default for Gb9704Backend {
    fn default() -> Self {
        Self::new()
    }
}

impl Gb9704Backend {
    /// Uses the style data embedded in the binary.
    ///
    /// # Panics
    ///
    /// Panics if the embedded style data is invalid. Use [`Self::try_new`] to
    /// handle the error instead.
    pub fn new() -> Self {
        Self::try_new().expect("内置 GB/T 9704 样式数据必须有效")
    }

    pub fn try_new() -> charta::Result<Self> {
        Self::try_with_options(DocOptions::default())
    }

    /// Uses the style data embedded in the binary.
    ///
    /// # Panics
    ///
    /// Panics if the embedded style data is invalid. Use
    /// [`Self::try_with_options`] to handle the error instead.
    pub fn with_options(options: DocOptions) -> Self {
        Self::try_with_options(options).expect("内置 GB/T 9704 样式数据必须有效")
    }

    pub fn try_with_options(options: DocOptions) -> charta::Result<Self> {
        Ok(Self {
            schema: build_schema(options.header),
            options,
            styles: DocStyles::embedded()?,
        })
    }

    /// Builds a backend from style data loaded by the caller.
    pub fn with_styles(options: DocOptions, styles: DocStyles) -> Self {
        Self {
            schema: build_schema(options.header),
            options,
            styles,
        }
    }

    /// Loads style data from an external JSON file. The file may be complete
    /// style data or a patch merged over the embedded data.
    pub fn with_style_file(
        options: DocOptions,
        path: impl AsRef<std::path::Path>,
    ) -> charta::Result<Self> {
        Ok(Self {
            schema: build_schema(options.header),
            options,
            styles: DocStyles::from_external_for(path, StyleSet::Notice)?,
        })
    }

    pub fn options(&self) -> &DocOptions {
        &self.options
    }

    pub fn styles(&self) -> &DocStyles {
        &self.styles
    }
}

impl Backend for Gb9704Backend {
    fn id(&self) -> &'static str {
        "gb9704"
    }

    fn display_name(&self) -> &str {
        "GB/T 9704-2012 通知"
    }

    fn schema(&self) -> &MetadataSchema {
        &self.schema
    }

    fn render(&self, request: RenderRequest<'_>) -> Result<RenderOutput> {
        let metadata = request.metadata;
        let mut issues = Vec::new();
        if metadata.get("signer").is_some() {
            issues.push(report::ignored_metadata(
                "signer",
                "通知属于下行文，不渲染签发人；上行文后端再实现",
            ));
        }
        if metadata.get("title").is_some() {
            issues.push(report::ignored_metadata(
                "title",
                "通知以正文第一个一级标题作为大标题，元数据 title 不参与渲染",
            ));
        }
        issues.extend(option_ignored_metadata(metadata, &self.options));
        let mut docx = Docx::new();
        docx.page = self.styles.page().setup();
        docx.styles = self.styles.stylesheet();
        docx.core = CoreProperties {
            title: layout::body::first_title(&request.document.blocks),
            creator: layout::string_field(metadata, "organization")
                .or_else(|| layout::string_field(metadata, "issuer"))
                .map(str::to_owned),
            subject: None,
            created: None,
        };
        docx.footers = layout::footer::build(&self.styles);

        if self.options.header {
            layout::header::render(&mut docx, metadata, &self.styles);
        }
        layout::body::render_main(
            &mut docx,
            metadata,
            &request.document.blocks,
            &self.options,
            &self.styles,
            &mut issues,
        );
        if self.options.record {
            layout::record::render(&mut docx, metadata, &self.styles);
        }

        let bytes = docx
            .to_bytes()
            .map_err(|error| charta::Error::backend("gb9704", error.to_string()))?;
        Ok(RenderOutput::with_issues(bytes, issues))
    }
}

#[derive(Debug, Clone)]
pub struct GeneralBackend {
    schema: MetadataSchema,
    styles: DocStyles,
}

impl Default for GeneralBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl GeneralBackend {
    /// # Panics
    ///
    /// Panics if the embedded general style data is invalid.
    pub fn new() -> Self {
        Self::try_new().expect("内置通用材料样式数据必须有效")
    }

    pub fn try_new() -> charta::Result<Self> {
        Self::try_with_styles(DocStyles::embedded_general()?)
    }

    /// Builds a backend from style data loaded by the caller. The data must
    /// satisfy the general-material required style set.
    pub fn try_with_styles(styles: DocStyles) -> charta::Result<Self> {
        styles.validate(StyleSet::General)?;
        Ok(Self {
            schema: build_general_schema(),
            styles,
        })
    }

    /// # Panics
    ///
    /// Panics if `styles` does not satisfy the general-material required style
    /// set. Use [`Self::try_with_styles`] to handle the error instead.
    pub fn with_styles(styles: DocStyles) -> Self {
        Self::try_with_styles(styles).expect("通用材料样式数据必须包含完整样式集")
    }

    /// Loads general-material style data from an external JSON file. The file
    /// may be complete style data or a patch merged over the embedded data.
    pub fn with_style_file(path: impl AsRef<std::path::Path>) -> charta::Result<Self> {
        Self::try_with_styles(DocStyles::from_external_for(path, StyleSet::General)?)
    }

    pub fn styles(&self) -> &DocStyles {
        &self.styles
    }
}

impl Backend for GeneralBackend {
    fn id(&self) -> &'static str {
        "general"
    }

    fn display_name(&self) -> &str {
        "通用材料（方正 GBK）"
    }

    fn schema(&self) -> &MetadataSchema {
        &self.schema
    }

    fn render(&self, request: RenderRequest<'_>) -> Result<RenderOutput> {
        let metadata = request.metadata;
        let mut issues = Vec::new();
        let options = DocOptions::general();
        let mut docx = Docx::new();
        docx.page = self.styles.page().setup();
        docx.styles = self.styles.stylesheet();
        docx.core = CoreProperties {
            title: layout::body::first_title(&request.document.blocks),
            creator: layout::string_field(metadata, "author").map(str::to_owned),
            subject: None,
            created: None,
        };
        docx.footers = layout::footer::build(&self.styles);

        if metadata.get("title").is_some() {
            issues.push(report::ignored_metadata(
                "title",
                "通用材料以一级标题作为大标题，元数据 title 不参与渲染",
            ));
        }
        if metadata
            .get("_charta")
            .and_then(MetaValue::as_map)
            .is_some_and(|control| control.contains_key("toc"))
        {
            issues.push(report::ignored_metadata(
                "_charta.toc",
                "通用材料不生成目录页，需要封面和目录请使用 compilation 后端",
            ));
        }

        layout::body::render_general(
            &mut docx,
            metadata,
            &request.document.blocks,
            &options,
            &self.styles,
            &mut issues,
        );

        let bytes = docx
            .to_bytes()
            .map_err(|error| charta::Error::backend("general", error.to_string()))?;
        Ok(RenderOutput::with_issues(bytes, issues))
    }
}

#[derive(Debug, Clone)]
pub struct CompilationBackend {
    schema: MetadataSchema,
    styles: DocStyles,
}

impl Default for CompilationBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilationBackend {
    /// # Panics
    ///
    /// Panics if the embedded compilation style data is invalid.
    pub fn new() -> Self {
        Self::try_new().expect("内置文件汇编样式数据必须有效")
    }

    pub fn try_new() -> charta::Result<Self> {
        Self::try_with_styles(DocStyles::embedded_general()?)
    }

    pub fn try_with_styles(styles: DocStyles) -> charta::Result<Self> {
        styles.validate(StyleSet::General)?;
        Ok(Self {
            schema: build_compilation_schema(),
            styles,
        })
    }

    pub fn with_styles(styles: DocStyles) -> Self {
        Self::try_with_styles(styles).expect("文件汇编样式数据必须包含完整样式集")
    }

    pub fn with_style_file(path: impl AsRef<std::path::Path>) -> charta::Result<Self> {
        Self::try_with_styles(DocStyles::from_external_for(path, StyleSet::General)?)
    }

    pub fn styles(&self) -> &DocStyles {
        &self.styles
    }
}

impl Backend for CompilationBackend {
    fn id(&self) -> &'static str {
        "compilation"
    }

    fn display_name(&self) -> &str {
        "文件汇编（方正 GBK）"
    }

    fn schema(&self) -> &MetadataSchema {
        &self.schema
    }

    fn render(&self, request: RenderRequest<'_>) -> Result<RenderOutput> {
        let metadata = request.metadata;
        let mut issues = Vec::new();
        let options = DocOptions::compilation();
        let mut docx = Docx::new();
        docx.page = self.styles.page().setup();
        docx.styles = self.styles.stylesheet();
        docx.core = CoreProperties {
            title: layout::string_field(metadata, "title").map(str::to_owned),
            creator: None,
            subject: None,
            created: None,
        };
        docx.footers = layout::footer::build(&self.styles);
        docx.update_fields = true;

        layout::cover::render(&mut docx, metadata, &self.styles);
        layout::toc::render(&mut docx, &self.styles);
        layout::body::render_compilation_content(
            &mut docx,
            &request.document.blocks,
            &options,
            &self.styles,
            &mut issues,
        );

        for (field, reason) in [
            ("author", "文件汇编封面不含署名，作者不参与渲染"),
            ("recipient", "文件汇编封面不含主送机关，该字段不参与渲染"),
        ] {
            if metadata.get(field).is_some() {
                issues.push(report::ignored_metadata(field, reason));
            }
        }

        let bytes = docx
            .to_bytes()
            .map_err(|error| charta::Error::backend("compilation", error.to_string()))?;
        Ok(RenderOutput::with_issues(bytes, issues))
    }
}

fn option_ignored_metadata(metadata: &Metadata, options: &DocOptions) -> Vec<Issue> {
    let mut issues = Vec::new();
    if !options.header {
        for field in ["doc_number", "copy_number", "security_level", "urgency"] {
            if metadata.get(field).is_some() {
                issues.push(report::ignored_metadata(
                    field,
                    "后端选项已关闭版头，该字段不参与渲染",
                ));
            }
        }
    }
    if !options.record {
        for field in ["cc", "issuing_office", "issuing_date"] {
            if metadata.get(field).is_some() {
                issues.push(report::ignored_metadata(
                    field,
                    "后端选项已关闭版记，该字段不参与渲染",
                ));
            }
        }
    }
    issues
}

#[cfg(test)]
mod tests;

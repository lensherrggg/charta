//! Charta core: semantic IR, frontend/backend contracts, metadata validation
//! and the report system.
//!
//! This crate is deliberately free of any document-standard semantics.
//! Everything normative lives in a backend.
//!
//! ```no_run
//! use charta::{Backend, ConversionRequest, Frontend, SourceFile, convert};
//! # fn run(frontend: &dyn Frontend, backend: &dyn Backend) -> charta::Result<()> {
//! let output = convert(ConversionRequest {
//!     source: SourceFile::read("notice.md")?,
//!     frontend,
//!     backend,
//!     strict: false,
//! })?;
//! if let Some(docx) = output.docx {
//!     std::fs::write("notice.docx", docx)?;
//! }
//! output.report.write_json("report.json")?;
//! # Ok(())
//! # }
//! ```

pub mod backend;
pub mod error;
pub mod frontend;
pub mod frontmatter;
mod headings;
pub mod ir;
pub mod pipeline;
pub mod report;
pub mod roles;
pub mod schema;

pub use backend::{Backend, RenderOutput, RenderRequest};
pub use error::{Error, Result};
pub use frontend::{Frontend, ParseOutput, ParseRequest};
pub use ir::{
    Alignment, Block, BlockKind, Determinism, Document, FrontendId, ImageBlock, Inline, ListBlock,
    ListItem, MetaValue, Metadata, RoleDeclaration, SourceSpan, StyleIntent, StyleProperty,
    TableBlock, TableCell, TableRow, IR_VERSION,
};
pub use pipeline::{convert, ConversionOutput, ConversionRequest, SourceFile};
pub use report::{Issue, IssueDetails, IssueKind, Report, ReportSummary, Severity, REPORT_VERSION};
pub use roles::{RoleRegistry, BASE_ROLES};
pub use schema::{FieldSchema, FieldType, MetadataSchema};

/// Engine version from Cargo. Independent from [`IR_VERSION`] and
/// [`REPORT_VERSION`].
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

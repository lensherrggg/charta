//! Backend contract.
//!
//! A backend owns the visual authority for its document standard. It receives
//! the IR plus metadata and returns a complete `.docx` package and issues.
//! Pagination, page fields, headers and footers are entirely backend concerns.

use crate::ir::{Document, Metadata, RoleDeclaration};
use crate::report::Issue;
use crate::schema::MetadataSchema;
use crate::Result;

pub struct RenderRequest<'a> {
    pub document: &'a Document,
    pub metadata: &'a Metadata,
}

pub struct RenderOutput {
    pub docx: Vec<u8>,
    pub issues: Vec<Issue>,
}

impl RenderOutput {
    pub fn new(docx: Vec<u8>) -> Self {
        Self {
            docx,
            issues: Vec::new(),
        }
    }

    pub fn with_issues(docx: Vec<u8>, issues: Vec<Issue>) -> Self {
        Self { docx, issues }
    }
}

pub trait Backend: Send + Sync {
    /// Stable machine-readable identifier, e.g. `gb9704`.
    fn id(&self) -> &'static str;

    fn display_name(&self) -> &str {
        self.id()
    }

    fn schema(&self) -> &MetadataSchema;

    /// Extended structure roles this backend understands. Declaring a role in
    /// document metadata activates it; registering it here marks it as known.
    fn roles(&self) -> Vec<RoleDeclaration> {
        Vec::new()
    }

    fn render(&self, request: RenderRequest<'_>) -> Result<RenderOutput>;
}

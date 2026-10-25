//! Frontend contract.

use crate::ir::{Document, FrontendId};
use crate::report::Issue;
use crate::Result;
use std::path::Path;

pub struct ParseRequest<'a> {
    pub path: &'a Path,
    pub source: &'a str,
}

impl ParseRequest<'_> {
    /// Stable source name used in IR spans and reports.
    pub fn source_name(&self) -> String {
        self.path.display().to_string()
    }
}

pub struct ParseOutput {
    pub document: Document,
    pub issues: Vec<Issue>,
}

impl ParseOutput {
    pub fn new(document: Document) -> Self {
        Self {
            document,
            issues: Vec::new(),
        }
    }

    pub fn with_issues(document: Document, issues: Vec<Issue>) -> Self {
        Self { document, issues }
    }
}

pub trait Frontend: Send + Sync {
    fn id(&self) -> FrontendId;

    fn display_name(&self) -> &str;

    fn parse(&self, request: ParseRequest<'_>) -> Result<ParseOutput>;
}

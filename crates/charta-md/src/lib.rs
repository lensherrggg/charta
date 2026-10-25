//! Markdown frontend for Charta.
//!
//! The frontend implements a controlled subset: ATX headings, paragraphs,
//! lists, GFM pipe tables, images, emphasis, block quotes and YAML front
//! matter. Every construct outside the subset is degraded *and* reported.

mod blocks;
mod inline;
mod line;

use charta::{Document, Frontend, FrontendId, ParseOutput, ParseRequest, Result};

#[derive(Debug, Default, Clone, Copy)]
pub struct MarkdownFrontend;

impl MarkdownFrontend {
    pub fn new() -> Self {
        Self
    }

    /// Parse a Markdown string without going through the trait. `source_name`
    /// is used verbatim in source spans and reports.
    pub fn parse_source(&self, source: &str, source_name: &str) -> ParseOutput {
        let source = source.strip_prefix('\u{feff}').unwrap_or(source);
        let parsed = blocks::parse(source, source_name);
        let document =
            Document::new(FrontendId::Markdown, parsed.metadata).with_blocks(parsed.blocks);
        ParseOutput::with_issues(document, parsed.issues)
    }
}

impl Frontend for MarkdownFrontend {
    fn id(&self) -> FrontendId {
        FrontendId::Markdown
    }

    fn display_name(&self) -> &str {
        "markdown"
    }

    fn parse(&self, request: ParseRequest<'_>) -> Result<ParseOutput> {
        Ok(self.parse_source(request.source, &request.source_name()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use charta::BlockKind;

    #[test]
    fn frontend_maps_metadata_and_blocks() {
        let frontend = MarkdownFrontend::new();
        let output =
            frontend.parse_source("---\ntitle: 通知\n---\n\n# 标题\n\n正文\n", "notice.md");
        assert!(output.issues.is_empty());
        assert_eq!(output.document.frontend, FrontendId::Markdown);
        assert_eq!(
            output.document.metadata.get("title").unwrap().as_str(),
            Some("通知")
        );
        assert_eq!(output.document.blocks.len(), 2);
        assert!(matches!(
            output.document.blocks[0].kind,
            BlockKind::Heading { level: 1, .. }
        ));
        assert_eq!(output.document.blocks[0].source.file, "notice.md");
    }
}

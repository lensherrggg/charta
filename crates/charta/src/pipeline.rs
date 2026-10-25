//! End-to-end conversion pipeline.
//!
//! `frontend parse -> IR -> role validation -> backend schema validation ->
//! backend render -> report`. The pipeline never interprets document-standard
//! semantics.

use crate::backend::{Backend, RenderRequest};
use crate::frontend::{Frontend, ParseRequest};
use crate::ir::Document;
use crate::report::{IssueKind, Report, Severity};

use crate::headings;
use crate::roles::{self, RoleRegistry};
use crate::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub content: String,
}

impl SourceFile {
    pub fn new(path: impl Into<PathBuf>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
        }
    }

    pub fn read(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let content = std::fs::read_to_string(&path)?;
        Ok(Self { path, content })
    }

    pub fn display_name(&self) -> String {
        self.path.display().to_string()
    }
}

pub struct ConversionRequest<'a> {
    pub source: SourceFile,
    pub frontend: &'a dyn Frontend,
    pub backend: &'a dyn Backend,
    /// When true, contract-external syntax becomes blocking instead of a
    /// warning. Default is false (degrade + report).
    pub strict: bool,
}

pub struct ConversionOutput {
    /// The parsed IR, always available even when rendering was skipped.
    pub document: Document,
    /// `None` when blocking issues prevented rendering.
    pub docx: Option<Vec<u8>>,
    pub report: Report,
}

pub fn convert(request: ConversionRequest<'_>) -> Result<ConversionOutput> {
    let source_name = request.source.display_name();
    let parse_output = request.frontend.parse(ParseRequest {
        path: &request.source.path,
        source: &request.source.content,
    })?;

    let mut document = parse_output.document;
    let mut issues = parse_output.issues;

    let (declarations, role_issues) = roles::declarations_from_metadata(&document.metadata);
    document.declared_roles = declarations;
    issues.extend(role_issues);
    issues.extend(headings::apply_document_numbering(&mut document));

    let mut registry = RoleRegistry::new();
    for declaration in request.backend.roles() {
        registry.register(declaration);
    }
    issues.extend(roles::validate_roles(&document, &registry));

    if request.strict {
        for issue in &mut issues {
            if issue.kind() == IssueKind::UnsupportedSyntax {
                issue.severity = Severity::Blocking;
            }
        }
    }

    issues.extend(request.backend.schema().validate(&document.metadata));

    let mut report = Report::new(
        source_name,
        request.frontend.id().as_str(),
        request.backend.id(),
    );
    report.add_all(issues);

    let mut docx = None;
    if !report.has_blocking() {
        let rendered = request.backend.render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })?;
        docx = Some(rendered.docx);
        report.add_all(rendered.issues);
    }

    Ok(ConversionOutput {
        document,
        docx,
        report: report.finish(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::RenderOutput;
    use crate::frontend::ParseOutput;
    use crate::ir::{Block, BlockKind, FrontendId, Inline, Metadata, SourceSpan};
    use crate::report::{Issue, IssueDetails};
    use crate::schema::{FieldSchema, MetadataSchema};

    struct FakeFrontend {
        issues: Vec<Issue>,
        metadata: Metadata,
    }

    impl Frontend for FakeFrontend {
        fn id(&self) -> FrontendId {
            FrontendId::Markdown
        }

        fn display_name(&self) -> &str {
            "fake"
        }

        fn parse(&self, request: ParseRequest<'_>) -> Result<ParseOutput> {
            let span = SourceSpan::point(request.source_name(), 1, 1, 0);
            let document =
                Document::new(FrontendId::Markdown, self.metadata.clone()).with_blocks(vec![
                    Block::paragraph(vec![Inline::text("正文", span.clone())], span),
                ]);
            Ok(ParseOutput::with_issues(document, self.issues.clone()))
        }
    }

    struct FakeBackend {
        schema: MetadataSchema,
        rendered: std::sync::atomic::AtomicUsize,
    }

    impl FakeBackend {
        fn new() -> Self {
            Self {
                schema: MetadataSchema::new(vec![FieldSchema::string("title", true)]),
                rendered: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl Backend for FakeBackend {
        fn id(&self) -> &'static str {
            "fake"
        }

        fn schema(&self) -> &MetadataSchema {
            &self.schema
        }

        fn render(&self, request: RenderRequest<'_>) -> Result<RenderOutput> {
            self.rendered
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let span = SourceSpan::point("fake", 1, 1, 0);
            let mut issues = Vec::new();
            if let BlockKind::Paragraph { content } = &request.document.blocks[0].kind {
                for inline in content {
                    for intent in &inline.intents {
                        issues.push(Issue::style_overridden(
                            "覆盖样式",
                            intent.property.name(),
                            intent.property.value_display(),
                            "默认",
                            "未启用采纳",
                            Some(span.clone()),
                        ));
                    }
                }
            }
            Ok(RenderOutput::with_issues(b"docx".to_vec(), issues))
        }
    }

    #[test]
    fn blocking_schema_errors_skip_rendering() {
        let backend = FakeBackend::new();
        let output = convert(ConversionRequest {
            source: SourceFile::new("in.md", "# hi"),
            frontend: &FakeFrontend {
                issues: vec![],
                metadata: Metadata::new(),
            },
            backend: &backend,
            strict: false,
        })
        .unwrap();
        assert!(output.docx.is_none());
        assert!(output.report.has_blocking());
        assert_eq!(
            backend.rendered.load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }

    struct AlternateBackend {
        schema: MetadataSchema,
    }

    impl Backend for AlternateBackend {
        fn id(&self) -> &'static str {
            "alternate"
        }

        fn schema(&self) -> &MetadataSchema {
            &self.schema
        }

        fn render(&self, _request: RenderRequest<'_>) -> Result<RenderOutput> {
            Ok(RenderOutput::new(b"alternate-docx".to_vec()))
        }
    }

    #[test]
    fn the_same_pipeline_switches_backends_without_core_changes() {
        let mut metadata = Metadata::new();
        metadata.insert("title", "x");
        metadata.insert("alt", "y");
        let frontend = FakeFrontend {
            issues: vec![],
            metadata,
        };
        let primary = FakeBackend::new();
        let alternate = AlternateBackend {
            schema: MetadataSchema::new(vec![FieldSchema::string("alt", true)]),
        };

        let first = convert(ConversionRequest {
            source: SourceFile::new("in.md", "text"),
            frontend: &frontend,
            backend: &primary,
            strict: false,
        })
        .unwrap();
        let second = convert(ConversionRequest {
            source: SourceFile::new("in.md", "text"),
            frontend: &frontend,
            backend: &alternate,
            strict: false,
        })
        .unwrap();

        assert_eq!(first.docx.as_deref(), Some(b"docx".as_slice()));
        assert_eq!(second.docx.as_deref(), Some(b"alternate-docx".as_slice()));
        assert_eq!(first.report.backend, "fake");
        assert_eq!(second.report.backend, "alternate");
        assert!(!first.report.has_blocking());
        assert!(!second.report.has_blocking());
    }

    #[test]
    fn strict_mode_escalates_unsupported_syntax() {
        let backend = FakeBackend::new();
        let mut metadata = Metadata::new();
        metadata.insert("title", "x");
        let frontend = FakeFrontend {
            issues: vec![Issue::unsupported("不支持", "x", "降级", None)],
            metadata,
        };
        let output = convert(ConversionRequest {
            source: SourceFile::new("in.md", "# hi"),
            frontend: &frontend,
            backend: &backend,
            strict: true,
        })
        .unwrap();
        assert!(output.docx.is_none());
        assert!(matches!(
            output.report.issues[0].details,
            IssueDetails::UnsupportedSyntax { .. }
        ));
    }
}

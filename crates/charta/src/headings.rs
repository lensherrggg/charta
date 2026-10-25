//! Engine-control metadata for heading numbering suggestions.
//!
//! `_charta.heading_numbering: true|false` sets the document-level default on
//! every heading that does not already carry a frontend suggestion.

use crate::ir::{Block, BlockKind, Document, MetaValue};
use crate::report::Issue;

pub(crate) const CONTROL_KEY: &str = "heading_numbering";

pub(crate) fn apply_document_numbering(document: &mut Document) -> Vec<Issue> {
    let mut issues = Vec::new();
    let Some(control) = document.metadata.get("_charta") else {
        return issues;
    };
    let Some(control) = control.as_map() else {
        return issues;
    };
    let Some(value) = control.get(CONTROL_KEY) else {
        return issues;
    };
    let MetaValue::Bool(value) = value else {
        issues.push(Issue::schema(
            "`_charta.heading_numbering` 必须是布尔值",
            "_charta.heading_numbering",
            "boolean",
            value.type_name(),
            None,
        ));
        return issues;
    };
    apply_blocks(&mut document.blocks, *value);
    issues
}

fn apply_blocks(blocks: &mut [Block], suggestion: bool) {
    for block in blocks {
        match &mut block.kind {
            BlockKind::Heading { auto_number, .. } => {
                if auto_number.is_none() {
                    *auto_number = Some(suggestion);
                }
            }
            BlockKind::Quote { blocks } | BlockKind::Extended { blocks, .. } => {
                apply_blocks(blocks, suggestion);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Block, FrontendId, Inline, Metadata, SourceSpan};
    use crate::report::IssueKind;
    use std::collections::BTreeMap;

    fn span() -> SourceSpan {
        SourceSpan::point("notice.md", 1, 1, 0)
    }

    fn document(metadata: Metadata, blocks: Vec<Block>) -> Document {
        Document::new(FrontendId::Markdown, metadata).with_blocks(blocks)
    }

    fn metadata_with_heading_numbering(value: MetaValue) -> Metadata {
        let mut control = BTreeMap::new();
        control.insert(CONTROL_KEY.to_owned(), value);
        let mut metadata = Metadata::new();
        metadata.insert("_charta", MetaValue::Map(control));
        metadata
    }

    fn heading_auto(block: &Block) -> Option<bool> {
        match &block.kind {
            BlockKind::Heading { auto_number, .. } => *auto_number,
            _ => panic!("not a heading"),
        }
    }

    #[test]
    fn applies_document_default_to_nested_headings() {
        let nested = Block::quote(
            vec![Block::heading(
                2,
                vec![Inline::text("引用标题", span())],
                span(),
            )],
            span(),
        );
        let blocks = vec![
            Block::heading(1, vec![Inline::text("标题", span())], span()),
            nested,
        ];
        let mut document = document(
            metadata_with_heading_numbering(MetaValue::Bool(true)),
            blocks,
        );

        let issues = apply_document_numbering(&mut document);

        assert!(issues.is_empty());
        assert_eq!(heading_auto(&document.blocks[0]), Some(true));
        let BlockKind::Quote { blocks } = &document.blocks[1].kind else {
            panic!("expected quote");
        };
        assert_eq!(heading_auto(&blocks[0]), Some(true));
    }

    #[test]
    fn explicit_frontend_suggestion_wins() {
        let blocks = vec![
            Block::heading(1, vec![Inline::text("标题", span())], span()).with_auto_number(true),
        ];
        let mut document = document(
            metadata_with_heading_numbering(MetaValue::Bool(false)),
            blocks,
        );

        apply_document_numbering(&mut document);

        assert_eq!(heading_auto(&document.blocks[0]), Some(true));
    }

    #[test]
    fn rejects_non_boolean_setting() {
        let mut document = document(
            metadata_with_heading_numbering(MetaValue::String("yes".to_owned())),
            vec![Block::heading(
                1,
                vec![Inline::text("标题", span())],
                span(),
            )],
        );

        let issues = apply_document_numbering(&mut document);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].kind(), IssueKind::SchemaValidation);
    }
}

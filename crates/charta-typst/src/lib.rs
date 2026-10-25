//! Typst frontend for Charta.
//!
//! The frontend reads a span-preserving subset of Typst's syntax tree. It never
//! evaluates code. Headings, nested lists, inline strong/emphasis (markup or
//! `#strong`/`#emph`), literal images and figures, literal tables, `#text(...)`
//! style intents and literal `include "file.typ"` statements are extracted;
//! loops, conditionals, `import`, `#set`, `#show` and unknown calls are
//! degraded and reported with source locations.
//!
//! Charta-specific extension: metadata is read from a top-level Typst literal
//! binding, `#let charta-meta = (...)`. A `#let` produces no output, so the
//! document remains compilable by Typst itself. YAML front matter is not
//! supported in Typst files.

mod blocks;
mod inline;
mod metadata;
mod source;
mod syntax;

use charta::{Document, Frontend, FrontendId, ParseOutput, ParseRequest, Result};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct TypstFrontend {
    max_include_depth: usize,
}

impl Default for TypstFrontend {
    fn default() -> Self {
        Self::new()
    }
}

impl TypstFrontend {
    pub fn new() -> Self {
        Self {
            max_include_depth: 16,
        }
    }

    pub fn with_max_include_depth(mut self, depth: usize) -> Self {
        self.max_include_depth = depth.max(1);
        self
    }

    /// Parse a Typst string. `path` is used for spans and for resolving
    /// relative includes.
    pub fn parse_source(&self, source: &str, path: impl AsRef<Path>) -> ParseOutput {
        let parsed = blocks::parse_main(path.as_ref(), source, self.max_include_depth);
        let document = Document::new(FrontendId::Typst, parsed.metadata).with_blocks(parsed.blocks);
        ParseOutput::with_issues(document, parsed.issues)
    }
}

impl Frontend for TypstFrontend {
    fn id(&self) -> FrontendId {
        FrontendId::Typst
    }

    fn display_name(&self) -> &str {
        "typst"
    }

    fn parse(&self, request: ParseRequest<'_>) -> Result<ParseOutput> {
        Ok(self.parse_source(request.source, request.path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use charta::{BlockKind, Determinism, IssueDetails, IssueKind, StyleProperty};
    use std::fs;
    use std::path::PathBuf;

    fn parse(source: &str) -> ParseOutput {
        TypstFrontend::new().parse_source(source, "sample.typ")
    }

    #[test]
    fn parses_headings_text_and_style_intents() {
        let output = parse(
            "#let charta-meta = (\n  title: \"测试通知\",\n  date: \"2026-10-25\",\n)\n\n= 一级标题\n\n这是 #text(font: \"SimSun\", size: 14pt, fill: red)[重点] 内容。\n",
        );
        assert!(output.issues.is_empty(), "{:?}", output.issues);
        assert_eq!(
            output.document.metadata.get("title").unwrap().as_str(),
            Some("测试通知")
        );
        assert_eq!(output.document.blocks.len(), 2);
        assert!(matches!(
            output.document.blocks[0].kind,
            BlockKind::Heading { level: 1, .. }
        ));
        let BlockKind::Paragraph { content } = &output.document.blocks[1].kind else {
            panic!("expected paragraph")
        };
        let run = content
            .iter()
            .find(|run| run.text == "重点")
            .expect("styled run");
        assert_eq!(run.intents.len(), 3);
        assert!(run.intents.iter().any(|intent| matches!(
            &intent.property,
            StyleProperty::Color { hex } if hex == "#FF0000"
        )));
        assert!(run.intents.iter().any(|intent| matches!(
            &intent.property,
            StyleProperty::FontSize { points } if (*points - 14.0).abs() < f64::EPSILON
        )));
        assert!(run.intents.iter().any(|intent| matches!(
            &intent.property,
            StyleProperty::FontFamily { value } if value == "SimSun"
        )));
    }

    #[test]
    fn reads_typst_native_metadata() {
        let output = parse(
            "#let charta-meta = (\n  title: \"测试通知\",\n  author: \"张三\",\n  date: \"2026-10-25\",\n  _charta: (heading_numbering: true),\n)\n\n= 正文\n",
        );
        assert!(output.issues.is_empty(), "{:?}", output.issues);
        assert_eq!(
            output.document.metadata.get("title").unwrap().as_str(),
            Some("测试通知")
        );
        assert_eq!(
            output
                .document
                .metadata
                .get("_charta")
                .unwrap()
                .as_map()
                .unwrap()
                .get("heading_numbering"),
            Some(&charta::MetaValue::Bool(true))
        );
        assert_eq!(output.document.blocks.len(), 1);
        assert!(matches!(
            output.document.blocks[0].kind,
            BlockKind::Heading { .. }
        ));
    }

    #[test]
    fn reports_legacy_typst_front_matter() {
        let output = parse("---\ntitle: 测试\n---\n\n= 正文\n");
        assert!(output.issues.iter().any(|issue| {
            issue.message.contains("YAML front matter") && issue.message.contains("charta-meta")
        }));
        assert!(output.document.metadata.is_empty());
        assert_eq!(output.document.blocks.len(), 1);
    }

    #[test]
    fn expands_literal_include_recursively() {
        let dir = temp_dir("include");
        fs::create_dir_all(&dir).unwrap();
        let main_path = dir.join("main.typ");
        let included_path = dir.join("part.typ");
        fs::write(&included_path, "= 子标题\n\n来自 include 的正文。\n").unwrap();
        let main = "#include \"part.typ\"\n\n= 主标题\n";
        fs::write(&main_path, main).unwrap();

        let output = TypstFrontend::new().parse_source(main, &main_path);
        assert!(output.issues.is_empty(), "{:?}", output.issues);
        assert_eq!(output.document.blocks.len(), 3);
        assert!(matches!(
            output.document.blocks[0].kind,
            BlockKind::Heading { level: 1, .. }
        ));
        assert!(output.document.blocks[0].source.file.ends_with("part.typ"));
        assert!(matches!(
            output.document.blocks[1].kind,
            BlockKind::Paragraph { .. }
        ));
        assert!(output.document.blocks[1].source.file.ends_with("part.typ"));
        assert!(output.document.blocks[2].source.file.ends_with("main.typ"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detects_include_cycles_without_hanging() {
        let dir = temp_dir("include-cycle");
        fs::create_dir_all(&dir).unwrap();
        let main_path = dir.join("main.typ");
        let other_path = dir.join("other.typ");
        let main = "#include \"other.typ\"\n";
        fs::write(&main_path, main).unwrap();
        fs::write(&other_path, "#include \"main.typ\"\n").unwrap();

        let output = TypstFrontend::new().parse_source(main, &main_path);
        assert!(output
            .issues
            .iter()
            .any(|issue| { issue.is_blocking() && issue.message.contains("循环 include") }));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reports_loops_and_conditionals_with_locations() {
        let output = parse(
            "= 标题\n\n#for item in items [循环内文字]\n\n#while true [循环二]\n\n#if cond [条件文字] else [否则文字]\n",
        );
        let nondeterministic: Vec<_> = output
            .issues
            .iter()
            .filter(|issue| issue.kind() == IssueKind::NonDeterministic)
            .collect();
        assert_eq!(nondeterministic.len(), 3);
        let first = nondeterministic[0];
        let location = first.location.as_ref().unwrap();
        assert_eq!(location.file, "sample.typ");
        assert_eq!(location.line, 3);
        assert!(matches!(
            &first.details,
            IssueDetails::NonDeterministic { reason, impact }
                if reason.contains("循环") && impact.contains("重复次数")
        ));
        let loops: Vec<_> = output
            .document
            .blocks
            .iter()
            .filter(|block| matches!(block.determinism, Determinism::NonDeterministic { .. }))
            .collect();
        assert_eq!(loops.len(), 3);
        assert!(output.document.blocks.iter().any(|block| {
            matches!(&block.kind, BlockKind::Paragraph { content }
                if content.iter().any(|run| run.text.contains("循环内文字")))
        }));
        assert!(output.document.blocks.iter().any(|block| {
            matches!(&block.kind, BlockKind::Paragraph { content }
                if content.iter().any(|run| run.text.contains("条件文字")))
        }));
    }

    #[test]
    fn reports_set_show_import_and_unknown_calls() {
        let output = parse(
            "#set text(font: \"SimSun\")\n\n#show heading: it => it\n\n#import \"lib.typ\": *\n\n#link(\"https://example.com\")[链接]\n",
        );
        let reasons: Vec<_> = output
            .issues
            .iter()
            .filter_map(|issue| match &issue.details {
                IssueDetails::NonDeterministic { reason, .. } => Some(reason.as_str()),
                _ => None,
            })
            .collect();
        assert!(reasons.iter().any(|reason| reason.contains("#set text")));
        assert!(reasons.iter().any(|reason| reason.contains("show")));
        assert!(reasons.iter().any(|reason| reason.contains("import")));
        assert!(output
            .issues
            .iter()
            .any(|issue| issue.message.contains("不支持的 Typst 函数")));
        assert!(output.document.blocks.iter().any(|block| {
            matches!(&block.kind, BlockKind::Paragraph { content }
                if content.iter().any(|run| run.text.contains("链接")))
        }));
    }

    #[test]
    fn reports_paren_and_brace_code_expressions() {
        let output = parse("开头 #(1 + 2) 中间 #{calc.add(1, 2)} 结尾\n");
        let messages: Vec<_> = output
            .issues
            .iter()
            .map(|issue| issue.message.as_str())
            .collect();
        assert!(messages
            .iter()
            .any(|message| message.contains("`#{...}` / `#(...)`")));
        assert_eq!(
            output
                .issues
                .iter()
                .filter(|issue| issue.message.contains("代码表达式"))
                .count(),
            2
        );
        let BlockKind::Paragraph { content } = &output.document.blocks[0].kind else {
            panic!("expected paragraph")
        };
        let text: String = content.iter().map(|run| run.text.as_str()).collect();
        assert!(text.contains("开头"));
        assert!(text.contains("结尾"));
        assert!(!text.contains("calc.add"));
        assert!(matches!(
            output.document.blocks[0].determinism,
            Determinism::NonDeterministic { .. }
        ));
    }

    #[test]
    fn keeps_content_argument_of_unknown_functions() {
        let output = parse("前 #highlight[强调文字] 后\n");
        assert!(output
            .issues
            .iter()
            .any(|issue| issue.message.contains("不支持的 Typst 函数")));
        let BlockKind::Paragraph { content } = &output.document.blocks[0].kind else {
            panic!("expected paragraph")
        };
        let text: String = content.iter().map(|run| run.text.as_str()).collect();
        assert!(text.contains("强调文字"));
        assert!(matches!(
            output.document.blocks[0].determinism,
            Determinism::NonDeterministic { .. }
        ));
    }

    #[test]
    fn parses_flat_and_nested_lists() {
        let output = parse(
            "= 清单\n\n- 第一项\n- 第二项 *重点*\n  - 子项一\n  - 子项二\n\n+ 有序一\n+ 有序二\n",
        );
        assert!(output.issues.is_empty(), "{:?}", output.issues);
        assert_eq!(output.document.blocks.len(), 3);
        let BlockKind::List(list) = &output.document.blocks[1].kind else {
            panic!("expected unordered list")
        };
        assert!(!list.ordered);
        assert_eq!(list.items.len(), 2);
        assert!(list.items[1]
            .content
            .iter()
            .any(|run| run.strong && run.text == "重点"));
        assert_eq!(list.items[1].children.len(), 1);
        assert_eq!(list.items[1].children[0].items.len(), 2);
        assert_eq!(list.items[1].children[0].items[0].content[0].text, "子项一");
        let BlockKind::List(ordered) = &output.document.blocks[2].kind else {
            panic!("expected ordered list")
        };
        assert!(ordered.ordered);
        assert_eq!(ordered.start, 1);
        assert_eq!(ordered.items.len(), 2);
    }

    #[test]
    fn parses_standalone_images_and_figure_captions() {
        let output = parse(
            "#image(\"images/a.png\", alt: \"示意图\")\n\n#figure(image(\"images/b.png\"), caption: [图 *一* 说明])\n\n#image(\"images/c.png\", caption: [单行题注])\n",
        );
        assert!(output.issues.is_empty(), "{:?}", output.issues);
        let BlockKind::Image(image) = &output.document.blocks[0].kind else {
            panic!("expected image")
        };
        assert_eq!(image.path, "images/a.png");
        assert_eq!(image.alt.as_deref(), Some("示意图"));
        assert!(image.caption.is_none());
        let BlockKind::Image(figure) = &output.document.blocks[1].kind else {
            panic!("expected figure image")
        };
        assert_eq!(figure.path, "images/b.png");
        let caption = figure.caption.as_ref().expect("caption");
        assert!(caption.iter().any(|run| run.strong && run.text == "一"));
        let BlockKind::Image(direct) = &output.document.blocks[2].kind else {
            panic!("expected direct-caption image")
        };
        assert_eq!(
            direct
                .caption
                .as_ref()
                .map(|caption| caption[0].text.as_str()),
            Some("单行题注")
        );
    }

    #[test]
    fn reports_non_literal_image_paths() {
        let output = parse("#image(path)\n");
        assert!(output
            .issues
            .iter()
            .any(|issue| issue.message.contains("路径不是字面量")));
        assert!(output.document.blocks.is_empty());
    }

    #[test]
    fn parses_tables_with_headers_spans_and_captions() {
        let output = parse(
            "#table(\n  columns: 2,\n  table.header[*名称*][*数量*],\n  [甲], [1],\n  [乙], [2],\n)\n\n#figure(table(columns: 3, table.cell(colspan: 2)[合并], [三]), caption: [表说明])\n",
        );
        assert!(output.issues.is_empty(), "{:?}", output.issues);
        let BlockKind::Table(table) = &output.document.blocks[0].kind else {
            panic!("expected table")
        };
        assert_eq!(table.column_count(), 2);
        assert_eq!(table.header.len(), 1);
        assert_eq!(table.header[0].cells.len(), 2);
        assert!(table.header[0].cells[0]
            .content
            .iter()
            .any(|run| run.strong));
        assert_eq!(table.rows.len(), 2);
        let BlockKind::Table(figure) = &output.document.blocks[1].kind else {
            panic!("expected figure table")
        };
        assert!(figure.caption.is_some());
        assert_eq!(figure.rows[0].cells.len(), 2);
        assert_eq!(figure.rows[0].cells[0].colspan, 2);
        assert_eq!(figure.column_count(), 3);
    }

    #[test]
    fn reports_dynamic_table_arguments() {
        let output = parse("#table(columns: cols, [甲], [乙])\n");
        assert!(output
            .issues
            .iter()
            .any(|issue| issue.message.contains("无法静态解析 `columns`")));
        assert!(output
            .issues
            .iter()
            .any(|issue| issue.message.contains("未指定 `columns`")));
        let BlockKind::Table(table) = &output.document.blocks[0].kind else {
            panic!("expected fallback table")
        };
        assert_eq!(table.column_count(), 1);
    }

    #[test]
    fn reports_content_block_expression() {
        let output = parse("内容 #[注入文字] 结束\n");
        assert!(output
            .issues
            .iter()
            .any(|issue| issue.message.contains("内容块表达式")));
        let BlockKind::Paragraph { content } = &output.document.blocks[0].kind else {
            panic!("expected paragraph")
        };
        let text: String = content.iter().map(|run| run.text.as_str()).collect();
        assert!(text.contains("注入文字"));
        assert!(matches!(
            output.document.blocks[0].determinism,
            Determinism::NonDeterministic { .. }
        ));
    }

    #[test]
    fn strips_comments_but_keeps_urls() {
        let output =
            parse("// 这是注释\n= 标题 // 行尾注释\n\n访问 https://example.com/a 获取详情\n");
        assert!(output.issues.is_empty(), "{:?}", output.issues);
        let BlockKind::Paragraph { content } = &output.document.blocks[1].kind else {
            panic!("expected paragraph")
        };
        let text: String = content.iter().map(|run| run.text.as_str()).collect();
        assert!(text.contains("https://example.com/a"));
        assert!(!text.contains("注释"));
    }

    #[test]
    fn reports_unsupported_syntax() {
        let output = parse("$ x + y $\n\n```\nraw\n```\n");
        assert!(output
            .issues
            .iter()
            .any(|issue| issue.message.contains("数学公式")));
        assert!(output
            .issues
            .iter()
            .any(|issue| issue.message.contains("原始块")));
    }

    #[test]
    fn missing_include_is_blocking() {
        let output = parse("#include \"missing-file.typ\"\n");
        assert!(output.report_has_blocking());
    }

    trait BlockingReport {
        fn report_has_blocking(&self) -> bool;
    }

    impl BlockingReport for ParseOutput {
        fn report_has_blocking(&self) -> bool {
            self.issues.iter().any(|issue| issue.is_blocking())
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("charta-typst-test-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        dir
    }
}

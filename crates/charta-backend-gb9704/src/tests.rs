use crate::numbering::list_marker;
use crate::styles::{Gb9704Styles, StyleSet, STYLE_BODY, STYLE_STRONG, STYLE_TITLE};
use crate::{Gb9704Backend, Gb9704Options, GeneralBackend};
use charta::{
    Alignment, Backend, Block, Determinism, Document, FrontendId, ImageBlock, Inline, IssueDetails,
    IssueKind, ListBlock, ListItem, MetaValue, Metadata, RenderRequest, SourceSpan, StyleIntent,
    StyleProperty, TableBlock, TableCell, TableRow,
};
use std::io::Read;

fn span() -> SourceSpan {
    SourceSpan::point("notice.md", 1, 1, 0)
}

fn intent(property: StyleProperty) -> StyleIntent {
    StyleIntent {
        property,
        frontend: FrontendId::Typst,
        source: span(),
    }
}

fn metadata() -> Metadata {
    let mut metadata = Metadata::new();
    metadata.insert("title", "关于开展 2026 年度公文格式检查的通知");
    metadata.insert("doc_number", "昆政发〔2026〕1号");
    metadata.insert("issuer", "昆山市人民政府");
    metadata.insert("date", "2026-10-25");
    metadata.insert("organization", "昆山市人民政府文件");
    metadata.insert("recipient", "各区镇人民政府");
    metadata.insert(
        "cc",
        MetaValue::List(vec![
            MetaValue::String("市委办公室".to_owned()),
            MetaValue::String("市人大常委会办公室".to_owned()),
        ]),
    );
    metadata.insert("issuing_office", "昆山市人民政府办公室");
    metadata.insert("issuing_date", "2026-10-26");
    metadata.insert("security_level", "秘密");
    metadata.insert("urgency", "特急");
    metadata.insert(
        "attachments",
        MetaValue::List(vec![MetaValue::String("格式检查表".to_owned())]),
    );
    metadata.insert("note", "联系人：张三");
    metadata
}

fn document() -> Document {
    let colored = Inline::text("高度重视", span())
        .with_intent(intent(StyleProperty::Color {
            hex: "#FF0000".to_owned(),
        }))
        .with_intent(intent(StyleProperty::FontFamily {
            value: "Arial".to_owned(),
        }))
        .with_intent(intent(StyleProperty::FontSize { points: 12.0 }))
        .with_intent(intent(StyleProperty::Bold { value: true }));

    let list = ListBlock::ordered(
        1,
        vec![
            ListItem::new(vec![Inline::text("第一项", span())]),
            ListItem {
                content: vec![Inline::text("第二项", span())],
                children: vec![ListBlock::unordered(vec![ListItem::new(vec![
                    Inline::text("子项", span()),
                ])])],
            },
        ],
    );

    let table = TableBlock {
        header: vec![TableRow::new(vec![
            TableCell::new(vec![Inline::text("名称", span())]),
            TableCell::new(vec![Inline::text("数量", span())]),
        ])],
        rows: vec![TableRow::new(vec![
            TableCell::new(vec![Inline::text("甲", span())]),
            TableCell::new(vec![Inline::text("1", span())]),
        ])],
        column_alignments: vec![Alignment::Left, Alignment::Right],
        caption: None,
    };

    let blocks = vec![
        Block::heading(1, vec![Inline::text("一、总体要求", span())], span()),
        Block::paragraph(
            vec![
                Inline::text("请各单位", span()),
                colored,
                Inline::text("，认真组织实施。", span()),
            ],
            span(),
        ),
        Block::list(list, span()),
        Block::table(table, span()),
        Block::image(
            ImageBlock {
                path: "images/check.png".to_owned(),
                alt: Some("检查表".to_owned()),
                title: None,
                caption: None,
            },
            span(),
        ),
        Block::quote(
            vec![Block::paragraph(
                vec![Inline::text("引用说明", span())],
                span(),
            )],
            span(),
        ),
        Block::paragraph(vec![Inline::text("动态生成的内容", span())], span()).with_determinism(
            Determinism::NonDeterministic {
                reason: "包含 #for 循环".to_owned(),
                impact: "循环体按字面量提取一次".to_owned(),
            },
        ),
    ];
    Document::new(FrontendId::Markdown, metadata()).with_blocks(blocks)
}

fn part(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut file = archive.by_name(name).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    contents
}

fn body_text(xml: &str) -> String {
    let mut text = String::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<w:t") {
        let after = &rest[start..];
        let Some(tag_end) = after.find('>') else {
            break;
        };
        let content_start = start + tag_end + 1;
        let Some(content_end) = rest[content_start..].find("</w:t>") else {
            break;
        };
        text.push_str(&rest[content_start..content_start + content_end]);
        rest = &rest[content_start + content_end + "</w:t>".len()..];
    }
    text
}

#[test]
fn renders_complete_notice() {
    let document = document();
    let backend = Gb9704Backend::new();
    let output = backend
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();

    assert!(output.docx.starts_with(b"PK"));
    let body = part(&output.docx, "word/document.xml");
    let styles = part(&output.docx, "word/styles.xml");
    let footer = part(&output.docx, "word/footer1.xml");
    let even_footer = part(&output.docx, "word/footer2.xml");
    let settings = part(&output.docx, "word/settings.xml");

    assert!(body.contains("昆山市人民政府文件"));
    assert!(body.contains("昆政发〔2026〕1号"));
    assert!(body.contains("关于开展 2026 年度公文格式检查的通知"));
    assert!(body.contains("各区镇人民政府："));
    assert!(body.contains("秘密"));
    assert!(body.contains("特急"));
    assert!(body.contains("抄送：市委办公室、市人大常委会办公室"));
    assert!(body.contains("2026年10月25日"));
    assert!(body.contains("2026年10月26日印发"));
    assert!(body.contains("联系人"));
    assert!(body.contains("C00000"));
    assert!(body.contains("FF0000"));
    assert!(!body.contains("Arial"));
    assert!(body.contains("w:top=\"2098\""));
    assert!(body.contains("w:bottom=\"1984\""));
    assert!(body.contains("w:left=\"1587\""));
    assert!(body.contains("w:right=\"1474\""));
    assert!(body.contains("<w:tbl>"));
    assert!(body.contains("<w:type w:val=\"nextPage\"/>"));
    assert!(body.contains("<w:vAlign w:val=\"bottom\"/>"));
    assert!(!body.contains("<w:tblpPr"));
    assert!(!body.contains("<w:numPr"));
    assert!(!body.contains('•'));
    let text = body_text(&body);
    assert!(text.contains("（1）第一项"));
    assert!(text.contains("（2）第二项"));
    assert!(text.contains("①子项"));
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&output.docx)).unwrap();
    assert!(archive.by_name("word/numbering.xml").is_err());
    assert!(styles.contains("仿宋_GB2312"));
    assert!(styles.contains("方正小标宋简体"));
    assert!(styles.contains("w:line=\"560\""));
    assert!(styles.contains("w:sz w:val=\"32\""));
    assert_eq!(styles.matches("<w:keepNext/>").count(), 4);
    assert!(footer.contains("PAGE"));
    assert!(even_footer.contains("PAGE"));
    assert!(settings.contains("evenAndOddHeaders"));

    let overrides: Vec<_> = output
        .issues
        .iter()
        .filter(|issue| issue.kind() == IssueKind::StyleOverridden)
        .collect();
    assert_eq!(overrides.len(), 3);
    assert!(overrides.iter().any(|issue| matches!(
        &issue.details,
        IssueDetails::StyleOverridden { property, .. } if property == "font-family"
    )));
    assert!(overrides.iter().any(|issue| matches!(
        &issue.details,
        IssueDetails::StyleOverridden { property, .. } if property == "font-size"
    )));
    assert!(overrides.iter().any(|issue| matches!(
        &issue.details,
        IssueDetails::StyleOverridden { property, .. } if property == "bold"
    )));
    assert!(!overrides.iter().any(|issue| matches!(
        &issue.details,
        IssueDetails::StyleOverridden { property, .. } if property == "color"
    )));

    assert!(output.issues.iter().any(|issue| matches!(
        &issue.details,
        IssueDetails::RenderWarning { code, .. } if code == "image_read_failed"
    )));
    assert!(output.issues.iter().any(|issue| matches!(
        &issue.details,
        IssueDetails::RenderWarning { code, .. } if code == "degraded_content"
    )));
}

const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0xFC, 0xCF, 0xC0, 0x50,
    0x0F, 0x00, 0x05, 0x83, 0x02, 0x7F, 0x94, 0xAD, 0xD0, 0xEB, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

#[test]
fn embeds_images_into_the_package() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-artifacts");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("charta-image-test.png");
    std::fs::write(&path, PNG_1X1).unwrap();

    let image = ImageBlock {
        path: path.display().to_string(),
        alt: Some("示意图".to_owned()),
        title: None,
        caption: None,
    };
    let document = Document::new(FrontendId::Markdown, metadata())
        .with_blocks(vec![Block::image(image, span())]);
    let output = Gb9704Backend::new()
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    let body = part(&output.docx, "word/document.xml");
    assert!(body.contains("<w:drawing>"));
    assert!(body.contains("r:embed="));
    assert!(body.contains("示意图"));
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&output.docx)).unwrap();
    assert!(archive.by_name("word/media/image1.png").is_ok());
    drop(archive);
    let content_types = part(&output.docx, "[Content_Types].xml");
    assert!(content_types.contains("image/png"));
    assert!(!output.issues.iter().any(|issue| matches!(
        &issue.details,
        IssueDetails::RenderWarning { code, .. }
            if code == "image_read_failed" || code == "image_decode_failed"
    )));

    std::fs::remove_file(&path).ok();
}

#[test]
fn image_captions_are_numbered_and_reported() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-artifacts");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("charta-caption-test.png");
    std::fs::write(&path, PNG_1X1).unwrap();
    let path = path.display().to_string();
    let image = |text: Option<&str>| {
        Block::image(
            ImageBlock {
                path: path.clone(),
                alt: None,
                title: None,
                caption: text.map(|text| vec![Inline::text(text, span())]),
            },
            span(),
        )
    };
    let document = Document::new(FrontendId::Markdown, metadata()).with_blocks(vec![
        image(Some("第一图")),
        image(None),
        image(Some("第二图")),
    ]);
    let output = Gb9704Backend::new()
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    let body = part(&output.docx, "word/document.xml");
    let text = body_text(&body);
    assert!(text.contains("图 1　第一图"));
    assert!(text.contains("图 2　第二图"));
    assert!(!text.contains("图 3"));
    assert!(body.contains("GbImageCaption"));
    assert_eq!(
        output
            .issues
            .iter()
            .filter(|issue| matches!(
                &issue.details,
                IssueDetails::RenderWarning { code, .. } if code == "image_caption_static"
            ))
            .count(),
        1
    );

    std::fs::remove_file(&path).ok();
}

fn general_metadata() -> Metadata {
    let mut metadata = Metadata::new();
    metadata.insert("title", "关于做好近期重点工作的通知");
    metadata.insert("author", "张三");
    metadata.insert("date", "2026-10-25");
    metadata.insert("recipient", "各部门");
    metadata
}

fn style_block<'a>(styles: &'a str, id: &str) -> &'a str {
    let marker = format!("w:styleId=\"{id}\"");
    let start = styles.find(&marker).expect("style exists");
    let tail = &styles[start..];
    let end = tail.find("</w:style>").expect("style ends");
    &tail[..end]
}

#[test]
fn general_template_renders_title_byline_and_recipient() {
    let document = Document::new(FrontendId::Markdown, general_metadata()).with_blocks(vec![
        Block::heading(1, vec![Inline::text("一、总体要求", span())], span()),
        Block::paragraph(
            vec![
                Inline::text("请各部门", span()),
                Inline::text("高度重视", span()).strong(),
                Inline::text("，", span()),
                Inline::text("抓紧落实", span()).emphasis(),
                Inline::text("。", span()),
            ],
            span(),
        ),
        Block::heading(2, vec![Inline::text("（一）加强统筹", span())], span()),
    ]);
    let output = GeneralBackend::new()
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    let body = part(&output.docx, "word/document.xml");
    let styles = part(&output.docx, "word/styles.xml");
    let text = body_text(&body);

    assert!(text.contains("张　三"));
    assert!(text.contains("（2026年10月25日）"));
    let name_at = body.find("张　三").unwrap();
    let break_at = body[name_at..].find("<w:br/>").unwrap() + name_at;
    let date_at = body.find("（2026年10月25日）").unwrap();
    assert!(name_at < break_at && break_at < date_at);
    assert!(text.contains("各部门："));
    assert!(!body.contains("抄送"));
    assert!(!body.contains("<w:type w:val=\"nextPage\"/>"));
    assert!(body.contains("方正楷体_GBK"));

    for font in [
        "方正小标宋_GBK",
        "方正黑体_GBK",
        "方正楷体_GBK",
        "方正仿宋_GBK",
    ] {
        assert!(styles.contains(font), "missing font {font}");
    }
    let heading2 = style_block(&styles, "GbHeading2");
    assert!(heading2.contains("方正楷体_GBK"));
    assert!(heading2.contains("w:b/>"));
    let heading3 = style_block(&styles, "GbHeading3");
    assert!(heading3.contains("方正仿宋_GBK"));
    assert!(heading3.contains("w:b w:val=\"0\""));
}

#[test]
fn general_recipient_is_optional() {
    let mut metadata = general_metadata();
    metadata.fields.remove("recipient");
    let document = Document::new(FrontendId::Markdown, metadata);
    let output = GeneralBackend::new()
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    let body = part(&output.docx, "word/document.xml");
    let text = body_text(&body);
    assert!(text.contains("张　三"));
    assert!(text.contains("（2026年10月25日）"));
    assert!(!text.contains("各部门："));
}

#[test]
fn general_schema_requires_author_and_date() {
    let mut metadata = general_metadata();
    metadata.fields.remove("author");
    metadata.fields.remove("date");
    let issues = GeneralBackend::new().schema().validate(&metadata);
    assert_eq!(issues.len(), 2);
}

#[test]
fn loads_general_style_data_from_an_external_file() {
    let styles =
        Gb9704Styles::from_external_for("data/general-styles.json", StyleSet::General).unwrap();
    let backend = GeneralBackend::with_styles(styles);
    assert_eq!(backend.styles().run(STYLE_TITLE).size_pt, 22.0);
    assert_eq!(backend.styles().run(STYLE_BODY).size_pt, 16.0);
}

#[test]
fn style_sets_are_not_interchangeable() {
    assert!(Gb9704Styles::from_path("data/general-styles.json").is_err());
    let notice_styles = Gb9704Styles::embedded().unwrap();
    assert!(GeneralBackend::try_with_styles(notice_styles).is_err());
}

#[test]
fn general_backend_loads_a_patch_file() {
    let path =
        std::env::temp_dir().join(format!("charta-general-patch-{}.json", std::process::id()));
    std::fs::write(
        &path,
        r#"{ "styles": { "GbBody": { "run": { "size_step": 1 } } } }"#,
    )
    .unwrap();
    let backend = GeneralBackend::with_style_file(&path).unwrap();
    assert_eq!(backend.styles().run(STYLE_BODY).size_pt, 18.0);
    std::fs::remove_file(&path).ok();
}

fn patch_general_styles(patch: &str) -> charta::Result<Gb9704Styles> {
    Gb9704Styles::from_json_with_patch(
        include_str!("../data/general-styles.json"),
        patch,
        StyleSet::General,
    )
}

#[test]
fn style_patch_adjusts_only_named_styles() {
    let styles = patch_general_styles(
        r#"{
          "styles": {
            "GbBody": { "run": { "size_step": 1 } },
            "GbStrong": { "run": { "east_asia_font": "方正黑体_GBK" } }
          }
        }"#,
    )
    .unwrap();
    assert_eq!(styles.run(STYLE_BODY).size_pt, 18.0);
    assert_eq!(styles.run(STYLE_TITLE).size_pt, 22.0);
    assert_eq!(styles.run(STYLE_STRONG).east_asia_font, "方正黑体_GBK");
    assert_eq!(styles.run(STYLE_STRONG).size_pt, 16.0);
}

#[test]
fn style_patch_rejects_unknown_or_conflicting_changes() {
    let out_of_range =
        patch_general_styles(r#"{ "styles": { "GbBody": { "run": { "size_step": 7 } } } }"#)
            .unwrap_err()
            .to_string();
    assert!(out_of_range.contains("超出中文字号阶梯"), "{out_of_range}");

    let conflict = patch_general_styles(
        r#"{ "styles": { "GbBody": { "run": { "size_step": 1, "size_pt": 18.0 } } } }"#,
    )
    .unwrap_err()
    .to_string();
    assert!(conflict.contains("不能同时指定"), "{conflict}");

    let unknown_style =
        patch_general_styles(r#"{ "styles": { "GbBoddy": { "run": { "size_step": 1 } } } }"#)
            .unwrap_err()
            .to_string();
    assert!(unknown_style.contains("未定义样式"), "{unknown_style}");

    let unknown_section = patch_general_styles(r#"{ "style": {} }"#)
        .unwrap_err()
        .to_string();
    assert!(unknown_section.contains("未知字段"), "{unknown_section}");
}

#[test]
fn style_patch_reaches_the_rendered_document() {
    let styles = patch_general_styles(
        r#"{
          "styles": {
            "GbBody": { "run": { "size_step": 1 } },
            "GbStrong": { "run": { "east_asia_font": "方正黑体_GBK" } }
          }
        }"#,
    )
    .unwrap();
    let backend = GeneralBackend::with_styles(styles);
    let document = Document::new(FrontendId::Markdown, general_metadata()).with_blocks(vec![
        Block::paragraph(
            vec![
                Inline::text("正文", span()),
                Inline::text("加粗", span()).strong(),
            ],
            span(),
        ),
    ]);
    let output = backend
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    let styles_xml = part(&output.docx, "word/styles.xml");
    let body_xml = part(&output.docx, "word/document.xml");
    assert!(style_block(&styles_xml, "GbBody").contains("w:sz w:val=\"36\""));
    assert!(style_block(&styles_xml, "GbTitle").contains("w:sz w:val=\"44\""));
    assert!(style_block(&styles_xml, "GbStrong").contains("方正黑体_GBK"));
    assert!(body_xml.contains("方正黑体_GBK"));
    assert!(body_xml.contains("w:sz w:val=\"36\""));
}

#[test]
fn two_character_names_get_one_full_width_space() {
    use crate::layout::body::spaced_name;
    assert_eq!(spaced_name("张三"), "张\u{3000}三");
    assert_eq!(spaced_name(" 张三 "), "张\u{3000}三");
    assert_eq!(spaced_name("王小明"), "王小明");
    assert_eq!(spaced_name("欧阳小明"), "欧阳小明");
}

#[test]
fn list_markers_cover_four_levels() {
    assert_eq!(list_marker(0, 1).as_deref(), Some("（1）"));
    assert_eq!(list_marker(0, 12).as_deref(), Some("（12）"));
    assert_eq!(list_marker(1, 1).as_deref(), Some("①"));
    assert_eq!(list_marker(1, 20).as_deref(), Some("⑳"));
    assert_eq!(list_marker(1, 21).as_deref(), Some("㉑"));
    assert_eq!(list_marker(1, 51).as_deref(), Some("(51)"));
    assert_eq!(list_marker(2, 1).as_deref(), Some("A."));
    assert_eq!(list_marker(2, 26).as_deref(), Some("Z."));
    assert_eq!(list_marker(2, 27).as_deref(), Some("AA."));
    assert_eq!(list_marker(3, 1).as_deref(), Some("a."));
    assert_eq!(list_marker(3, 26).as_deref(), Some("z."));
    assert_eq!(list_marker(3, 27).as_deref(), Some("aa."));
    assert_eq!(list_marker(4, 1), None);
}

fn nested_list(levels: u32) -> ListBlock {
    let mut list = ListBlock::unordered(vec![ListItem::new(vec![Inline::text(
        format!("第{levels}级"),
        span(),
    )])]);
    for level in (1..levels).rev() {
        list = ListBlock::unordered(vec![ListItem {
            content: vec![Inline::text(format!("第{level}级"), span())],
            children: vec![list],
        }]);
    }
    list
}

#[test]
fn lists_fall_back_to_body_at_fifth_level() {
    let list = nested_list(5);
    let document = Document::new(FrontendId::Markdown, metadata())
        .with_blocks(vec![Block::list(list, span())]);
    let output = Gb9704Backend::new()
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    let body = part(&output.docx, "word/document.xml");
    let text = body_text(&body);
    assert!(text.contains("（1）第1级"));
    assert!(text.contains("①第2级"));
    assert!(text.contains("A.第3级"));
    assert!(text.contains("a.第4级"));
    assert!(text.contains("第5级"));
    assert!(!text.contains("（1）第5级"));
    assert!(!text.contains("①第5级"));
    assert!(!text.contains("A.第5级"));
    assert!(!text.contains("a.第5级"));
    assert!(!body.contains("<w:numPr"));
    assert!(output.issues.iter().any(|issue| matches!(
        &issue.details,
        IssueDetails::RenderWarning { code, .. } if code == "list_nesting_flattened"
    )));
}

fn numbered_headings() -> Vec<Block> {
    vec![
        Block::heading(1, vec![Inline::text("范围", span())], span()).with_auto_number(true),
        Block::heading(2, vec![Inline::text("对象", span())], span()).with_auto_number(true),
        Block::heading(3, vec![Inline::text("细则", span())], span()).with_auto_number(true),
        Block::heading(4, vec![Inline::text("条目", span())], span()).with_auto_number(true),
        Block::heading(2, vec![Inline::text("再对象", span())], span()).with_auto_number(true),
        Block::heading(5, vec![Inline::text("五级", span())], span()).with_auto_number(true),
    ]
}

#[test]
fn heading_numbering_uses_word_multilevel_list() {
    let document = Document::new(FrontendId::Markdown, metadata()).with_blocks(numbered_headings());
    let output = Gb9704Backend::new()
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    let body = part(&output.docx, "word/document.xml");
    let numbering = part(&output.docx, "word/numbering.xml");
    let text = body_text(&body);

    assert!(text.contains("范围"));
    assert!(!text.contains("一、范围"));
    assert_eq!(body.matches("<w:numPr>").count(), 5);
    assert!(body.contains("<w:ilvl w:val=\"0\"/><w:numId w:val=\"1000\"/>"));
    assert!(body.contains("<w:ilvl w:val=\"3\"/><w:numId w:val=\"1000\"/>"));
    assert!(text.contains("五级"));
    assert!(!body.contains("<w:ilvl w:val=\"4\"/>"));

    assert!(numbering.contains("w:multiLevelType w:val=\"multilevel\""));
    assert!(numbering.contains("w:numFmt w:val=\"chineseCounting\""));
    assert!(numbering.contains("w:numFmt w:val=\"decimal\""));
    assert!(numbering.contains("w:lvlText w:val=\"%1、\""));
    assert!(numbering.contains("w:lvlText w:val=\"（%2）\""));
    assert!(numbering.contains("w:lvlText w:val=\"%3.\""));
    assert!(numbering.contains("w:lvlText w:val=\"（%4）\""));
    assert!(numbering.contains("w:suff w:val=\"nothing\""));

    assert_eq!(
        output
            .issues
            .iter()
            .filter(|issue| matches!(
                &issue.details,
                IssueDetails::RenderWarning { code, .. } if code == "heading_numbering_word"
            ))
            .count(),
        1
    );
    assert_eq!(
        output
            .issues
            .iter()
            .filter(|issue| matches!(
                &issue.details,
                IssueDetails::RenderWarning { code, .. } if code == "heading_numbering_static"
            ))
            .count(),
        0
    );
    assert_eq!(
        output
            .issues
            .iter()
            .filter(|issue| matches!(
                &issue.details,
                IssueDetails::RenderWarning { code, .. } if code == "heading_numbering_flattened"
            ))
            .count(),
        1
    );
}

#[test]
fn explicitly_unnumbered_headings_skip_native_numbering() {
    let blocks = vec![
        Block::heading(1, vec![Inline::text("第一", span())], span()).with_auto_number(true),
        Block::heading(1, vec![Inline::text("手写序号", span())], span()).with_auto_number(false),
        Block::heading(1, vec![Inline::text("第三", span())], span()).with_auto_number(true),
    ];
    let document = Document::new(FrontendId::Markdown, metadata()).with_blocks(blocks);
    let output = Gb9704Backend::new()
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    let body = part(&output.docx, "word/document.xml");
    assert_eq!(body.matches("<w:numPr>").count(), 2);
    assert_eq!(
        output
            .issues
            .iter()
            .filter(|issue| matches!(
                &issue.details,
                IssueDetails::RenderWarning { code, .. } if code == "heading_numbering_word"
            ))
            .count(),
        1
    );
}

#[test]
fn heading_numbering_falls_back_to_static_without_a_definition() {
    let styles = Gb9704Styles::from_json(&changed_style_json(|data| {
        data.as_object_mut().unwrap().remove("numbering");
    }))
    .unwrap();
    let backend = Gb9704Backend::with_styles(Gb9704Options::default(), styles);
    let document = Document::new(FrontendId::Markdown, metadata()).with_blocks(numbered_headings());
    let output = backend
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    let body = part(&output.docx, "word/document.xml");
    let text = body_text(&body);

    assert!(text.contains("一、范围"));
    assert!(text.contains("（二）再对象"));
    assert!(!body.contains("<w:numPr"));
    assert_eq!(
        output
            .issues
            .iter()
            .filter(|issue| matches!(
                &issue.details,
                IssueDetails::RenderWarning { code, .. } if code == "heading_numbering_static"
            ))
            .count(),
        1
    );
    assert_eq!(
        output
            .issues
            .iter()
            .filter(|issue| matches!(
                &issue.details,
                IssueDetails::RenderWarning { code, .. } if code == "heading_numbering_word"
            ))
            .count(),
        0
    );
}

#[test]
fn ordered_list_start_is_honored() {
    let list = ListBlock::ordered(
        3,
        vec![ListItem::new(vec![Inline::text("从三开始", span())])],
    );
    let document = Document::new(FrontendId::Markdown, metadata())
        .with_blocks(vec![Block::list(list, span())]);
    let output = Gb9704Backend::new()
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    let body = part(&output.docx, "word/document.xml");
    assert!(body_text(&body).contains("（3）从三开始"));
    assert!(!output.issues.iter().any(|issue| matches!(
        &issue.details,
        IssueDetails::RenderWarning { code, .. } if code == "list_start_ignored"
    )));
}

fn merged_cell(text: &str, colspan: u32, rowspan: u32) -> charta::TableCell {
    let mut cell = charta::TableCell::new(vec![Inline::text(text, span())]);
    cell.colspan = colspan;
    cell.rowspan = rowspan;
    cell
}

#[test]
fn renders_horizontal_and_vertical_merges() {
    let table = TableBlock {
        header: vec![charta::TableRow::new(vec![
            merged_cell("项目", 1, 1),
            merged_cell("情况", 2, 1),
        ])],
        rows: vec![
            charta::TableRow::new(vec![
                merged_cell("甲", 1, 2),
                merged_cell("乙", 1, 1),
                merged_cell("丙", 1, 1),
            ]),
            charta::TableRow::new(vec![merged_cell("丁", 1, 1), merged_cell("戊", 1, 1)]),
            charta::TableRow::new(vec![
                merged_cell("己", 1, 1),
                merged_cell("庚", 1, 1),
                merged_cell("辛", 1, 1),
            ]),
        ],
        column_alignments: Vec::new(),
        caption: None,
    };
    let document = Document::new(FrontendId::Markdown, metadata())
        .with_blocks(vec![Block::table(table, span())]);
    let output = Gb9704Backend::new()
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    let body = part(&output.docx, "word/document.xml");
    assert!(body.contains("<w:gridSpan w:val=\"2\"/>"));
    assert!(body.contains("<w:vMerge w:val=\"restart\"/>"));
    assert!(body.contains("<w:vMerge w:val=\"continue\"/>"));
    let text = body_text(&body);
    for value in [
        "项目", "情况", "甲", "乙", "丙", "丁", "戊", "己", "庚", "辛",
    ] {
        assert!(text.contains(value), "missing {value}");
    }
    assert!(!output.issues.iter().any(|issue| matches!(
        &issue.details,
        IssueDetails::RenderWarning { code, .. } if code == "table_merge_unsupported"
    )));
}

#[test]
fn rowspan_beyond_last_row_is_reported() {
    let table = TableBlock {
        header: Vec::new(),
        rows: vec![
            charta::TableRow::new(vec![merged_cell("跨行", 1, 3), merged_cell("同行", 1, 1)]),
            charta::TableRow::new(vec![merged_cell("后续", 1, 1)]),
        ],
        column_alignments: Vec::new(),
        caption: None,
    };
    let document = Document::new(FrontendId::Markdown, metadata())
        .with_blocks(vec![Block::table(table, span())]);
    let output = Gb9704Backend::new()
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    assert!(output.issues.iter().any(|issue| matches!(
        &issue.details,
        IssueDetails::RenderWarning { code, .. } if code == "table_rowspan_truncated"
    )));
}

#[test]
fn signer_is_reported_as_ignored() {
    let mut document = document();
    document.metadata.insert("signer", "张三");
    let output = Gb9704Backend::new()
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    assert!(output.issues.iter().any(|issue| matches!(
        &issue.details,
        IssueDetails::RenderWarning { code, .. } if code == "metadata_ignored"
    )));
}

#[test]
fn header_and_record_can_be_disabled() {
    let document = document();
    let backend =
        Gb9704Backend::with_options(Gb9704Options::new().without_header().without_record());
    let output = backend
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    let body = part(&output.docx, "word/document.xml");

    assert!(!body.contains("昆山市人民政府文件"));
    assert!(!body.contains("昆政发〔2026〕1号"));
    assert!(!body.contains("秘密"));
    assert!(!body.contains("特急"));
    assert!(!body.contains("抄送："));
    assert!(!body.contains("2026年10月26日印发"));
    assert!(!body.contains("<w:type w:val=\"nextPage\"/>"));
    assert!(!body.contains("<w:vAlign w:val=\"bottom\"/>"));

    assert!(body.contains("关于开展 2026 年度公文格式检查的通知"));
    assert!(body.contains("昆山市人民政府"));
    assert!(body.contains("2026年10月25日"));

    let ignored: Vec<_> = output
        .issues
        .iter()
        .filter(|issue| {
            matches!(
                &issue.details,
                IssueDetails::RenderWarning { code, .. } if code == "metadata_ignored"
            )
        })
        .collect();
    assert_eq!(ignored.len(), 6);
}

#[test]
fn header_and_record_switches_are_independent() {
    let document = document();

    let without_record = Gb9704Backend::with_options(Gb9704Options::new().without_record());
    let body = part(
        &without_record
            .render(RenderRequest {
                document: &document,
                metadata: &document.metadata,
            })
            .unwrap()
            .docx,
        "word/document.xml",
    );
    assert!(body.contains("昆政发〔2026〕1号"));
    assert!(!body.contains("抄送："));
    assert!(!body.contains("<w:type w:val=\"nextPage\"/>"));

    let without_header = Gb9704Backend::with_options(Gb9704Options::new().without_header());
    let body = part(
        &without_header
            .render(RenderRequest {
                document: &document,
                metadata: &document.metadata,
            })
            .unwrap()
            .docx,
        "word/document.xml",
    );
    assert!(!body.contains("昆政发〔2026〕1号"));
    assert!(body.contains("抄送：市委办公室、市人大常委会办公室"));
    assert!(body.contains("<w:type w:val=\"nextPage\"/>"));
}

#[test]
fn optional_header_relaxes_doc_number() {
    let mut metadata = metadata();
    metadata.fields.remove("doc_number");

    let default = Gb9704Backend::new();
    assert!(default.schema().field("doc_number").unwrap().required);
    assert!(!default.schema().validate(&metadata).is_empty());

    let backend = Gb9704Backend::with_options(Gb9704Options::new().without_header());
    assert!(!backend.schema().field("doc_number").unwrap().required);
    assert!(backend.schema().validate(&metadata).is_empty());
}

#[test]
fn color_can_be_disabled_explicitly() {
    let document = document();
    let backend = Gb9704Backend::with_options(Gb9704Options {
        adopt_color: false,
        ..Gb9704Options::default()
    });
    let output = backend
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    let body = part(&output.docx, "word/document.xml");
    assert!(!body.contains("FF0000"));
    let overrides: Vec<_> = output
        .issues
        .iter()
        .filter(|issue| issue.kind() == IssueKind::StyleOverridden)
        .collect();
    assert_eq!(overrides.len(), 4);
    assert!(overrides.iter().any(|issue| matches!(
        &issue.details,
        IssueDetails::StyleOverridden { property, .. } if property == "color"
    )));
}

#[test]
fn all_intents_can_be_adopted() {
    let document = document();
    let backend = Gb9704Backend::with_options(Gb9704Options::new().adopt_all());
    let output = backend
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    let body = part(&output.docx, "word/document.xml");
    assert!(body.contains("Arial"));
    assert!(!output
        .issues
        .iter()
        .any(|issue| issue.kind() == IssueKind::StyleOverridden));
}

fn style_json() -> serde_json::Value {
    serde_json::from_str(include_str!("../data/styles.json")).unwrap()
}

fn changed_style_json(change: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut data = style_json();
    change(&mut data);
    data.to_string()
}

#[test]
fn style_values_come_from_the_data_file() {
    let json = changed_style_json(|data| {
        data["page"]["margin_top_mm"] = serde_json::json!(42.0);
        let styles = data["styles"].as_array_mut().unwrap();
        let title = styles
            .iter_mut()
            .find(|style| style["id"] == "GbTitle")
            .unwrap();
        title["run"]["east_asia_font"] = serde_json::json!("测试标题字体");
    });
    let styles = Gb9704Styles::from_json(&json).unwrap();
    let backend = Gb9704Backend::with_styles(Gb9704Options::default(), styles);
    let document = document();
    let output = backend
        .render(RenderRequest {
            document: &document,
            metadata: &document.metadata,
        })
        .unwrap();
    let body = part(&output.docx, "word/document.xml");
    let styles_xml = part(&output.docx, "word/styles.xml");
    assert!(body.contains("w:top=\"2381\""));
    assert!(body.contains("测试标题字体"));
    assert!(styles_xml.contains("测试标题字体"));
}

#[test]
fn loads_style_data_from_an_external_file() {
    let styles = Gb9704Styles::from_path("data/styles.json").unwrap();
    let backend = Gb9704Backend::with_styles(Gb9704Options::default(), styles);
    assert_eq!(backend.styles().table().width_mm, 156.0);
    assert_eq!(
        backend.styles().version_record().columns_mm,
        vec![100.0, 56.0]
    );
}

#[test]
fn rejects_invalid_heading_numbering_data() {
    let missing_level = Gb9704Styles::from_json(&changed_style_json(|data| {
        data["numbering"]["heading"]["levels"]
            .as_array_mut()
            .unwrap()
            .pop();
    }))
    .unwrap_err()
    .to_string();
    assert!(missing_level.contains("缺少第 4 级"), "{missing_level}");

    let bad_format = Gb9704Styles::from_json(&changed_style_json(|data| {
        data["numbering"]["heading"]["levels"][0]["format"] = serde_json::json!("roman");
    }))
    .unwrap_err()
    .to_string();
    assert!(bad_format.contains("编号格式"), "{bad_format}");

    let both_indents = Gb9704Styles::from_json(&changed_style_json(|data| {
        data["numbering"]["heading"]["levels"][0]["hanging_twips"] = serde_json::json!(0);
    }))
    .unwrap_err()
    .to_string();
    assert!(both_indents.contains("不能同时指定"), "{both_indents}");

    let bad_placeholder = Gb9704Styles::from_json(&changed_style_json(|data| {
        data["numbering"]["heading"]["levels"][0]["text"] = serde_json::json!("第一章");
    }))
    .unwrap_err()
    .to_string();
    assert!(bad_placeholder.contains("占位符"), "{bad_placeholder}");
}

#[test]
fn rejects_missing_or_invalid_style_data() {
    assert!(Gb9704Styles::from_json("{}").is_err());
    assert!(Gb9704Styles::from_json(&changed_style_json(|data| {
        data["schema"] = serde_json::json!(2);
    }))
    .is_err());
    assert!(Gb9704Styles::from_json(&changed_style_json(|data| {
        data["colors"].as_object_mut().unwrap().remove("red");
    }))
    .is_err());
    assert!(Gb9704Styles::from_json(&changed_style_json(|data| {
        data["styles"]
            .as_array_mut()
            .unwrap()
            .retain(|style| style["id"] != "GbBody");
    }))
    .is_err());
    assert!(Gb9704Styles::from_json(&changed_style_json(|data| {
        let styles = data["styles"].as_array_mut().unwrap();
        let body = styles
            .iter_mut()
            .find(|style| style["id"] == "GbBody")
            .unwrap();
        body.as_object_mut().unwrap().remove("run");
    }))
    .is_err());
    assert!(Gb9704Styles::from_json(&changed_style_json(|data| {
        data["page"]["margin_top"] = serde_json::json!(37.0);
    }))
    .is_err());
    assert!(Gb9704Styles::from_json(&changed_style_json(|data| {
        let styles = data["styles"].as_array_mut().unwrap();
        let body = styles
            .iter_mut()
            .find(|style| style["id"] == "GbBody")
            .unwrap();
        body["paragraph"]["alignment"] = serde_json::json!("middle");
    }))
    .is_err());
    assert!(Gb9704Styles::from_path("target/definitely-missing-styles.json").is_err());
}

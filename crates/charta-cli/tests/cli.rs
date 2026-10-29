use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn charta() -> Command {
    Command::new(env!("CARGO_BIN_EXE_charta"))
}

fn work_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("charta-cli-test-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn assert_ok(output: &Output) {
    assert!(
        output.status.success(),
        "exit={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn report(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

const VALID_MD: &str = "---\ntitle: 测试通知\ndoc_number: 测试〔2026〕1号\nissuer: 测试单位\ndate: 2026-10-25\n---\n\n# 一、正文\n\n这是正文。\n";

#[test]
fn converts_markdown_and_writes_all_outputs() {
    let dir = work_dir("convert-md");
    let input = dir.join("notice.md");
    let docx = dir.join("notice.docx");
    let report_json = dir.join("notice.report.json");
    let report_md = dir.join("notice.report.md");
    let ir = dir.join("notice.ir.json");
    fs::write(&input, VALID_MD).unwrap();

    let output = charta()
        .args([
            "convert",
            input.to_str().unwrap(),
            "--backend",
            "gb9704",
            "--output",
            docx.to_str().unwrap(),
            "--report",
            report_json.to_str().unwrap(),
            "--report-md",
            report_md.to_str().unwrap(),
            "--emit-ir",
            ir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_ok(&output);
    assert!(docx.exists());
    assert!(report_json.exists());
    assert!(report_md.exists());
    assert!(ir.exists());
    assert!(fs::read(&docx).unwrap().starts_with(b"PK"));

    let report = report(&report_json);
    assert_eq!(report["report_version"], "1.0");
    assert_eq!(report["frontend"], "markdown");
    assert_eq!(report["backend"], "gb9704");
    assert_eq!(report["summary"]["blocking"], 0);
    assert_eq!(report["summary"]["total"], 0);

    let ir: Value = serde_json::from_str(&fs::read_to_string(&ir).unwrap()).unwrap();
    assert_eq!(ir["ir_version"], "1.1");
    assert_eq!(ir["frontend"], "markdown");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn missing_required_metadata_blocks_rendering() {
    let dir = work_dir("schema-block");
    let input = dir.join("notice.md");
    let docx = dir.join("notice.docx");
    let report_json = dir.join("notice.report.json");
    fs::write(&input, "---\ntitle: 只有标题\n---\n\n正文。\n").unwrap();

    let output = charta()
        .args([
            "convert",
            input.to_str().unwrap(),
            "--backend",
            "gb9704",
            "--output",
            docx.to_str().unwrap(),
            "--report",
            report_json.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!docx.exists());
    let report = report(&report_json);
    assert!(report["summary"]["blocking"].as_u64().unwrap() >= 3);
    assert!(report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| { issue["type"] == "schema_validation" && issue["field"] == "doc_number" }));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn no_header_and_no_record_allow_plain_documents() {
    let dir = work_dir("plain");
    let input = dir.join("plain.md");
    let docx = dir.join("plain.docx");
    let report_json = dir.join("plain.report.json");
    fs::write(
        &input,
        "---\ntitle: 测试通知\nissuer: 测试单位\ndate: 2026-10-25\ncc:\n  - 某单位\n---\n\n正文。\n",
    )
    .unwrap();

    let output = charta()
        .args([
            "convert",
            input.to_str().unwrap(),
            "--backend",
            "gb9704",
            "--no-header",
            "--no-record",
            "--output",
            docx.to_str().unwrap(),
            "--report",
            report_json.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_ok(&output);
    assert!(docx.exists());
    let report = report(&report_json);
    assert_eq!(report["summary"]["blocking"], 0);
    assert!(report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| issue["code"] == "metadata_ignored"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn typst_loop_report_has_source_location_and_impact() {
    let dir = work_dir("typst-loop");
    let input = dir.join("notice.typ");
    let docx = dir.join("notice.docx");
    let report_json = dir.join("notice.report.json");
    fs::write(
        &input,
        "#let charta-meta = (\n  title: \"Typst 测试通知\",\n  doc_number: \"测试〔2026〕1号\",\n  issuer: \"测试单位\",\n  date: \"2026-10-25\",\n)\n\n= 正文\n\n#for item in items [循环体文字]\n",
    )
    .unwrap();

    let output = charta()
        .args([
            "convert",
            input.to_str().unwrap(),
            "--backend",
            "gb9704",
            "--output",
            docx.to_str().unwrap(),
            "--report",
            report_json.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_ok(&output);
    assert!(docx.exists());
    let report = report(&report_json);
    assert_eq!(report["frontend"], "typst");
    let issue = report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|issue| issue["type"] == "non_deterministic")
        .expect("non-deterministic issue");
    assert_eq!(issue["location"]["file"], input.to_str().unwrap());
    assert_eq!(issue["location"]["line"], 10);
    assert!(issue["reason"].as_str().unwrap().contains("循环"));
    assert!(issue["impact"].as_str().unwrap().contains("重复次数"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn typst_lists_tables_and_images_reach_the_docx() {
    let dir = work_dir("typst-rich");
    let input = dir.join("rich.typ");
    let image = dir.join("chart.png");
    let asset =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/assets/format-check-chart.png");
    fs::copy(asset, &image).unwrap();
    fs::write(
        &input,
        "#let charta-meta = (\n  title: \"Typst 富内容\",\n  author: \"张三\",\n  date: \"2026-10-25\",\n)\n\n= 正文\n\n- 第一项\n  - 子项\n\n+ 办理登记\n\n#table(columns: 2, table.header[环节][状态], [受理], [完成])\n\n#figure(image(\"chart.png\"), caption: [合规率对比])\n",
    )
    .unwrap();
    let docx = dir.join("rich.docx");
    let report_json = dir.join("rich.report.json");

    let output = charta()
        .args([
            "convert",
            input.to_str().unwrap(),
            "--output",
            docx.to_str().unwrap(),
            "--report",
            report_json.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_ok(&output);
    assert!(docx.exists());
    let report = report(&report_json);
    assert_eq!(report["summary"]["blocking"], 0);
    let issues = report["issues"].as_array().unwrap();
    assert!(
        !issues
            .iter()
            .any(|issue| issue["type"] == "unsupported_syntax"),
        "{issues:?}"
    );
    assert!(
        !issues
            .iter()
            .any(|issue| issue["type"] == "non_deterministic"),
        "{issues:?}"
    );
    assert!(issues
        .iter()
        .any(|issue| issue["code"] == "image_caption_static"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn strict_mode_turns_unsupported_syntax_into_blocking() {
    let dir = work_dir("strict");
    let input = dir.join("notice.md");
    fs::write(&input, format!("{VALID_MD}\n[链接](https://example.com)\n")).unwrap();

    let lenient = charta()
        .args([
            "convert",
            input.to_str().unwrap(),
            "--backend",
            "gb9704",
            "--output",
            dir.join("a.docx").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_ok(&lenient);
    assert!(dir.join("a.docx").exists());

    let strict_docx = dir.join("b.docx");
    let strict = charta()
        .args([
            "convert",
            input.to_str().unwrap(),
            "--backend",
            "gb9704",
            "--strict",
            "--output",
            strict_docx.to_str().unwrap(),
            "--report",
            dir.join("b.report.json").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(strict.status.code(), Some(2));
    assert!(!strict_docx.exists());
    let report = report(&dir.join("b.report.json"));
    assert!(report["summary"]["blocking"].as_u64().unwrap() >= 1);
    assert!(report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| { issue["type"] == "unsupported_syntax" }));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn converts_general_template_with_author_and_date() {
    let dir = work_dir("general");
    let input = dir.join("general.md");
    let docx = dir.join("general.docx");
    let report_json = dir.join("general.report.json");
    fs::write(
        &input,
        "---\nauthor: 张三\ndate: 2026-10-25\nrecipient: 各部门\n---\n\n# 关于做好近期重点工作的通知\n\n## 总体要求\n\n请各部门**高度重视**，*抓紧落实*。\n",
    )
    .unwrap();

    let output = charta()
        .args([
            "convert",
            input.to_str().unwrap(),
            "--output",
            docx.to_str().unwrap(),
            "--report",
            report_json.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_ok(&output);
    assert!(docx.exists());
    let report = report(&report_json);
    assert_eq!(report["backend"], "general");
    assert_eq!(report["summary"]["blocking"], 0);
    assert_eq!(report["summary"]["total"], 0);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn converts_compilation_with_cover_and_toc() {
    let dir = work_dir("compilation");
    let input = dir.join("compilation.md");
    let docx = dir.join("compilation.docx");
    let report_json = dir.join("compilation.report.json");
    fs::write(
        &input,
        "---\ntitle: 2026 年度重点工作文件汇编\ndate: 2026-10-25\n---\n\n# 第一篇\n\n## 总体要求\n\n正文内容。\n",
    )
    .unwrap();

    let output = charta()
        .args([
            "convert",
            input.to_str().unwrap(),
            "--backend",
            "compilation",
            "--output",
            docx.to_str().unwrap(),
            "--report",
            report_json.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_ok(&output);
    assert!(docx.exists());
    let report = report(&report_json);
    assert_eq!(report["backend"], "compilation");
    assert_eq!(report["summary"]["blocking"], 0);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn version_backends_and_schema_commands_work() {
    let version = charta().arg("--version").output().unwrap();
    assert_ok(&version);
    assert!(String::from_utf8_lossy(&version.stdout).contains("0.1.0"));

    let backends = charta().arg("backends").output().unwrap();
    assert_ok(&backends);
    let stdout = String::from_utf8_lossy(&backends.stdout);
    assert!(stdout.contains("gb9704"));
    assert!(stdout.contains("GB/T 9704-2012"));
    assert!(stdout.contains("general"));
    assert!(stdout.contains("通用材料"));
    assert!(stdout.contains("compilation"));
    assert!(stdout.contains("文件汇编"));

    let schema = charta()
        .args(["schema", "--backend", "gb9704"])
        .output()
        .unwrap();
    assert_ok(&schema);
    let stdout = String::from_utf8_lossy(&schema.stdout);
    assert!(stdout.contains("doc_number"));
    assert!(stdout.contains("title"));

    let schema_json = charta()
        .args(["schema", "--backend", "gb9704", "--json"])
        .output()
        .unwrap();
    assert_ok(&schema_json);
    let value: Value = serde_json::from_slice(&schema_json.stdout).unwrap();
    assert!(value["fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| { field["name"] == "date" && field["required"] == true }));

    let general_schema = charta()
        .args(["schema", "--backend", "general", "--json"])
        .output()
        .unwrap();
    assert_ok(&general_schema);
    let value: Value = serde_json::from_slice(&general_schema.stdout).unwrap();
    assert!(value["fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| { field["name"] == "author" && field["required"] == false }));
    assert!(value["fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| { field["name"] == "recipient" && field["required"] == false }));

    let compilation_schema = charta()
        .args(["schema", "--backend", "compilation", "--json"])
        .output()
        .unwrap();
    assert_ok(&compilation_schema);
    let value: Value = serde_json::from_slice(&compilation_schema.stdout).unwrap();
    assert!(value["fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| { field["name"] == "title" && field["required"] == true }));
    let date_style = value["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["name"] == "date_style")
        .unwrap();
    assert!(date_style["enum_values"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "chinese"));

    let default_schema = charta().args(["schema", "--json"]).output().unwrap();
    assert_ok(&default_schema);
    let value: Value = serde_json::from_slice(&default_schema.stdout).unwrap();
    assert!(value["fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| { field["name"] == "author" && field["required"] == false }));
}

#[test]
fn external_style_files_and_patches_are_wired() {
    let dir = work_dir("styles");
    let input = dir.join("general.md");
    let patch = dir.join("tweaks.json");
    let docx = dir.join("general.docx");
    fs::write(
        &input,
        "---\ntitle: 外部样式\nauthor: 李四\ndate: 2026-10-25\n---\n\n正文**重点**。\n",
    )
    .unwrap();
    fs::write(
        &patch,
        r#"{
          "styles": {
            "GbBody": { "run": { "size_step": 1 } },
            "GbStrong": { "run": { "east_asia_font": "方正黑体_GBK" } }
          }
        }"#,
    )
    .unwrap();

    let output = charta()
        .args([
            "convert",
            input.to_str().unwrap(),
            "--styles",
            patch.to_str().unwrap(),
            "--output",
            docx.to_str().unwrap(),
            "--report",
            dir.join("general.report.json").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_ok(&output);
    assert!(docx.exists());

    let complete =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../charta-backend-cn/data/general-styles.json");
    let output = charta()
        .args([
            "convert",
            input.to_str().unwrap(),
            "--styles",
            complete.to_str().unwrap(),
            "--output",
            dir.join("complete.docx").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_ok(&output);
    assert!(dir.join("complete.docx").exists());

    let missing = charta()
        .args([
            "convert",
            input.to_str().unwrap(),
            "--styles",
            dir.join("missing.json").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("无法读取样式数据"));

    let bad_patch = dir.join("bad.json");
    fs::write(&bad_patch, r#"{ "styles": { "GbBoddy": {} } }"#).unwrap();
    let bad = charta()
        .args([
            "convert",
            input.to_str().unwrap(),
            "--styles",
            bad_patch.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("未定义样式"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn generates_a_pandoc_reference_docx() {
    let dir = work_dir("pandoc-reference");
    let output_path = dir.join("reference.docx");
    let output = charta()
        .args([
            "reference",
            "--backend",
            "general",
            "--output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_ok(&output);
    let bytes = fs::read(&output_path).unwrap();
    assert!(bytes.starts_with(b"PK"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("GbBody → BodyText"));
    assert!(stdout.contains("reference.docx："));

    let patch = dir.join("tweak.json");
    fs::write(
        &patch,
        r#"{ "styles": { "GbBody": { "run": { "size_step": 1 } } } }"#,
    )
    .unwrap();
    let patched_path = dir.join("patched.docx");
    let patched = charta()
        .args([
            "reference",
            "--backend",
            "general",
            "--styles",
            patch.to_str().unwrap(),
            "--output",
            patched_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_ok(&patched);
    assert_ne!(fs::read(&patched_path).unwrap(), bytes);

    let unknown = charta()
        .args(["reference", "--backend", "notice"])
        .output()
        .unwrap();
    assert_eq!(unknown.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("未知后端"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn reports_are_written_only_when_requested() {
    let dir = work_dir("report-opt");
    let input = dir.join("general.md");
    fs::write(
        &input,
        "---\nauthor: 张三\ndate: 2026-10-25\n---\n\n# 材料标题\n\n正文。\n",
    )
    .unwrap();

    let plain = charta()
        .args([
            "convert",
            input.to_str().unwrap(),
            "--output",
            dir.join("plain.docx").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_ok(&plain);
    assert!(dir.join("plain.docx").exists());
    assert!(!dir.join("plain.report.json").exists());
    assert!(!dir.join("plain.report.md").exists());
    assert!(!String::from_utf8_lossy(&plain.stdout).contains("报告："));

    let json_only = charta()
        .args([
            "convert",
            input.to_str().unwrap(),
            "--output",
            dir.join("json.docx").to_str().unwrap(),
            "--report",
            dir.join("json.report.json").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_ok(&json_only);
    assert!(dir.join("json.report.json").exists());
    assert!(!dir.join("json.report.md").exists());
    assert_eq!(report(&dir.join("json.report.json"))["backend"], "general");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn auto_detects_typst_extension() {
    let dir = work_dir("auto-typst");
    let input = dir.join("notice.typ");
    fs::write(
        &input,
        "#let charta-meta = (\n  title: \"标题\",\n  doc_number: \"测试〔2026〕1号\",\n  issuer: \"测试单位\",\n  date: \"2026-10-25\",\n)\n\n= 正文\n\n内容。\n",
    )
    .unwrap();
    let output = charta()
        .args(["convert", input.to_str().unwrap(), "--backend", "gb9704"])
        .output()
        .unwrap();
    assert_ok(&output);
    assert!(dir.join("notice.docx").exists());
    assert!(String::from_utf8_lossy(&output.stdout).contains("前端：typst"));
    let _ = fs::remove_dir_all(&dir);
}

//! Thin CLI shell. All document logic lives in the library crates.

use charta::{convert, Backend, ConversionRequest, Frontend, SourceFile};
use charta_backend_gb9704::{
    CompilationBackend, Gb9704Backend, Gb9704Options, Gb9704Styles, Gb9704Template, GeneralBackend,
    StyleSet,
};
use charta_md::MarkdownFrontend;
use charta_typst::TypstFrontend;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};

type CliResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(
    name = "charta",
    version,
    about = "Charta 语义文书引擎：Markdown/Typst → IR → 规范后端 → docx（可选审计报告）",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Command {
    /// 将单个 Markdown 或 Typst 文档转换为 docx，可选输出审计报告
    Convert(ConvertArgs),
    /// 生成供 pandoc 使用的 reference.docx（样式映射为 pandoc 内置名）
    Reference(ReferenceArgs),
    /// 列出内置规范后端
    Backends,
    /// 查看后端的元数据 schema
    Schema(SchemaArgs),
}

#[derive(Args)]
struct ReferenceArgs {
    /// 样式来源后端：general（通用材料）、compilation（文件汇编）或 gb9704（通知）
    #[arg(long, default_value = "general", value_name = "ID")]
    backend: String,

    /// 输出路径
    #[arg(
        long,
        short,
        value_name = "PATH",
        default_value = "pandoc-reference.docx"
    )]
    output: PathBuf,

    /// 外部样式 JSON：完整样式文件或只含改动项的补丁
    #[arg(long, value_name = "PATH")]
    styles: Option<PathBuf>,
}

#[derive(Args)]
struct ConvertArgs {
    /// 源文件（.md / .typ）
    #[arg(value_name = "INPUT")]
    input: PathBuf,

    /// 前端；默认按扩展名自动判断
    #[arg(long, value_enum, default_value_t = FrontendChoice::Auto)]
    frontend: FrontendChoice,

    /// 目标规范后端：general（通用材料）、compilation（文件汇编）或 gb9704（通知）
    #[arg(long, default_value = "general", value_name = "ID")]
    backend: String,

    /// docx 输出路径；缺省为源文件同名 .docx
    #[arg(long, short, value_name = "PATH")]
    output: Option<PathBuf>,

    /// 机器可读报告路径；提供时才写出 JSON 报告
    #[arg(long, value_name = "PATH")]
    report: Option<PathBuf>,

    /// 人可读报告路径；提供时才写出 Markdown 报告
    #[arg(long = "report-md", value_name = "PATH")]
    report_md: Option<PathBuf>,

    /// 额外输出 IR JSON，便于审计与调试
    #[arg(long = "emit-ir", value_name = "PATH")]
    emit_ir: Option<PathBuf>,

    /// 严格模式：契约外语法从告警升级为阻断
    #[arg(long)]
    strict: bool,

    /// 外部样式 JSON：完整样式文件或只含改动项的补丁；缺省使用内置样式
    #[arg(long, value_name = "PATH")]
    styles: Option<PathBuf>,

    /// 不生成版头（份号、密级、紧急程度、发文机关标志、发文字号与红色分隔线；仅 gb9704）
    #[arg(long)]
    no_header: bool,

    /// 不生成版记（抄送、印发机关、印发日期及末页沉底分节；仅 gb9704）
    #[arg(long)]
    no_record: bool,

    /// 是否采纳源文件中的颜色意图（后端显式配置）
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    adopt_color: bool,

    /// 是否采纳源文件中的字体意图
    #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
    adopt_font_family: bool,

    /// 是否采纳源文件中的字号意图
    #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
    adopt_font_size: bool,

    /// 是否采纳源文件中的加粗意图
    #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
    adopt_bold: bool,

    /// 是否采纳源文件中的斜体意图
    #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
    adopt_italic: bool,
}

#[derive(Args)]
struct SchemaArgs {
    /// 后端 ID
    #[arg(long, default_value = "general", value_name = "ID")]
    backend: String,

    /// 以 JSON 输出
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FrontendChoice {
    Auto,
    Md,
    Typst,
}

fn main() {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("charta: {error}");
            std::process::exit(1);
        }
    }
}

fn run(cli: Cli) -> CliResult<i32> {
    match cli.command {
        Command::Convert(args) => convert_command(args),
        Command::Reference(args) => reference_command(args),
        Command::Backends => {
            println!("charta {}", charta::VERSION);
            println!("{:<12} {:<28} schema 字段数", "ID", "名称");
            let backends: Vec<Box<dyn Backend>> = vec![
                Box::new(Gb9704Backend::try_new()?),
                Box::new(GeneralBackend::try_new()?),
                Box::new(CompilationBackend::try_new()?),
            ];
            for backend in &backends {
                println!(
                    "{:<12} {:<28} {}",
                    backend.id(),
                    backend.display_name(),
                    backend.schema().fields.len()
                );
            }
            Ok(0)
        }
        Command::Schema(args) => {
            let backend = resolve_backend(&args.backend)?;
            if args.json {
                print!("{}", backend.schema().to_json()?);
            } else {
                println!("# {} ({})\n", backend.display_name(), backend.id());
                print!("{}", backend.schema().to_markdown());
            }
            Ok(0)
        }
    }
}

fn convert_command(args: ConvertArgs) -> CliResult<i32> {
    let frontend_choice = resolve_frontend(&args.input, args.frontend)?;
    let frontend: Box<dyn Frontend> = match frontend_choice {
        FrontendChoice::Md => Box::new(MarkdownFrontend::new()),
        FrontendChoice::Typst => Box::new(TypstFrontend::new()),
        FrontendChoice::Auto => unreachable!("auto is resolved by resolve_frontend"),
    };
    let backend = resolve_backend_with_options(
        &args.backend,
        Gb9704Options {
            template: Gb9704Template::Notice,
            header: !args.no_header,
            record: !args.no_record,
            adopt_color: args.adopt_color,
            adopt_font_family: args.adopt_font_family,
            adopt_font_size: args.adopt_font_size,
            adopt_bold: args.adopt_bold,
            adopt_italic: args.adopt_italic,
        },
        args.styles.as_deref(),
    )?;

    let output_path = args
        .output
        .clone()
        .unwrap_or_else(|| args.input.with_extension("docx"));

    let source = SourceFile::read(&args.input)?;
    let result = convert(ConversionRequest {
        source,
        frontend: frontend.as_ref(),
        backend: backend.as_ref(),
        strict: args.strict,
    })?;

    if let Some(docx) = &result.docx {
        std::fs::write(&output_path, docx)?;
    }
    if let Some(path) = &args.report {
        result.report.write_json(path)?;
    }
    if let Some(path) = &args.report_md {
        result.report.write_markdown(path)?;
    }
    if let Some(path) = &args.emit_ir {
        let mut json = serde_json::to_string_pretty(&result.document)?;
        json.push('\n');
        std::fs::write(path, json)?;
    }

    println!("charta {}", charta::VERSION);
    println!("源文件：{}", args.input.display());
    println!("前端：{}", frontend.display_name());
    println!("后端：{} ({})", backend.display_name(), backend.id());
    match &result.docx {
        Some(_) => println!("docx：{}", output_path.display()),
        None => println!("docx：未生成（存在阻断性问题）"),
    }
    if let Some(path) = &args.report {
        println!("报告：{}", path.display());
    }
    if let Some(path) = &args.report_md {
        println!("报告：{}", path.display());
    }
    if let Some(path) = &args.emit_ir {
        println!("IR：{}", path.display());
    }
    println!(
        "问题：{} 阻断 / {} 告警",
        result.report.summary.blocking, result.report.summary.warnings
    );

    let blocking: Vec<_> = result
        .report
        .issues
        .iter()
        .filter(|issue| issue.is_blocking())
        .take(5)
        .collect();
    if !blocking.is_empty() {
        println!("阻断性问题：");
        for issue in blocking {
            let location = issue
                .location
                .as_ref()
                .map(|span| format!(" ({})", span.display()))
                .unwrap_or_default();
            println!(
                "  - [{}] {}{}",
                issue.kind().label_zh(),
                issue.message,
                location
            );
        }
        if result.report.blocking_count() > 5 {
            let rest = result.report.blocking_count() - 5;
            if args.report.is_some() || args.report_md.is_some() {
                println!("  …… 其余 {rest} 项见报告");
            } else {
                println!("  …… 其余 {rest} 项");
            }
        }
        if args.strict {
            println!("提示：严格模式会把契约外语法升级为阻断。");
        }
    }

    Ok(if result.report.has_blocking() { 2 } else { 0 })
}

fn reference_command(args: ReferenceArgs) -> CliResult<i32> {
    let set = match args.backend.as_str() {
        "gb9704" => StyleSet::Notice,
        "general" | "compilation" => StyleSet::General,
        other => {
            return Err(charta::Error::InvalidArgument(format!(
                "reference 支持 general、compilation、gb9704，未知后端 `{other}`"
            ))
            .into())
        }
    };
    let styles = load_styles(args.styles.as_deref(), set)?;
    let mapping = styles.pandoc_style_mapping();
    let bytes = styles.pandoc_reference_docx()?;
    std::fs::write(&args.output, bytes)?;

    println!("charta {}", charta::VERSION);
    println!("样式集：{}", args.backend);
    for (source, target) in mapping {
        println!("  {source} → {target}");
    }
    println!("reference.docx：{}", args.output.display());
    Ok(0)
}

fn resolve_frontend(path: &Path, choice: FrontendChoice) -> CliResult<FrontendChoice> {
    if !matches!(choice, FrontendChoice::Auto) {
        return Ok(choice);
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "md" | "markdown" | "mdown" => Ok(FrontendChoice::Md),
        "typ" => Ok(FrontendChoice::Typst),
        _ => Err(charta::Error::InvalidArgument(format!(
            "无法根据扩展名判断前端：{}；请使用 --frontend md|typst",
            path.display()
        ))
        .into()),
    }
}

fn resolve_backend(id: &str) -> CliResult<Box<dyn Backend>> {
    resolve_backend_with_options(id, Gb9704Options::default(), None)
}

fn resolve_backend_with_options(
    id: &str,
    options: Gb9704Options,
    style_path: Option<&Path>,
) -> CliResult<Box<dyn Backend>> {
    match id {
        "gb9704" => {
            let styles = load_styles(style_path, StyleSet::Notice)?;
            Ok(Box::new(Gb9704Backend::with_styles(options, styles)))
        }
        "general" => {
            let styles = load_styles(style_path, StyleSet::General)?;
            Ok(Box::new(GeneralBackend::try_with_styles(styles)?))
        }
        "compilation" => {
            let styles = load_styles(style_path, StyleSet::General)?;
            Ok(Box::new(CompilationBackend::try_with_styles(styles)?))
        }
        other => Err(charta::Error::InvalidArgument(format!(
            "未知后端 `{other}`；当前可用：general、compilation、gb9704"
        ))
        .into()),
    }
}

fn load_styles(path: Option<&Path>, set: StyleSet) -> CliResult<Gb9704Styles> {
    match path {
        Some(path) => Ok(Gb9704Styles::from_external_for(path, set)?),
        None => Ok(Gb9704Styles::embedded_for(set)?),
    }
}

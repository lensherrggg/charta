---
name: charta
description: 在 Charta 仓库中开发、调试和交付中文公文 docx 引擎时使用。涵盖工作区结构、构建与测试命令、样式数据与后端约定、CLI 用法、Word/pandoc 渲染核验和提交流程。
---

# Charta 开发技能

## 开始前

- 先读 `README.md`（用法、CLI 参数、模板口径）和 `docs/DESIGN.md`（架构、IR 契约、后端细节）。
- 看 `CHANGELOG.md` 的 `Unreleased`：本轮改动先写在这里。
- 改动前跑 `git status`、`git log -3`，确认没有别人的未提交工作。
- 涉及 GB/T 9704 条文时先查标准原文，区分“有规定”和“无规定”，不要凭印象补规则。

## 仓库结构

```
crates/
  charta/                  核心库：IR、管线、schema、报告、角色
  charta-md/               Markdown 前端
  charta-typst/            Typst 前端（只读 CST 子集）
  charta-docx-kit/         OOXML 机械操作（页面、样式、表格、编号、域、页脚）
  charta-backend-cn/       中文公文模板后端：通知、通用材料、文件汇编
  charta-cli/              CLI 薄壳
docs/DESIGN.md             架构与设计决策
examples/                  可直接转换的样例（含 assets/）
scripts/bump-version.sh    版本号更新
dist/                      发布 zip（已 gitignore）
```

`charta-backend-cn` 同时承载三个后端：`Gb9704Backend`（通知）、`GeneralBackend`（通用材料）、`CompilationBackend`（文件汇编）。共享类型是 `DocStyles` / `DocOptions` / `DocTemplate`，样式集由 `StyleSet::{Notice, General}` 区分。

## 常用命令

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release -p charta-cli
# Windows 下二进制为 target/release/charta.exe，其他平台为 target/release/charta

# 转换样例（报告默认不落盘）
./target/release/charta.exe convert examples/general.md -o out/general.docx
./target/release/charta.exe convert examples/compilation.md --backend compilation
./target/release/charta.exe convert examples/notice.md --backend gb9704
./target/release/charta.exe convert examples/notice.md --backend gb9704 \
  --report out/notice.report.json --report-md out/notice.report.md

# 生成 pandoc 参考样式
./target/release/charta.exe reference --backend general -o pandoc-reference.docx

# 查询
./target/release/charta.exe backends
./target/release/charta.exe schema --backend general --json
```

- Rust MSRV 1.75：不要用更新的标准库 API。
- 测试只写工作区内的临时目录，收尾删净；`out/` 已被 gitignore，但不要长期堆积产物。

## 关键约定

- **零外部模板**：docx 全部由 `charta-docx-kit` 从零生成。
- **样式值只在数据文件**：`crates/charta-backend-cn/data/styles.json`（通知）和 `general-styles.json`（通用/汇编）。渲染流程只按样式 ID 取 `RunConfig` / 段落配置，不内联字体、字号、行距、颜色、页边距、边框等字面量。静态列表序数属于例外。
- **样式编译进二进制**：`include_str!` 嵌入，运行时不需要仓库里的 `data/`。
- **docx-kit 只做机械操作**：不承载国标语义；后端可以直接改模型字段。
- **新增样式**：同步更新 data JSON、`styles.rs` 的 `STYLE_*` 常量与必需样式列表、`tests.rs` 断言和 `docs/DESIGN.md`。
- **新增后端/模板**：`DocTemplate` 枚举、schema、`layout/` 渲染函数、`lib.rs` 的 `Backend` 实现、CLI 的 `resolve_backend_with_options` 与帮助文本，最后补测试和文档。
- **样式补丁**：`--styles` 接受完整文件（`styles` 为数组）或补丁（`styles` 为对象）；未知样式 ID、未知字段、越界字号直接报错，不做静默回退。
- **报告 opt-in**：`--report` / `--report-md` 只在显式给出路径时写文件，标准输出也只回显实际生成的文件。
- **路径解析**：图片和 Typst `include` 相对源文件目录解析；`--styles`、`--output`、`--report`、`--emit-ir`、`reference` 的默认输出相对当前工作目录。

## 模板口径

- `general`：正文首个 `#` 是大标题；`##`–`#####` 依次编号 `一、` / `（一）` / `1.` / `（1）`；`author`、`date`、`recipient` 均可选，两项署名都没有时不生成署名区；没有目录。
- `compilation`：封面（空 5 行 + 标题 + 空 7 行 + 日期，1.5 倍行距）→ 目录（单独一页，标题“目　　录”，标题下三号空行，Word TOC 域只收 `#`）→ 正文。封面与目录单独成节、不显示页码；正文从 1 起编，奇偶页页码居右/居左；每个 `#` 用普通分页符另起一页，不另分节。
- `gb9704`：通知版头、主体、版记；默认不渲染签发人；列表为四级静态文本序数。
- 标题自动编号由 Word 多级列表维护（`numbering.heading`），超过四级报告 `heading_numbering_flattened`；样式数据缺少编号定义时回退静态文本。

## 渲染核验（Windows）

测试通过不代表版式正确；涉及版面、分页、页码、目录的改动必须实渲染确认。

```powershell
$word = New-Object -ComObject Word.Application
$word.Visible = $false
$word.DisplayAlerts = 0
try {
  $doc = $word.Documents.Open('D:\path\input.docx', $false, $true)  # ReadOnly
  $doc.Fields.Update() | Out-Null
  $doc.Repaginate()
  $doc.SaveAs2('D:\path\output.pdf', 17)
  $doc.Close($false)
} finally {
  $word.Quit()
}
```

```python
import pymupdf
doc = pymupdf.open('output.pdf')
for i, page in enumerate(doc):
    page.get_pixmap(dpi=110).save(f'page{i + 1}.png')
```

- PowerShell 脚本保存为 UTF-8 带 BOM，避免中文路径乱码。
- 只关闭自己打开的文档，不要 `taskkill WINWORD`；COM 卡住时先确认没有残留进程。
- 目录、页码要先 `Fields.Update()` 再导出 PDF，否则 TOC 是空的。
- 看完图再下结论；没有渲染结果时明确说“未实测”，不要口头认定已修好。

## pandoc 参考样式

- `charta reference` 把 `Gb*` 样式 ID 映射为 pandoc 内置 ID（`Title`、`BodyText`、`Heading1`–`9`、`BlockText` 等），并派生 `FirstParagraph`、`Compact`、`Date`。
- 验证：`pandoc sample.md --reference-doc=pandoc-reference.docx -o sample.docx`，再检查 `word/document.xml` 的 `pStyle` 与 `styles.xml` 的字体/行距。
- 已知限制：表格样式 `Table` 是表样式，docx-kit 尚未支持，pandoc 输出的表格没有边框。
- pandoc 的 `#` 对应 `Heading 1`，与通用模板“`#` 是大标题”的口径不同，文档里要写清楚。

## 常见坑

- **Windows 换行**：仓库文件是 CRLF，`git diff --check` 会把每行报成行尾空白，属正常现象，不要批量转 LF。
- **exe 被占用**：运行中的 `charta.exe` 会锁住 `target/release`，需要重新编译时先让进程退出，或换 `--target-dir`。
- **分节语义**：`sectPr` 描述它所在节的内容，文末 `sectPr` 描述最后一节；段内 `sectPr` 属于被它结束的那一节。改页脚/页码先画清“哪一节是哪几页”。
- **页码**：`w:pgNumType w:start` 只对所在节生效；封面/目录无页码靠“该节不带 `footerReference`”，不是靠隐藏。
- **样式改名**：改样式 ID 时要同步 `based_on` / `next` 和 `numbering` 里的 `pStyle` 引用；多个 docx 合并编号时还要换 `w:nsid`。
- **首行缩进**：`w:firstLineChars` 与 `w:firstLine` 同时写；复制样式时不要只抄字符缩进。
- **图片**：后端按版心等比缩放；读取或解码失败报告 `image_read_failed` / `image_decode_failed` 并跳过。

## 提交与交付

- 先把本轮改动写进 `CHANGELOG.md` 的 `Unreleased`。
- 只有用户明确要求时才 commit；push 需要单独点名。
- 发布包默认放 `dist/`（已 gitignore），内容为 `charta.exe` + `README.md` + `CHANGELOG.md` + `examples/`，打完从解压目录跑一遍 `--version`、`backends` 和一次转换。
- 交付前删除临时 docx/pdf/png/report 和解压目录，确认 `git status` 只剩预期改动。

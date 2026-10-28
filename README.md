# Charta

Charta（拉丁语 *charta*，纸、文书）是一个以**语义 IR** 为核心的 docx 生成引擎。前端只负责写内容、填元数据；视觉形态由规范后端决定；样式意图与静态不可确定性都会进入审计报告。

当前版本：**0.1.0**（第一期 MVP）

- IR 格式：`1.1`
- 报告格式：`1.0`
- 第一期后端：GB/T 9704-2012 通知；通用材料（方正 GBK）

## 设计要点

- **唯一契约是 IR。** Markdown / Typst 前端先解析为 IR，后端只消费 IR + 元数据。
- **后端是视觉权威。** 输入源中的字体、字号、颜色等只记录为样式意图；后端显式启用采纳后才生效，被覆盖的意图逐条进入报告。
- **不静默降级。** Typst 循环、条件、`import`、`#set`、`#show` 等不可静态确定的内容会降级并报告源文件位置、原因和影响范围。
- **零外部模板。** 引擎不感知 `.docx` 模板；GB/T 9704 后端使用 `charta-docx-kit` 从零构建 OOXML 包。
- **样式与流程分离。** GB/T 9704 的字体、字号、颜色、页边距、行距、边框、表格列宽等样式值位于后端样式数据文件；渲染流程只按样式 ID 套用，不在代码里内联样式字面量。
- **可审计。** 每次转换同时产出 JSON 报告和 Markdown 报告，区分阻断性错误与非阻断告警。

## 工程结构

```
charta/
├── crates/
│   ├── charta/                  # 核心库：IR、管线、schema、报告、角色
│   ├── charta-md/               # Markdown 前端
│   ├── charta-typst/            # Typst 前端（只读 CST 子集）
│   ├── charta-docx-kit/         # OOXML 机械操作工具箱
│   ├── charta-backend-gb9704/   # GB/T 9704 通知 + 通用材料后端
│   │   ├── data/styles.json     # 通知样式（字体/字号/页边距/表格等）
│   │   ├── data/general-styles.json # 通用材料样式
│   │   └── src/layout/          # 版头/主体/版记/文本/页脚流程
│   └── charta-cli/              # CLI 薄壳
├── docs/DESIGN.md               # 设计文档
├── docs/VERSIONING.md           # 版本管理
├── examples/                    # 示例输入
├── scripts/bump-version.sh      # 版本递增脚本
├── CHANGELOG.md
└── Cargo.toml                   # workspace 版本单一来源
```

每个规范后端 crate 按同一模式组织：`lib.rs` 只做 `Backend` 契约与渲染编排，`schema.rs` 声明元数据，`styles.rs` + `data/` 提供样式值，`layout/` 承载版式流程，`numbering.rs` 管编号策略，`report.rs` 管后端报告口径。同一 crate 可以承载多个轻量模板；结构差异大的文种再新建 crate。

## 快速开始

要求 Rust 1.75+。在仓库根目录执行：

```bash
cargo build --release
cargo test --workspace
```

CLI 二进制为 `target/release/charta`。也可以直接用 `cargo run -p charta-cli --`。

### 转换 Markdown 通知

```bash
mkdir -p out
cargo run -p charta-cli -- convert examples/notice.md --backend gb9704 \
  --output out/notice.docx \
  --report out/notice.report.json \
  --report-md out/notice.report.md
```

### 转换 Markdown 通用材料（默认后端）

```bash
cargo run -p charta-cli -- convert examples/general.md \
  --output out/general.docx \
  --report out/general.report.json \
  --report-md out/general.report.md
```

### 转换 Typst 通知（含字面量 include）

```bash
cargo run -p charta-cli -- convert examples/notice.typ --backend gb9704 \
  --output out/typst-notice.docx \
  --report out/typst-notice.report.json \
  --report-md out/typst-notice.report.md
```

### 查看降级报告

```bash
cargo run -p charta-cli -- convert examples/degraded.typ --backend gb9704 \
  --output out/degraded.docx \
  --report out/degraded.report.json \
  --report-md out/degraded.report.md \
  --emit-ir out/degraded.ir.json
```

`degraded.typ` 包含循环、条件、`#set text`、`#show`、`#import` 和显式样式意图。默认配置会采纳颜色意图，覆盖字体/字号意图；报告会同时列出“不可静态确定”“样式意图被覆盖”“渲染告警”三类条目。

### 严格模式

默认策略是“降级 + 告警”。使用 `--strict` 可把契约外语法升级为阻断，阻止 docx 生成：

```bash
cargo run -p charta-cli -- convert examples/notice.md --backend gb9704 --strict
```

### 样式数据（FR-BE-5）

通知样式值位于 `crates/charta-backend-gb9704/data/styles.json`，通用材料样式值位于同目录的 `general-styles.json`，默认编译进二进制。`--styles <PATH>` 支持两种文件：

- **局部补丁**：只写要改的样式或段落/页面字段，其余沿用当前后端的内置值。
- **完整样式文件**：`styles` 是数组时视为完整数据，整份替换内置值。

补丁可以精确到单个样式，例如“正文字大一号、`strong` 改黑体”：

```json
{
  "styles": {
    "GbBody": { "run": { "size_step": 1 } },
    "GbStrong": { "run": { "east_asia_font": "方正黑体_GBK" } }
  }
}
```

```bash
cargo run -p charta-cli -- convert examples/general.md --styles examples/general-tweaks.json
```

`examples/general-tweaks.json` 就是上面这份补丁，可直接运行对照。

补丁规则：

- `styles` 以样式 ID 为键，只能修改已有样式；写错 ID 会直接报错。
- `run.size_step` 只调整该样式的字号：`1` 表示大一号，`-1` 表示小一号；`run.size_pt` 可直接指定字号，但二者不能同时写。
- `run`、`paragraph`、`defaults`、`page`、`table`、`image`、`version_record`、`footer`、`colors` 都可局部覆盖，没写的键保持内置值。
- 行内 `strong` / `emphasis` 从对应样式取字体和加粗设置，字号跟随所在段落。因此改 `GbStrong.run.east_asia_font` 会生效；单独改 `GbStrong` 的字号不会改变行内字号，行内字号由 `GbBody` 等段落样式决定。

### 标题编号的重置规则

`numbering.heading.restart` 控制下级标题编号是否随上级变化重置。默认 `{ "enabled": true }`，即 Word 的默认行为：任意上级编号递增时，下级重新从 1 开始。可以直接改内置数据，也可以写进 `--styles` 补丁：

```json
{
  "styles": {},
  "numbering": {
    "heading": {
      "restart": {
        "enabled": true,
        "never_from": 3
      }
    }
  }
}
```

- `enabled: false`：所有下级编号都不重置；`（一）`、`1.`、`（1）` 都接着往下数，不会因为上级变化回到 `（一）` 或 `1.`。
- `never_from: N`（2–4，可省略）：从第 N 级起不再重置。例如 `3` 表示 `1.` 和 `（1）` 不再随 `（一）` 重置，而 `（一）` 仍随 `一、` 重置。
- `never_from` 只能和 `enabled: true` 一起用；与 `enabled: false` 同时写会直接报错。不写 `restart` 或只写 `enabled: true` 时输出与之前一致。

补丁只为已有样式和字段服务：`restart` 已在内置数据中占位，因此补丁可直接增删 `never_from`。

库用户可自行加载完整文件或补丁：

```rust
use charta_backend_gb9704::{
    Gb9704Backend, Gb9704Options, Gb9704Styles, GeneralBackend, StyleSet,
};

fn build_notice() -> Result<Gb9704Backend, Box<dyn std::error::Error>> {
    let styles = Gb9704Styles::from_path_for("my-gb9704.json", StyleSet::Notice)?;
    Ok(Gb9704Backend::with_styles(Gb9704Options::default(), styles))
}

fn build_general() -> Result<GeneralBackend, Box<dyn std::error::Error>> {
    let styles = Gb9704Styles::from_external_for("tweaks.json", StyleSet::General)?;
    Ok(GeneralBackend::with_styles(styles))
}
```

`from_external_for` 会自动判断完整文件或补丁；`from_path_for` 只接受完整文件，`from_json_with_patch(base, patch, set)` 可把补丁应用到指定基础数据。也可用 `Gb9704Backend::with_style_file(options, path)` 或 `GeneralBackend::with_style_file(path)` 一步加载完整文件或补丁。样式数据缺失、出现未知字段或未知样式、枚举值非法、引用了未定义样式/颜色，或用错模板的必需样式集时都会直接返回错误，不会静默回退。列表静态序数 `（1）`/`①`/`A.`/`a.` 按 FR-BE-5 的例外保留在代码中。

### 查看后端与 schema

```bash
cargo run -p charta-cli -- backends
cargo run -p charta-cli -- schema --backend gb9704
cargo run -p charta-cli -- schema --backend gb9704 --json
cargo run -p charta-cli -- schema --backend general
```

### 通用材料模板（general）

`general` 是 CLI 默认后端，用于日常小材料：不带版头、版记，适合通知、工作安排、情况说明等短材料。需要完整公文形态时显式加 `--backend gb9704`。

大标题取自正文第一个一级标题（`#`），不用元数据 `title`；元数据只放署名和主送：

```yaml
---
author: 张三
date: 2026-10-25
recipient: 各部门
---

# 关于做好近期重点工作的通知

## 总体要求

### 加强统筹

#### 明确责任
```

生成顺序与字体：

- 大标题（`#`）：方正小标宋_GBK，二号，居中。
- 姓名：方正楷体_GBK，三号，居中，两字姓名中间加一个全角空格。
- 日期：方正楷体_GBK，三号，居中，两侧加全角括号，例如 `（2026年10月25日）`。姓名与日期分两行。
- `recipient` 可选：日期行之后左对齐，自动补全角冒号；不填就不生成。
- 正文：方正仿宋_GBK，三号。
- 一级标题（`##`）：方正黑体_GBK，三号。
- 二级标题（`###`）：方正楷体_GBK，三号，加粗。
- 三级及以下标题（`####` 等）：方正仿宋_GBK，三号，不加粗。
- `**加粗**`：方正楷体_GBK 加粗；`*斜体*`：方正楷体_GBK 不加粗，不生成真正斜体。
- 列表、表格、图片沿用后端能力；图片题注使用方正楷体_GBK。

标题层级整体比通知后端下移一级：通知的 `#` 是一级标题，通用材料的 `#` 是大标题，编号也从 `##` 开始（`#` 不编号）。出现多个 `#` 时，每个 `#` 开启新的编号区段，其后的 `##` 从 `一、` 重新编号；第二个及以后的 `#` 只按大标题排版，不重复姓名和日期。两字姓名中间加一个全角空格（`张三` → `张　三`），三字及以上原样。这是常见排版惯例，GB/T 9704 没有直接条款；单位另有模板时以模板为准。字体名按方正 GBK 名称写入 docx，目标机器未安装时由 Word 替换显示。

## CLI

```
charta convert <INPUT> [OPTIONS]
charta backends
charta schema --backend <ID> [--json]
charta --version
```

`convert` 主要参数：

| 参数 | 说明 |
|---|---|
| `--frontend auto\|md\|typst` | 默认按扩展名判断 |
| `--backend <ID>` | 默认 `general`（通用材料）；可选 `gb9704`（通知） |
| `-o, --output <PATH>` | docx 路径，默认与源文件同名 |
| `--report <PATH>` | JSON 报告，默认 `<output>.report.json` |
| `--report-md <PATH>` | Markdown 报告，默认 `<output>.report.md` |
| `--emit-ir <PATH>` | 额外输出 IR JSON |
| `--strict` | 契约外语法升级为阻断 |
| `--styles <PATH>` | 外部样式 JSON：完整样式文件或只含改动项的补丁 |
| `--no-header` | 不生成版头；`doc_number` 随之不再必填（仅 gb9704） |
| `--no-record` | 不生成版记及末页沉底分节（仅 gb9704） |
| `--adopt-color <BOOL>` | 是否采纳颜色意图，默认 `true` |
| `--adopt-font-family <BOOL>` | 是否采纳字体意图，默认 `false` |
| `--adopt-font-size <BOOL>` | 是否采纳字号意图，默认 `false` |
| `--adopt-bold <BOOL>` | 是否采纳加粗意图，默认 `false` |
| `--adopt-italic <BOOL>` | 是否采纳斜体意图，默认 `false` |

退出码：

| 退出码 | 含义 |
|---:|---|
| `0` | 转换完成，无阻断性问题 |
| `1` | 参数、I/O、前端或后端致命错误 |
| `2` | 转换完成但存在阻断性问题；docx 未生成（schema 校验失败等） |

## 输入契约

### Markdown

受控子集：

- ATX 标题 `#` ~ `######`
- 正文段落
- 有序/无序列表（支持嵌套；GB 输出四级序数 `（1）`/`①`/`A.`/`a.`，第五级起回落正文）
- GFM 管道表格
- 独立图片行（GB 后端嵌入 PNG/JPEG/GIF，按版心等比缩放）；Markdown title 作为题注，自动编号“图 N”
- 强调 `**加粗**`、`*斜体*`
- 引用 `>`
- YAML front matter

受控子集之外的语法（链接、引用式链接、脚注引用、行内代码、删除线、HTML/实体、围栏代码块、分隔线、Setext 标题、任务列表等）会产生 `unsupported_syntax` 报告，并按下述策略处理：默认降级为正文/提示告警；`--strict` 时阻断。

### Typst

只读 CST 子集，不做求值：

| 内容 | 处理 |
|---|---|
| `= / == / ===` 标题 | 映射为标题层级 |
| `*加粗*` / `_斜体_` / `#strong[...]` / `#emph[...]` | 映射为行内 `strong` / `emphasis`，可嵌套 |
| 内联 `#text(font: ..., size: ..., fill: ..., weight: ..., style: ...)[...]` | 记录为样式意图 |
| `-` / `+` 列表，按缩进嵌套 | 映射为 `ListBlock`，保留有序性与层级 |
| 整行 `#image("path")` | 映射为图片块；支持 `alt`、`caption`；`width`/`height`/`fit` 不采纳 |
| 整行 `#figure(image("path"), caption: [...])` | 图片块 + 题注 |
| 整行 `#table(columns: N, ...)` | 映射为表格块；单元格为字面量内容块 |
| `table.header[...]` / `table.cell(colspan: ..., rowspan: ...)` | 表头行与合并单元格 |
| 整行 `#figure(table(...), caption: [...])` | 表格块 + 题注 |
| 字面量 `#include "path.typ"`（整行） | 递归展开，循环/深度/读取失败显式报错 |
| `#for` / `#while` / `#if` | 提取内容块一次，标记“不可静态确定”，报告原因与影响 |
| `#set` / `#show` / `#import` / `#let`（`charta-meta` 除外） | 不应用，标记“不可静态确定”并报告 |
| 其他函数/变量、标签、引用、数学公式、行内原始文本 | 显式报告并降级 |
| 原始块、非字面量图片路径、动态表格参数 | 显式报告；能保留的文字按正文保留 |

Typst 没有 front matter，元数据写在**主文件**顶部的一个原生字面量绑定里：`#let charta-meta = (...)`。`#let` 在 Typst 编译时没有输出，所以同一份 `.typ` 既能交给 Charta，也能直接用 Typst 编译。被 include 的文件不解析元数据。YAML front matter 仅用于 Markdown；如果 `.typ` 仍以 `---` 开头，Charta 会报告迁移提示并跳过该区块。

Typst 表格只接受字面量子集：`columns` 为整数或元组（元组只取列数，列宽不采纳），单元格为 `[...]` 内容块或 `table.cell(...)`；`table.header` 的 `repeat`/`level`、`table.footer`、`table.hline`/`vline`、`align`/`stroke`/`inset`/`fill` 等版式参数会报告未采纳。未写 `columns` 时按单列处理并报告。图片路径与 include 一样按源文件目录解析；`#image` 的 `width`/`height`/`fit` 与 `#figure` 的 `kind`/`supplement`/`numbering` 不采纳，图片按后端最大宽高缩放，题注沿用后端静态编号。`#image`/`#table` 直接写 `caption:` 便于单行书写，属于 Charta 扩展；Typst 原生写法是包一层 `#figure(...)`。

```typst
#let charta-meta = (
  author: "张三",
  date: "2026-10-25",
)

= 关于近期重点工作的安排

== 适用范围

请 #strong[重点] 关注，_按计划_推进。

- 第一项
  - 子项

#table(
  columns: 2,
  table.header[环节][状态],
  [受理], [完成],
)

#figure(image("assets/chart.png"), caption: [合规率对比])
```

### 元数据入口

引擎不定义字段语义，元数据由目标后端的 schema 校验。建议通用字段命名：`title`、`date`、`author`。通用材料后端要求 `author`、`date`，`recipient` 可选；大标题取自正文第一个一级标题，元数据 `title` 不再使用（仍提供会报告 `metadata_ignored`）。GB/T 9704 后端要求：

| 字段 | 类型 | 必填 | 说明 |
|---|---|:---:|---|
| `title` | string | 是 | 公文标题 |
| `doc_number` | string | 是* | 发文字号；关闭版头时不必填 |
| `issuer` | string | 是 | 发文机关署名 |
| `date` | date | 是 | 成文日期，`YYYY-MM-DD` |
| `organization` | string | 否 | 发文机关标志，缺省用 `issuer` |
| `recipient` | string | 否 | 主送机关，后端补齐冒号 |
| `cc` | list<string> | 否 | 抄送机关 |
| `issuing_office` | string | 否 | 印发机关，缺省用 `issuer` |
| `issuing_date` | date | 否 | 印发日期，缺省用 `date` |
| `security_level` | enum | 否 | `绝密` / `机密` / `秘密` |
| `urgency` | enum | 否 | `特急` / `加急` |
| `copy_number` | string | 否 | 份号 |
| `attachments` | list<string> | 否 | 附件名称 |
| `note` | string | 否 | 附注，缺省自动加圆括号 |

通知属于下行文，不接受 `signer`；若元数据中仍提供，会报告 `metadata_ignored` 且不渲染。上行文的签发人口径留到请示/报告后端。

关闭版头后 `doc_number` 不再必填；`doc_number`、`copy_number`、`security_level`、`urgency` 若仍提供会报告 `metadata_ignored`。关闭版记后 `cc`、`issuing_office`、`issuing_date` 同理。

元数据入口按前端区分：

- **Markdown**：文件顶部的 YAML front matter（`---` 包裹）。解析器只支持 YAML 子集：映射、缩进列表、flow 列表、引号字符串、注释、布尔/整数/浮点/null。锚点、别名、标签、多行标量、flow mapping 会报阻断性解析错误。
- **Typst**：主文件顶部、第一个非空行处的 `#let charta-meta = (...)` 字典字面量。值只接受字符串、数字、布尔、`none`、列表与嵌套字典；`(x)` 按 Typst 语义是分组，`(x,)` 才是单元素列表，`(:)` 是空字典。`#let` 在 Typst 编译时没有输出，源文件仍可直接编译；旧的 YAML front matter 会报告 `unsupported_syntax` 并跳过，提示迁移到 `charta-meta`。

### 版头与版记开关

默认生成版头和版记，保持完整公文形态。只需要主体内容时可以分别关闭：

```bash
charta convert examples/report-with-image.md --backend gb9704 \
  --no-header --no-record \
  --output out/plain.docx
```

- `--no-header`：不生成份号、密级、紧急程度、发文机关标志、发文字号和红色分隔线，文档从标题开始。
- `--no-record`：不生成抄送、印发机关/日期和末页沉底分节，短公文不再因此多一页。
- 两个开关互相独立；页码不受影响。

### 标题自动编号

默认不自动编号，标题文本按原样呈现，因此作者手写的“一、”“（一）”不会重复。需要后端生成序号时，在元数据里显式开启：

Markdown：

```yaml
_charta:
  heading_numbering: true
```

Typst：

```typst
#let charta-meta = (
  author: "张三",
  date: "2026-10-25",
  _charta: (heading_numbering: true),
)
```

开启后，未带前端建议的标题都会请求自动编号。GB/T 9704 后端使用 Word 内置多级列表生成 `一、` / `（一）` / `1.` / `（1）` 四级编号，第三级 `1.` 与标题文字之间保留一个空格（`1. 标题`），其余级不加后缀空格；编号定义写入 `numbering.xml`，在 Word 中增删标题会自动重排。超过四级不编号并报告 `heading_numbering_flattened`；首次生成报告 `heading_numbering_word`，提示编号由 Word 维护、纯文本抽取不包含编号文字。`heading_numbering: false` 显式关闭。

通用材料模板的 `#` 是大标题，不编号；从 `##` 开始按上述四级编号依次对应 `一、` / `（一）` / `1.` / `（1）`，`#####` 及更深层级报告 `heading_numbering_flattened`。每个 `#` 开启新的编号区段，其后的 `##` 从 `一、` 重新开始；第二个及以后的 `#` 只按大标题排版，不重复署名区。区段内部的下级是否随上级重置，由样式数据 `numbering.heading.restart` 控制（见“标题编号的重置规则”）。

列表编号仍是后端生成的静态文本，不使用 Word 列表编号。若外部样式数据没有 `numbering.heading` 定义，标题会回退到静态文本编号并报告 `heading_numbering_static`。

### 图片题注

独立图片行末尾的 Markdown title 会作为题注，排在图片下方：

```markdown
![2026 年度公文格式检查结果](assets/format-check-chart.png "各环节合规率对比")
```

生成“图 1　各环节合规率对比”。题注使用楷体四号、居中，并与图片保持同页。序号按带题注图片的出现顺序从 1 开始，是静态文本而不是 Word 自动编号；首次生成报告 `image_caption_static`。不写 title 就不生成题注，alt 仍作为图片替代文字。

扩展结构角色通过引擎控制元数据 `_charta.roles` 声明，例如：

Markdown：

```yaml
_charta:
  roles:
    - name: appendix
      description: 附录
```

Typst：

```typst
#let charta-meta = (
  title: "通用材料",
  author: "张三",
  date: "2026-10-25",
  _charta: (
    roles: (
      (name: "appendix", description: "附录"),
    ),
  ),
)
```

`_charta` 命名空间属于引擎控制信息，不作为后端业务字段校验。

## 报告

JSON 报告结构在 1.x 系列内稳定：只增字段，不改已有字段含义。条目类型：

| 类型 | 来源 | 关键信息 |
|---|---|---|
| `schema_validation` | 元数据校验 | 字段、期望、实际 |
| `style_overridden` | 后端 | 源位置、意图值、实际采用值、原因 |
| `non_deterministic` | Typst 前端 / 后端 | 源位置、原因、影响范围 |
| `unsupported_syntax` | 两个前端 | 源位置、语法片段、处理方式 |
| `render_warning` | 后端 | 代码、上下文 |

severity 为 `blocking` 或 `warning`。默认只有 schema 校验（含 `charta-meta` / front matter 解析）、include 失败/循环是阻断性的；`--strict` 会把 `unsupported_syntax` 也升级为阻断。

## 版本管理

- workspace 版本单一来源：根 `Cargo.toml` 的 `[workspace.package]`，所有 crate 使用 `version.workspace = true`。
- 内部依赖版本集中在 `[workspace.dependencies]`。
- IR 与报告格式版本独立维护（当前 IR `1.1`、报告 `1.0`）。
- 递增版本：`bash scripts/bump-version.sh 0.2.0`，然后更新 `CHANGELOG.md` 并执行发布检查。
- 详见 [docs/VERSIONING.md](docs/VERSIONING.md)。

## 已知限制（0.1.0）

- 已提供 GB/T 9704 通知与通用材料两个后端；会议纪要、请示、函、企业公文后端在后续阶段。
- 通用材料后端以 Markdown 使用为主；表格题注的后端渲染已存在，但 Markdown 语法入口尚未实现。
- 图片只支持 PNG/JPEG/GIF；读取失败、格式不支持或尺寸解析失败会报告 `image_read_failed` / `image_decode_failed` 并跳过该图，不阻断转换。
- Typst 已支持标题、行内强调、嵌套列表、图片与受限表格；动态内容（`#for`/`#if` 结果、变量路径、动态列数）仍按“不可静态确定”报告，数学公式与原始块只保留文字。
- Typst 表格不接受动态单元格、`table.footer` 与表格线调用；列宽与单元格填充等视觉参数沿用后端样式，未采纳的项在报告中列出。
- 表格 `colspan`/`rowspan` 已渲染为 Word 网格合并；后续行缺少被合并列的单元格时自动补空，合并超出最后一行会报告 `table_rowspan_truncated`。
- 版记通过末节底端对齐，整块固定另起一页并置于最后一页版心底部。这样不会被 Word 分页拆开，但短公文会多出一页；不需要版记时可用 `--no-record` 关闭，同页沉底留待后续版面测量优化。
- GB/T 9704 列表按层级生成 `（1）` / `①` / `A.` / `a.` 四级静态序数，第五级起按正文段落渲染并产生 `list_nesting_flattened` 告警；列表序号与标题层级解耦，每个列表独立从第一级开始。
- 标题自动编号使用 Word 内置多级列表；前端目前只有文档级 `_charta.heading_numbering` 开关，逐标题显式标记在 IR 中已支持（`auto_number`），但 Markdown/Typst 语法尚未提供。列表编号仍为静态文本。
- Typst 前端是面向第一期契约的 CST 子集扫描器，不是完整 Typst 求值器；`#set text` 不做作用域模拟。
- 正文字体名称按后端样式数据写入，实际显示取决于操作系统是否安装 `方正小标宋简体`、`仿宋_GB2312`、`楷体_GB2312` 或通用材料所需的 `方正小标宋_GBK`、`方正黑体_GBK`、`方正楷体_GBK`、`方正仿宋_GBK` 等字体。
- CLI 只处理单文档；目录批处理在后续阶段。

## 验收对照

| 验收标准 | 证据 |
|---|---|
| 1. Markdown 通知经通知后端生成可核对的 docx | `examples/notice.md` 实际转换；`cargo test -p charta-backend-gb9704`；Word COM 转 PDF 人工核对 |
| 2. 循环/条件 Typst 稿的降级报告可定位源码 | `examples/degraded.typ` 报告；`charta-typst` 单元测试；CLI 集成测试检查 `file/line` |
| 3. 至少一类样式意图被采纳并形成报告链路 | 默认采纳颜色（`--adopt-color true`）；`degraded.typ` 中颜色生效、字体/字号进入覆盖清单 |
| 4. 切换后端时引擎核心零改动 | 核心管线只依赖 `Backend`/`Frontend` trait；`charta` 管线测试使用独立假后端 |
| 5. 现有工具以库依赖方式调用核心 | `charta` 提供 `convert`、IR、报告 API；CLI 只是薄壳；无 CLI 依赖 |
| 6. 从零程序化生成 docx，无模板字段 | `charta-docx-kit` 生成完整 OOXML 包；GB 后端测试；IR 中无模板字段 |

## 文档

- [设计文档](docs/DESIGN.md)
- [版本管理](docs/VERSIONING.md)
- [变更记录](CHANGELOG.md)

# Charta 设计文档

- 引擎版本：`0.2.0`
- IR 格式：`1.1`
- 报告格式：`1.0`
- 适用：第一期 MVP（Markdown / Typst → IR → GB/T 9704 通知 → docx + 报告）

## 1. 目标与边界

### 1.1 设计目标

1. 同一份内容可通过不同规范后端渲染为符合对应版式规范的 docx。
2. 前端只写内容、填元数据；视觉形态完全由后端决定。
3. 输入源中的显式样式意图被记录、可审计，但不默认生效。
4. Typst 在静态可确定范围内提取结构；超出范围的内容显式降级并报告。
5. 核心以 library 形态提供，CLI 仅为薄壳。
6. 报告系统在第一期定型，区分阻断性错误与非阻断告警。

### 1.2 非目标

- 不做通用 Markdown/Typst 到 docx 的全保真转换。
- 不承诺 Typst 源文件的视觉还原。
- 不在引擎层定义公文规范语义或元数据字段语义。
- 不替代后端做分页、页码域、版头版记拼接等公共渲染逻辑。
- 第一期不做 Typst 求值（循环展开、条件分支、`import` 符号追踪）。
- 不做 PDF/HTML 等其他输出格式。
- 不在引擎层管理 docx 模板文件。

## 2. 总体架构

```
 Markdown ─┐                            ┌──────────────┐
           ├─► 前端 ─► IR + 样式意图 ─► │ 后端 schema   │ ─► 规范后端 ─► docx
 Typst ────┘        + 确定性标记        │   校验        │      │
                                        └──────────────┘      │
                                                │              │
                                                ▼              ▼
                                        ┌─────────────────────────┐
                                        │ 报告系统（JSON/Markdown）│
                                        └─────────────────────────┘
```

关键边界：

- 前端与后端**只通过 IR 交换数据**，不共享解析器、样式表或模板。
- 后端接收 `IR + 元数据`，返回完整 docx 字节与报告条目。
- 引擎不参与分页、页码域、版头/版记拼接；这些属于后端与工具箱。
- 报告由三处汇入：前端解析、引擎校验、后端渲染。

## 3. crate 边界

| crate | 职责 | 依赖 |
|---|---|---|
| `charta` | IR、前端/后端 trait、schema 校验、角色、管线、报告 | serde、serde_json、thiserror |
| `charta-md` | Markdown 受控子集 → IR | `charta` |
| `charta-typst` | Typst 只读 CST 子集 → IR | `charta` |
| `charta-docx-kit` | OOXML 机械操作与 zip 打包 | zip、thiserror |
| `charta-backend-cn` | 中文公文模板：通知、通用材料、文件汇编 | `charta`、`charta-docx-kit` |
| `charta-cli` | 参数解析、文件读写、退出码 | 全部上层 crate、clap |

`charta` 不依赖任何前端、后端或 docx 库；新增后端不需要修改核心。

## 4. IR 设计

IR 是唯一契约，承载**结构语义 + 样式意图 + 来源位置 + 确定性**，不承载“版头”“版记”等规范语义。

### 4.1 Document

```rust
pub struct Document {
    pub ir_version: String,                 // "1.1"
    pub frontend: FrontendId,               // markdown | typst
    pub metadata: Metadata,
    pub blocks: Vec<Block>,
    pub declared_roles: Vec<RoleDeclaration>,
}
```

`metadata.fields` 是后端 schema 驱动的前端原始值；引擎不解释字段含义。

### 4.2 Block 与 Inline

`Block` 由 `BlockKind + SourceSpan + Determinism` 组成，`BlockKind` 以 `type` 标签序列化：

```rust
pub enum BlockKind {
    Heading { level: u8, content: Vec<Inline> },
    Paragraph { content: Vec<Inline> },
    List(ListBlock),
    Table(TableBlock),
    Image(ImageBlock),
    Quote { blocks: Vec<Block> },
    Extended { role: String, blocks: Vec<Block> },
}
```

基线结构角色：标题 1–6 级、正文、列表、表格、图片、强调、引用。强调以 `Inline.strong` / `Inline.emphasis` 表达，仍属于结构角色；后端决定其视觉表现。

`Extended` 用于文档级声明的扩展角色。未知角色的内容按透明容器渲染，由后端决定具体语义。

### 4.3 样式意图

```rust
pub struct StyleIntent {
    pub property: StyleProperty, // font-family/font-size/color/bold/italic
    pub frontend: FrontendId,
    pub source: SourceSpan,
}
```

- 意图挂在 `Inline.intents` 上，可表达局部样式。
- 意图是**建议值**，不直接参与渲染。
- 后端必须逐条决定采纳或覆盖；覆盖项写入 `style_overridden` 报告，包含源位置、意图值、实际值、原因。
- 第一期默认只有 GB/T 9704 的 `color` 采纳，其余属性需通过 CLI 显式开启。

### 4.4 确定性标记

```rust
pub enum Determinism {
    Deterministic,
    NonDeterministic { reason: String, impact: String },
}
```

Typst 循环/条件/`set`/`show`/`import`/未知调用产生的节点带 `NonDeterministic`。内容仍以“尽力而为”的结构进入 IR，但必须同时产生报告。Markdown 节点在受控子集内均为确定。

### 4.5 源位置

```rust
pub struct SourceSpan {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub byte_start: usize,
    pub byte_end: usize,
}
```

行、列均为 1 起始；列按 Unicode 标量计数；字节偏移按 UTF-8。Typst include 展开后，节点与报告位置指向被包含文件。

### 4.6 角色与 `_charta` 控制元数据

`RoleRegistry` 在运行期维护基础角色与后端注册角色。Markdown 文档在 front matter、Typst 文档在 `charta-meta` 中声明扩展角色：

```yaml
_charta:
  roles:
    - name: appendix
      description: 附录
```

Typst 写在 `charta-meta` 的 `_charta` 映射里：

```typst
#let charta-meta = (
  author: "张三",
  date: "2026-10-25",
  _charta: (
    roles: (
      (name: "appendix", description: "附录"),
    ),
  ),
)
```

- `_charta` 是引擎控制命名空间，不属于任何后端业务 schema。
- 声明与使用分离：`Extended` 节点引用未声明角色是阻断性错误。
- 后端未注册但文档已声明的角色产生 `render_warning`（非阻断），内容按透明容器处理。
- 这解决了需求 §9.3 的“编译期 trait vs 运行期注册表”：第一期选择运行期注册表，便于后端独立扩展。

### 4.7 序列化示例

以下为结构示意，字节偏移为示例值。

```json
{
  "ir_version": "1.1",
  "frontend": "markdown",
  "metadata": {
    "fields": {
      "doc_number": "昆政发〔2026〕1号",
      "issuer": "昆山市人民政府",
      "date": "2026-10-25"
    }
  },
  "blocks": [
    {
      "type": "heading",
      "level": 2,
      "content": [
        {
          "text": "一、检查范围",
          "source": {
            "file": "examples/notice.md",
            "line": 22,
            "column": 3,
            "end_line": 22,
            "end_column": 9,
            "byte_start": 480,
            "byte_end": 486
          }
        }
      ],
      "source": {
        "file": "examples/notice.md",
        "line": 22,
        "column": 1,
        "end_line": 22,
        "end_column": 9,
        "byte_start": 478,
        "byte_end": 486
      },
      "determinism": { "kind": "deterministic" }
    }
  ]
}
```

## 5. 前端

### 5.1 Markdown 前端

实现方式：手写行级块解析 + 递归内联解析，保留每个节点的字节/行列范围。

| 输入 | IR |
|---|---|
| `#` ~ `######` | `Heading{level}` |
| 非空段落行 | `Paragraph` |
| `-` / `*` / `+` / `1.` 列表 | `List`（支持缩进嵌套） |
| GFM 管道表格 | `Table` |
| 独立 `![alt](path)` | `Image` |
| `**`/`*`/`__`/`_` | 内联 `strong`/`emphasis` |
| `>` | `Quote` |
| `---` front matter | `Metadata` |

受控子集外的语法逐条报告：

- 链接：保留链接文字，丢弃 URL；引用式链接另外丢弃 `[ref]` 标签。
- 脚注引用：`[^1]` 按字面文本保留。
- 行内代码：移除反引号，内容按普通文本保留。
- 删除线：移除 `~~` 标记，内容按普通文本保留。
- 围栏代码块：代码内容按纯文本段落保留。
- HTML：按纯文本段落保留；已知 HTML 实体解码为字符，未知实体按字面保留。
- 分隔线、Setext 标题、链接引用定义、任务框：忽略或移除标记并报告。
- 行尾双空格/反斜杠硬换行：按空格合并并报告。

默认策略是“降级 + 告警”，`--strict` 将 `unsupported_syntax` 升级为阻断。这是对需求 §9.1 的落地选择。

front matter 只接受 YAML 子集（映射、缩进/flow 列表、引号字符串、注释、布尔/整数/浮点/null）；不支持 YAML 高级特性时产生阻断性解析错误，不猜测语义。

### 5.2 Typst 前端

实现方式：**只读 CST 子集扫描器**，不引入求值器。源文本先做等长注释替换（注释字节替换为空格，保留换行与全部偏移），再按行/括号平衡解析。

| 构造 | 处理 |
|---|---|
| `#let charta-meta = (...)`（主文件顶部） | 元数据入口；只接受字符串、数字、布尔、`none`、列表与嵌套字典字面量，编译时无输出 |
| `= / == / ===` | 标题；超过 6 级截断为 6 并报告 |
| `*...*` / `_..._` / `#strong[...]` / `#emph[...]` | 行内 `strong` / `emphasis`；`#strong` 与 `#emph` 等价于标记语法，内容继续递归解析 |
| `#text(font, size, fill, weight, style)[...]` | 解析命名参数，生成 `StyleIntent` |
| `-` / `+` 列表（按缩进嵌套） | 映射为 `ListBlock` / `ListItem.children`；`+` 为有序，`-` 为无序，续行并入当前项 |
| 整行 `#image("path", ...)` | 映射为 `ImageBlock`；路径必须是字面量，`alt`、`caption` 采纳，`width`/`height`/`fit` 报告未采纳 |
| 整行 `#figure(image(...), caption: [...])` | 映射为带题注的 `ImageBlock`；其他 figure 参数报告未采纳 |
| 整行 `#table(columns: N, ...)` | 映射为 `TableBlock`；`columns` 为整数或元组（只取列数），单元格按 `columns` 分行 |
| `table.header[...]` / `table.cell(...)` | 表头行、`colspan`/`rowspan`/`align`；`repeat`/`level`、`table.footer`、表格线与视觉参数报告未采纳 |
| 整行 `#figure(table(...), caption: [...])` | 映射为带题注的 `TableBlock` |
| `#include "literal.typ"`（整行） | 解析相对路径、递归展开；循环/深度/读取失败为阻断性错误 |
| `#for` / `#while` | 定位第一个顶层 `[...]` 内容块，按字面量提取一次，标记 `NonDeterministic` |
| `#if` | 提取 then 分支内容，忽略 else，标记 `NonDeterministic` |
| `#set` / `#show` / `#import` / `#let` | 不应用，报告“需要求值” |
| 其他函数/变量 | 报告不支持；函数后的内容块文字按普通文本保留 |
| 数学公式 `$...$` | 报告不支持，公式源码按字面文本保留 |
| 标签 `<...>` | 报告并忽略 |
| 引用 `@...` | 报告，按字面文本保留 |
| 原始块 ```...``` | 报告不支持，内容按纯文本段落保留 |

块级 `#image` / `#table` / `#figure` 只识别**整行独立调用**（缩进不超过 3 个空格，闭合后同一行没有其他内容），允许调用跨行；行内出现的同类调用仍按未知调用降级。表格缺少 `columns` 时按单列处理并报告，表头单元格数可作为推断列数的兜底。动态列数、非字面量图片路径、无法静态解析的单元格参数都会报告并跳过对应内容，不执行求值。

降级报告三要素：

1. **源文件位置**：include 展开后仍指向真实文件。
2. **原因**：例如“循环生成的内容不可静态确定”。
3. **影响范围**：例如“循环体按字面量提取一次；实际重复次数与变量替换未知”。

Typst 没有 front matter。Charta 在主文件的第一个非空行读取原生绑定 `#let charta-meta = (...)`：`#let` 在 Typst 编译时没有输出，因此同一份源文件既可由 Charta 使用，也可由 Typst 直接编译。值只接受 Typst 字面量子集——字符串、整数/浮点、布尔、`none`、列表与嵌套字典；`(x)` 是分组，`(x,)` 才是单元素列表，`(:)` 是空字典；动态表达式、单位数值、重复键与未闭合括号产生阻断性解析错误。被 include 的文件不解析元数据。旧的 YAML front matter 在 Typst 文件中不再解析，会报告 `unsupported_syntax`、跳过该区块并提示迁移。

注释处理采用保守规则：`//` 紧跟冒号（如 URL `https://`）不视为注释；`/* ... */` 支持嵌套。该规则写入测试，避免破坏正文 URL。

第一期不实现 `#set text` 作用域模拟、循环展开、条件求值、`import` 追踪——这些在需求中本就被列为不可静态确定。

### 5.3 编号与结构语义

#### 5.3.1 编号语义

编号属于结构语义，不属于视觉样式。前端只描述结构并给出编号建议，IR 不固化具体符号形式，后端拥有最终呈现权；列表序号不得写入 IR 的列表文本。标题只有在请求自动编号时，序号才不得写入标题文本。

- **FR-IR-4（列表）**：IR 的列表节点须记录有序性、起始值和嵌套层级，不记录具体符号形式。0.1.0 以 `ListBlock { ordered, start, items }` 加 `ListItem.children` 表达：层级由子 `ListBlock` 的嵌套关系推导，前端不得把嵌套列表拍平。
- **FR-IR-5（标题）**：标题节点须携带“期望自动编号”标记。前端可给出建议，后端可覆盖该建议，覆盖结果须可审计。IR 1.1 以带默认值的兼容增项加入 `auto_number: Option<bool>`：`None`/`false` 表示不请求自动编号，标题文本按原样呈现（允许作者手写序号）；`true` 表示请求后端生成，前端不得再写入序号。
- **FR-BE-4（后端）**：标题层级编号的形式与生成方式由后端决定。若后端采用静态文本编号，须在转换报告中标注所用形式，并说明该编号不是自动编号；若采用 Word 自动编号，须说明编号由 Word 维护，纯文本抽取不包含编号文字。
- **FR-KIT-4（工具箱）**：`charta-docx-kit` 提供构建 `numbering.xml` 的能力，包括抽象编号、编号实例和各层级格式；后端可以选择不使用该能力，自行决定列表与标题的编号实现。

0.1.0 落地情况：列表已按 FR-IR-4 记录 `ordered`/`start`，嵌套通过 `ListItem.children` 表达；Markdown 与 Typst 前端都保留列表层级，列表序号不写入 IR。列表仍使用 `（1）` / `①` / `A.` / `a.` 四级静态序数，第五级起回落正文。`charta-docx-kit` 的 `numbering.xml` 构建能力只用于标题，列表不挂 `numPr`。

标题自动编号通过引擎控制元数据 `_charta.heading_numbering: true|false` 显式开启或关闭，并写入未带前端建议的标题节点。默认不自动编号，避免与作者手写的“一、”重复。GB/T 9704 后端对请求自动编号的标题使用 Word 内置多级列表，生成 `一、` / `（一）` / `1.` / `（1）` 四级编号，超过四级不编号并报告 `heading_numbering_flattened`；首次生成报告 `heading_numbering_word`，说明编号由 Word 维护、纯文本抽取不含编号文字。通用材料模板的 `#` 是大标题、不编号，`##` 起才对应这四级；文件汇编模板的 `#` 是篇章标题、同样不编号，`##` 起默认自动编号，可用 `_charta.heading_numbering: false` 关闭。下级是否随上级变更重置由 `numbering.heading.restart` 控制：默认按 Word 规则重置，可整体关闭，或指定从第几级起不再重置（写入 `w:lvlRestart w:val="0"`）。样式数据缺少 `numbering.heading` 时回退为静态文本，并报告 `heading_numbering_static`。IR 的 `auto_number: Option<bool>` 已支持逐标题覆盖，前端语法尚未提供。

图片题注不进入标题编号体系：IR 1.1 的 `ImageBlock` 增加可选 `caption` 字段（带默认值的兼容增项），前端把 Markdown 图片 title 写入其中；GB/T 9704 后端按带题注图片的出现顺序生成静态 `图 N`，首次生成报告 `image_caption_static`。无 `caption` 的图片不编号。

## 6. 后端契约

### 6.1 trait

```rust
pub trait Backend: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &str;
    fn schema(&self) -> &MetadataSchema;
    fn roles(&self) -> Vec<RoleDeclaration> { Vec::new() }
    fn render(&self, request: RenderRequest<'_>) -> Result<RenderOutput>;
}
```

`RenderRequest` 只包含 `&Document` 与 `&Metadata`；没有模板、文件系统路径或前端类型。`RenderOutput` 返回 docx 字节与后端自有问题。

### 6.2 schema

`MetadataSchema` 由后端构造，字段支持：

- 类型：string、integer、number、boolean、date（ISO `YYYY-MM-DD`）、list<string>、any。
- 必填性。
- 枚举值。
- 描述（用于 `charta schema`）。

校验失败报告字段名、期望、实际值；未知字段被忽略并转发，保证 schema 向前兼容。引擎代码中不存在任何业务字段名。

### 6.3 样式意图与视觉权威

- 后端默认不采纳任何意图。
- GB/T 9704 后端默认显式采纳 `color`，因为颜色可用于强调；字体、字号、加粗、斜体默认覆盖。CLI 可逐项开启。
- 每个未采纳意图生成 `style_overridden`，包含：
  - 源位置
  - 属性与意图值
  - 实际采用值
  - 覆盖原因
- 采纳的意图不产生告警；验收链路通过“颜色采纳 + 字体/字号覆盖”同时体现。

### 6.4 GB/T 9704 通知版式

| 项 | 实现 |
|---|---|
| 纸张 | A4 210×297 mm |
| 页边距 | 上 37、下 35、左 28、右 26 mm |
| 页脚距边界 | 28 mm（版心下 7 mm） |
| 正文字体字号 | 3 号仿宋（16 pt） |
| 行距 | 固定 28 pt |
| 正文首行缩进 | 2 字符（`firstLineChars=200` + 640 twips 回退） |
| 列表 | 3 号仿宋；第一级 `（1）（2）…`、第二级 `①②…`、第三级 `A.B.…`、第四级 `a.b.…`；第五级起回落正文段落 |
| 一级标题 | 3 号黑体 |
| 二级标题 | 3 号楷体 |
| 三级及以下标题 | 3 号仿宋加粗 |
| 标题编号（可选） | `_charta.heading_numbering` 开启；Word 内置多级列表生成 `一、` / `（一）` / `1.` / `（1）`；三种模板都从 `##` 起编号；超过四级不编号并告警 |
| 大标题（正文首个 `#`） | 2 号小标宋（22 pt），居中；元数据 `title` 不再使用 |
| 发文机关标志 | 36 pt 小标宋，红色 `C00000`，居中；名称超出版心时按比例做字符缩放，字高不变、保持单行 |
| 发文字号 | 3 号仿宋，居中 |
| 红色分隔线 | 段落底边框，1.5 pt 红色，宽度随版心 |
| 主送机关 | 3 号仿宋，左对齐，缺省补全角冒号 |
| 附件说明 | 3 号仿宋，列出附件名称 |
| 署名/日期 | 右对齐；偏移量由样式 `right_chars`（署名 4 字、日期 2 字）换算成行尾普通空格写入正文，段右缩进置零并写 `wordWrap=0`，打开 docx 后可直接增删空格微调 |
| 附注 | 4 号仿宋，首行缩进 2 字符，自动补圆括号 |
| 表格 | 4 号仿宋，表头加粗，0.5 pt 全框线；支持 `colspan`/`rowspan` 网格合并 |
| 图片 | 居中；按 156×200 mm 版心等比缩放；支持 PNG/JPEG/GIF，读取或解码失败报告并跳过 |
| 图片题注 | 图片 title 转题注；`图 N` 静态编号；楷体四号、居中、段前 3 pt，与图片同页 |
| 版记 | 4 号仿宋，上下 0.5 pt 横线；抄送行 + 印发机关/日期双栏；末节底端对齐，末节按偶数页起始（必要时 Word 补空白页），空白页与版记页不编页码 |
| 页码 | 4 号宋体，`— PAGE —`，单页居右、双页居左，空 1 字 |
| 版头/版记开关 | 默认生成；后端选项可分别关闭（CLI `--no-header` / `--no-record`） |

版式元素全部由后端根据自身样式数据生成，不读取任何 `.docx` 模板。字体名称按国标写入；是否可用取决于运行环境。

通知的大标题取正文第一个 `#`，`##` 起为一级标题，与通用/汇编一致；元数据 `title` 不再参与渲染（仍提供会报告 `metadata_ignored`）。所有非引用标题都写 `w:outlineLvl`，引用块里的标题不写。

署名与日期的右空不用段右缩进，而是按 4 个普通空格约等于 1 个全角字宽，换算后追加到文本末尾（16 pt 仿宋下 1 字 = 4 个空格）；段落 `w:ind` 同时写 `rightChars="0"` 和 `right="0"`，并写 `w:wordWrap w:val="0"`。Word/WPS 默认会把右对齐段落的行尾普通空格当空白忽略，关掉 `wordWrap` 后才会计入行宽。这样生成后可直接在 Word 里增删空格微调，不必改样式再重生成。

版头与版记是后端版面选项（`DocOptions::header` / `record`），默认开启，关闭不改变 IR。关闭版头后文档从标题开始，`doc_number` 不再必填，仍提供的版头专属元数据报告 `metadata_ignored`；关闭版记后取消末节分节与沉底，短公文不再因此多页，`cc`、`issuing_office`、`issuing_date` 若仍提供同样报告忽略。页码不受两个开关影响。

列表序数与标题层级解耦：每个列表独立从第一级开始，有序列表的 `start` 用于起始序号；四级以内分别生成 `（1）`、`①`、`A.`、`a.` 静态文本序数，第五级起按普通正文段落渲染并报告 `list_nesting_flattened`。列表不使用 `numbering.xml`，避免列表编号与标题多级列表共用编号状态；标题请求自动编号时只给标题段落挂 `numPr`。

### 6.5 已知版式限制

版记通过末节底端对齐：在附注之后插入 `nextPage` 分节，末节设置 `w:vAlign=bottom`，版记整块沉到最后一页版心底部，且不会被 Word 分页拆开。代价是版记固定另起一页，短公文会多一页；同页沉底需要版面测量或浮动定位，可在后端内部优化，不影响 IR 或核心契约。不需要版记时可关闭 `record` 选项，分节与沉底一并取消。

表格合并按行从左到右放置单元格，`rowspan` 覆盖的后续行自动生成 `vMerge=continue` 占位单元格；后续行不要重复写被覆盖列的单元格，否则会被当作该行下一个普通单元格。合并超出最后一行报告 `table_rowspan_truncated`，单元格超出网格列数报告 `table_cells_overflow`。

### 6.6 样式数据外置

**FR-BE-5（样式外置）**：后端的样式值须可外置为数据文件，渲染流程代码不得内联样式字面量。样式值包括但不限于字体、字号、颜色、页边距、行距、缩进、边框、表格列宽和编号定义。例外：当标题编号采用静态文本拼接实现时，编号符号可作为流程逻辑的一部分保留在代码中；当采用 Word 自动编号时，编号定义（抽象编号、编号格式、层级文本、缩进等）属样式数据，须外置。

**FR-BE-6（数据格式）**：样式数据文件的格式由后端自行决定，引擎不规定；引擎只依赖后端的 `Backend` 契约，不读取或校验样式数据文件。

0.1.0 落地情况：已满足 FR-BE-5。GB/T 9704 通知后端的样式值集中在 `crates/charta-backend-cn/data/styles.json`，通用材料后端使用同目录的 `general-styles.json`，两者共用加载与校验逻辑、按各自必需样式集校验；采用带 `schema` 版本号的 JSON（本后端自定格式）。文件覆盖：

- 页面尺寸、页边距、页眉页脚距、正文/版记页的垂直对齐
- 段落样式：对齐、固定行距、段前段后、缩进、`keepNext`、边框
- run 样式：西文/中文字体、字号、加粗、颜色
- 表格：版心宽度、单元格边距、对齐、框线、单元格垂直对齐
- 图片：最大显示宽高
- 版记：总宽、双栏列宽、单元格边距、日期列对齐、框线
- 页脚：页码字体、`— PAGE —` 前后缀、奇偶页对齐与缩进
- 标题编号：`numbering.heading` 的抽象编号/编号实例 ID、多级类型、四级 `numFmt`/`lvlText`/起始值、缩进、后缀与重置规则（`restart.enabled` / `restart.never_from`）

渲染流程只按样式 ID 取 `RunConfig` / `TableConfig` / `VersionRecordConfig` / `FooterConfig` 并套用，不再内联样式值。列表静态序数 `（1）`、`①`、`A.`、`a.` 属于 FR-BE-5 的例外，保留在流程逻辑中。标题使用 Word 自动编号时，抽象编号、层级格式、层级文本与缩进已外置到 `numbering.heading`，由 `charta-docx-kit` 写入 `numbering.xml`；样式数据缺少该段时回退静态编号并报告。

加载入口为 `DocStyles::embedded_for` / `from_json_for` / `from_path_for`，按 `StyleSet::Notice` 或 `StyleSet::General` 选择必需样式集；`from_external_for` 自动识别完整文件与补丁，`from_json_with_patch` 把补丁应用到调用方给定的基础数据；`embedded` / `from_json` / `from_path` 是通知样式集的简写。后端可用 `with_styles` / `with_style_file` 接收外部数据，通用材料后端会再次调用 `validate(StyleSet::General)`，防止把通知样式集误用于通用模板。加载时校验 `schema`、重复/缺失样式、`based_on` 指向、颜色引用、必需 `run`、数值范围，并拒绝未知字段和未知枚举值；失败直接返回错误，不做静默回退。样式数据随后端 crate 发布并与后端版本绑定，不进入 IR，引擎核心也不读取或校验该文件。

局部样式覆盖由外部补丁实现：`--styles` 指向的文件若 `styles` 是对象（按样式 ID 索引），就与当前后端的内置数据递归合并；若 `styles` 是数组，则视为完整样式文件并整份替换。合并支持 `styles[*].run`、`styles[*].paragraph`、`defaults`、`numbering`、`page`、`table`、`image`、`version_record`、`footer` 和 `colors`，未写的键保持基础值。

`numbering.heading.restart` 控制下级标题编号的重置：`enabled` 默认 `true`（Word 默认，任意上级递增即重置下级），设为 `false` 时各级都保持连续；`never_from`（2–4）指定从第几级起不再重置，例如 `3` 让 `1.` / `（1）` 跨 `（一）` 连续、`（一）` 仍随 `一、` 重置。转换时按级写入 `w:lvlRestart w:val="0"`（永不重置），未命中的级别不写该元素。`never_from` 与 `enabled: false` 互斥，越界或冲突直接报错。

补丁里的 `styles.<ID>.run.size_step` 按中文字号阶梯 `42 / 36 / 26 / 24 / 22 / 18 / 16 / 15 / 14 / 12 / 10.5 / 9 / 7.5 / 6.5 / 5.5 / 5` 只调整该样式的字号，`1` 为大一号、`-1` 为小一号；也可以用 `run.size_pt` 指定精确值，但二者不能同时出现。`size_step` 以基础样式当前的 `size_pt` 为起点，越界或不在阶梯上返回带样式 ID 的错误。补丁只能引用已有样式，写错 ID 直接报错，避免“改了但没生效”的静默失败。合并结果仍走同一套 `RawFile` 校验、必需样式集校验与 `deny_unknown_fields`，不会绕过后端契约。

### 6.7 后端内部结构

规范后端不是配置文件，也不是纯代码，而是“契约实现 + 数据 + 流程 + 报告”的 crate。`charta-backend-cn` 承载通知、通用材料和文件汇编三个模板，按职责拆分：

| 路径 | 职责 |
|---|---|
| `lib.rs` | `Backend` trait 实现、构造器、渲染编排（含版头/版记开关） |
| `schema.rs` | 元数据 schema；校验由引擎执行 |
| `styles.rs` + `data/styles.json` / `data/general-styles.json` | 样式值与标题编号定义加载、校验与 docx-kit 转换（通知/通用材料各自必需集） |
| `layout/header.rs` | 版头：份号、密级/紧急程度、机关标志、发文字号、红线 |
| `layout/body.rs` | 主体：标题、主送机关、IR 块、附件说明、署名日期、附注；列表、表格、图片与题注流程 |
| `layout/record.rs` | 版记：末节底端对齐、抄送与印发机关/日期表格 |
| `layout/text.rs` | run 构建、样式意图采纳与覆盖记录 |
| `layout/footer.rs` | 奇偶页页码 |
| `layout/cover.rs` | 文件汇编封面（空行、标题、日期） |
| `layout/toc.rs` | 目录页与 Word TOC 域 |
| `numbering.rs` | 列表静态序数、标题编号状态与静态回退（FR-IR-5） |
| `pandoc.rs` | 由样式数据派生 pandoc `reference.docx`（样式 ID 映射为内置英文名） |
| `report.rs` | 后端报告的消息、代码与严重级别 |

边界：数据管“值”，流程代码管“怎么拼”，`charta-docx-kit` 管“机械操作”，引擎管“调度、schema 校验与报告结构”。同一规范族内的轻量模板可共用一个后端 crate；文种结构差异大时再新建 crate，复用核心、工具箱和样式加载模式。

### 6.8 通用材料模板（general）

`general` 与通知共用同一个后端 crate 和 `layout/` 流程，通过 `DocTemplate::General` 选择。CLI 默认后端是 `general`；库层 `DocOptions::default()` 与 `Gb9704Backend` 仍以 `Notice` 为准，需要完整通知时由调用方或 CLI 显式选择。

- 无版头、无版记；页面设置、页码、列表、表格、图片与题注流程复用。
- 元数据：`author`、`date`、`recipient` 均可选，两个署名项都不填时不生成署名区。大标题取自正文第一个一级标题（`#`），元数据不再提供 `title`；若仍提供会报告 `metadata_ignored`。docx 的 `dc:title` 同样取该一级标题。
- 标题层级与通知一致：`#` 用大标题样式（方正小标宋_GBK 二号），`##` 用一级标题样式，`###` 用二级标题样式，`####` 及更深用三级标题样式。自动编号相应下移：`#` 不编号，`##`–`#####` 对应 `一、` / `（一）` / `1.` / `（1）`，`######` 及更深报告 `heading_numbering_flattened`。
- 每个 `#` 开启新的标题编号区段：后端为第一个区段使用样式数据的编号定义，之后每个 `#` 分配新的抽象编号/编号实例 ID（内容沿用同一份定义），使其后 `##` 从 `一、` 重新开始；静态文本回退同样在每个 `#` 处清零各级计数。第二个及以后的 `#` 仍只按大标题渲染，不重复署名区。区段内部的下级重置仍由 `numbering.heading.restart` 控制。
- 各模板的标题都写入 `w:outlineLvl`（`#` 为 0，逐级递增），Word 导航窗格可以直接折叠跳转；引用块里的标题不写大纲级别。
- 署名区紧跟在第一个一级标题之后、正文之前，分两行居中：第一行姓名，第二行 `（YYYY年M月D日）`。姓名与日期都可省略：只提供一项时只生成对应一行，两项都未提供时不生成署名区。姓名恰好两个字符时中间插入一个全角空格；三字及以上原样。GB/T 9704 没有直接规定姓名字距，这是常见排版惯例，单位模板优先。
- `recipient` 位于署名区之后、正文之前，左对齐，后端补全角冒号。
- 样式数据：`data/general-styles.json`。大标题方正小标宋_GBK 二号；一级标题（`##`）方正黑体_GBK 三号；二级标题（`###`）方正楷体_GBK 三号加粗；三级及以下（`####` 等）方正仿宋_GBK 三号不加粗；正文方正仿宋_GBK 三号。
- 行内强调：`strong` 使用 `GbStrong`（方正楷体_GBK 加粗），`emphasis` 使用 `GbEmphasis`（方正楷体_GBK 不加粗、不生成斜体）；通知模板保持原有行为。两套样式 ID 都来自样式数据，不在流程中内联字体名。
- 样式外部化与局部微调：`GeneralBackend::with_style_file(path)` 按 `StyleSet::General` 加载完整文件或补丁；`--styles` 补丁可以只改 `GbBody` 的字号、`GbStrong` 的字体等单项。例如 `GbBody.run.size_step: 1` 把正文 16pt 提到 18pt，标题和表格保持原值；行内 `strong` 的字号跟随所在段落，字体和加粗取自 `GbStrong`。

`DocOptions.template` 只影响行内强调与模板选择；通知的默认值不变，输出与加入通用材料前逐字节一致。

### 6.9 文件汇编模板（compilation）

`compilation` 与通用材料共用 `StyleSet::General` 样式集和正文流程，通过 `DocTemplate::Compilation` 选择，额外生成封面和目录页。schema 要求 `title`、`date`，`date_style` 可选（`arabic` 默认，`chinese` 为汉字日期）。

- 封面：5 个 `CoverSpacer` 空段 → `CoverTitle`（方正小标宋_GBK 二号，居中）→ 7 个空段 → `CoverDate`（方正楷体_GBK 二号，居中），三者行距均为 1.5 倍。`date_style: chinese` 输出 `二〇二六年十月二十五日`。
- 目录：紧接封面的单独一页，`TocTitle`（“目　　录”，中间 2 个全角空格，方正小标宋_GBK 二号）+ `TOCSpacer`（三号字空行）+ Word TOC 域 `TOC \o "1-1" \h \z \u`；条目套 `TOC1`（方正楷体_GBK 三号、无缩进、单倍行距、段前段后 6 磅、156 mm 右对齐点线制表位），写入 `w:updateFields`。
- 分节与页码：封面与目录合成一节，节属性不带页脚引用，不显示页码；正文从下一节开始显示页码，`w:pgNumType w:start="1"` 让正文从 1 起编。每个 `#` 篇章之间用普通分页符（`w:br w:type="page"`）隔开，正文保持同一节，页码连续。页脚沿用 `footer` 配置的奇偶页区分：奇数页居右、偶数页居左，写入 `w:evenAndOddHeaders`。
- 正文：`#` 为篇章标题，使用 `GbTitle` 样式但不编号；`##` 起按通用模板四级编号（`一、` / `（一）` / `1.` / `（1）`），每个 `#` 重开编号区段；标题写 `outlineLvl`。不生成署名区、主送机关和附件说明，`author` / `recipient` 若提供会报告 `metadata_ignored`。
- 封面、目录与正文样式都在 `general-styles.json`；`CompilationBackend` 提供 `try_new` / `try_with_styles` / `with_styles` / `with_style_file`，补丁与通用模板一致。

### 6.10 pandoc reference.docx

`DocStyles::pandoc_reference_docx()` 用样式数据生成 pandoc 的 `reference.docx`；CLI 入口为 `charta reference`。pandoc 按 Word 内置样式 ID 取样式，而 Charta 的 `Gb*` ID 带规范语义，因此生成时只改 ID 与显示名，格式值全部沿用样式数据：

- 映射：`GbTitle`→`Title`、`GbByline`→`Author`、`GbDate`→`Date`、`GbBody`→`BodyText`、`GbHeading1`–`3`→`Heading1`–`3`、`GbQuote`→`BlockText`、`GbTableCaption`→`TableCaption`、`GbImageCaption`→`ImageCaption`、`TocTitle`→`TOCHeading`。
- 派生：`BodyText` 复制出 `FirstParagraph` 与 `Compact`（去掉首行缩进），`GbByline`（通用材料）复制出 `Date`，`Heading3` 复制出 `Heading4`–`Heading9`，避免 pandoc 的首段、紧凑列表、元数据日期和四级以下标题无样式可用。
- 无对应角色的样式（封面、`TOC1`、行内强调、页码等）保留原 ID，可经 `custom-style` 使用；源样式被改名后不再存在。
- 生成物只含样式与页面设置，不含编号定义；pandoc 会以它为基础包生成正文。`--styles` 补丁先生效再映射，`--backend` 决定使用通知或通用材料样式集。

## 7. docx 工具箱

`charta-docx-kit` 只封装与规范无关的机械操作：

- 页面设置与分节
- 样式定义与应用
- 段落/run 属性、字体、字号、颜色
- 表格与单元格（含 `gridSpan`/`vMerge` 网格合并）
- 图片嵌入（PNG/JPEG/GIF 尺寸识别、关系、media 部件、drawing XML）
- 节垂直对齐（`vAlign`）
- 编号定义
- PAGE 域与其他通用域（`fldChar`/`instrText`）
- 制表位（含点线前导）
- 段落大纲级别、分页符、打开时更新域
- 奇偶页页脚
- 段落下边框（红线、版记线）

不包含：

- 版记位置逻辑
- 发文字号排版规则
- 版头结构
- 任何国标语义判断

API 以公开数据模型为主，后端可完全绕开某个 helper 直接构造 XML 或修改模型。它不依赖 `charta`，可独立使用。

打包方式：直接生成 `[Content_Types].xml`、关系文件、`document.xml`、`styles.xml`、`numbering.xml`、`settings.xml`、页脚、`docProps`，用 zip 写入内存。测试会解压检查关键结构；生成结果另经 Word COM → PDF 人工核对。

## 8. 报告系统

### 8.1 JSON 结构

```json
{
  "report_version": "1.0",
  "engine_version": "0.2.0",
  "source": "examples/degraded.typ",
  "frontend": "typst",
  "backend": "gb9704",
  "summary": {
    "blocking": 0,
    "warnings": 11,
    "total": 11,
    "by_kind": {
      "non_deterministic": 6,
      "render_warning": 3,
      "style_overridden": 2
    }
  },
  "issues": [
    {
      "severity": "warning",
      "message": "`#for` 不可静态确定",
      "type": "non_deterministic",
      "location": {
        "file": "examples/degraded.typ",
        "line": 11,
        "column": 1,
        "end_line": 11,
        "end_column": 35,
        "byte_start": 260,
        "byte_end": 294
      },
      "reason": "循环生成的内容不可静态确定",
      "impact": "循环体按字面量提取一次；实际重复次数与变量替换未知"
    }
  ]
}
```

`IssueDetails` 使用内部标签 `type` 扁平化到 issue 对象。新增条目类型或字段为兼容变更；已有字段语义不变。

### 8.2 排序与摘要

`Report::finish()` 按“阻断优先 → 文件/行/列 → 消息”稳定排序，并重算摘要。这样同一输入的报告可逐字节比较。

### 8.3 严重性

- `Blocking`：schema 必填/类型/枚举错误、Markdown front matter / Typst `charta-meta` 解析失败、include 失败/循环/过深；`--strict` 下的契约外语法。
- `Warning`：样式覆盖、不可静态确定、契约外语法（默认）、渲染告警。

后端在渲染完成后返回的 blocking 条目不会回滚已生成的 docx；CLI 会写 docx 并以退出码 2 结束，由调用方决定是否采用。

## 9. 管线语义

```
parse → 角色声明/校验 → strict 升级 → schema 校验 → [阻断? 跳过渲染]
      → backend.render → 合并报告 → 排序/摘要
```

- `ConversionOutput.document` 始终返回 IR，即使未渲染。
- `docx` 在阻断性错误时为空；`report` 始终返回。
- CLI：0 成功；1 致命错误；2 报告含阻断项。
- 新增后端只需实现 `Backend` 并在 CLI 注册；核心 `charta` 零改动。

## 10. 版本与兼容

- workspace 版本单一来源，所有 crate 锁步；IR/报告格式版本独立。
- 0.x 阶段允许次版本内调整内部 API；1.0 后遵循 SemVer。
- 报告 JSON 消费方应忽略未知字段；引擎不读取报告。
- 引擎接口与 IR 中不存在模板相关字段；模板策略完全归后端。

详见 [VERSIONING.md](VERSIONING.md)。

## 11. 需求 §9 待定项的落地决定

| # | 事项 | 0.1.0 决定 | 理由/后续 |
|---:|---|---|---|
| 1 | Markdown 契约外语法 | 默认降级 + 告警；`--strict` 阻断 | 保留可用性，同时允许严格流水线；后续可配置逐类策略 |
| 2 | Typst `#set text` 轻量作用域 | 第一期不做，报告“不可静态确定” | 作用域模拟属于求值，列入二期 |
| 3 | 扩展角色注册 | 运行期 `RoleRegistry`；文档用 `_charta.roles` 声明；后端 `roles()` 注册 | 避免编译期耦合，便于后端独立扩展 |
| 4 | docx-kit 与核心兼容 | 0.1.x 锁步；kit 可独立使用 | 第一期接口仍在快速演进，锁步降低组合风险 |
| 5 | 报告消费方 | 同时面向人工与程序化 | JSON 1.0 稳定，Markdown 供审阅 |
| 6 | CLI 输入输出 | 单文档；默认同名输出；可选 JSON/Markdown 报告；退出码 0/1/2 | 目录批处理列入三期 |
| 7 | 后续模板策略 | 引擎不感知；由各后端自行决定 | GB 后端第一期从零生成，验证零模板路径 |

## 12. 验收标准对应

| 验收标准 | 实现与证据 |
|---|---|
| 1. Markdown 通知 → 可核对 docx | `examples/notice.md`；GB 后端测试；Word COM 转 PDF 人工核对（页边距、字体、行距、页码、版记） |
| 2. 循环/条件降级可定位 | `examples/degraded.typ`；Typst 单元测试；CLI 测试断言 `location.file/line`、reason、impact |
| 3. 意图 → 覆盖决策 → 报告 | 默认采纳颜色；`degraded.typ` 颜色生效，字体/字号进入覆盖清单；后端测试覆盖两种配置 |
| 4. 切换后端零改核心 | 核心只依赖 trait；`charta` 管线测试使用假后端；GB 后端仅通过 trait 接入 |
| 5. 工具以库方式调用 | `charta::convert` 等公开 API；CLI 无核心逻辑；docx-kit 不依赖核心 |
| 6. 从零生成、无模板字段 | docx-kit 生成完整 zip 包；GB 后端不读模板；IR/Backend trait 无模板字段 |

## 13. 测试策略

- **单元测试**：IR 序列化与颜色规范化；schema 类型/枚举/日期；报告排序与 Markdown；Markdown 块/内联/front matter；Typst 注释、span、include、降级、`charta-meta` 字面量、`#text` 参数、嵌套列表、图片/figure 题注、受限表格；docx XML 片段与包结构；GB 后端版式与意图采纳。
- **集成测试**：CLI 实际执行，检查 docx zip 魔数、报告 JSON、IR JSON、退出码、严格模式、schema 命令。
- **人工验证**：生成 `notice.docx`、`typst-notice.docx`、`degraded.docx`，经 Word COM 导出 PDF，再渲染为 PNG 检查版头、标题、正文、表格、署名、版记、页码；Typst 富内容样本另核对列表层级、表头与图片题注。
- **回归重点**：Typst span 定位、include 循环、报告排序稳定性、PAGE 域、奇偶页脚、样式意图覆盖报告。

## 14. 风险与后续

| 风险 | 现状 | 后续 |
|---|---|---|
| IR 过早定型 | 1.x 只做带默认值的兼容增项 | 破坏性改动必须升主版本并做前后端双向评审 |
| Typst 降级粒度 | 已到文件+行列+原因+影响 | 增加更多真实稿回归；二期做 `#set` 作用域 |
| Typst 富内容子集 | 嵌套列表、图片/figure 题注、受限表格已进入 IR；动态内容显式报告 | 二期扩展表格模板与 figure 内容，或引入受控求值 |
| 样式意图死字段 | 颜色已采纳，覆盖报告完整 | 二期做属性级采纳策略 |
| 工具箱隐式约束 | API 公开且可绕开 | 第二后端作为真实消费者验证 |
| 版记位置 | 已通过末节底端对齐锚定，固定另起一页；可用 `--no-record` 关闭 | 同页沉底待后续版面测量优化 |
| 图片 | 已嵌入 PNG/JPEG/GIF，按版心等比缩放并居中 | 后续支持更多格式与 DPI 元数据 |
| 图片题注 | `caption` 已进入 IR，GB 后端生成 `图 N` 静态编号并报告一次 | Word 内增删图片不会自动重排；后续可做 SEQ 域或分章编号 |
| 标题编号 | 已改为 Word 多级列表，增删标题自动重排；纯文本抽取不含编号，Word/WPS 渲染可能有差异 | 需要静态文本或逐标题语法时再补；列表仍保持静态 |
| 通用材料字体 | 方正 GBK 字体名写入 docx，目标机未安装时由 Word 替换 | 交付前确认目标机器字体；两字姓名空格为惯例而非 9704 明文 |
| 批量处理 | CLI 单文件 | 三期目录级转换与报告汇总 |

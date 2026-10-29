---
name: charta
description: 使用 Charta CLI 把 Markdown/Typst 转成中文公文 docx 时使用。涵盖命令与参数、三个后端的选择与元数据写法、标题/列表/表格/图片约定、报告与退出码、交付前检查。
---

# Charta 使用技能

Charta 是一个命令行转换器：读 Markdown 或 Typst，按选定模板生成 `.docx`。它不做交互式编辑，所有版式由后端决定。

## 工作方式

1. 先确定体裁：通知、通用材料还是文件汇编，对应 `gb9704` / `general` / `compilation`。
2. 检查源文件元数据是否满足该后端必填项（`charta schema --backend <ID>` 可查）。
3. 运行 `charta convert`，读标准输出和可选报告。
4. 有阻断问题先改源文件；告警按需处理，不要忽略会影响版式的告警。
5. 交付时给出 docx 路径、后端和“阻断/告警”计数。

## 命令总览

```
charta convert <INPUT> [OPTIONS]      # 转换单个 .md / .typ
charta reference [OPTIONS]            # 生成 pandoc 用的 reference.docx
charta backends                       # 列出后端
charta schema --backend <ID> [--json] # 查看元数据字段
charta --version
```

Windows 下可执行文件是 `charta.exe`，参数相同。

### convert

| 参数 | 说明 |
|---|---|
| `<INPUT>` | 源文件；`.md`/`.markdown` 走 Markdown 前端，`.typ` 走 Typst |
| `--frontend auto\|md\|typst` | 默认 `auto`，按扩展名判断 |
| `--backend general\|compilation\|gb9704` | 默认 `general` |
| `-o, --output <PATH>` | docx 路径；缺省为源文件同名 `.docx` |
| `--report <PATH>` | 写 JSON 报告；不传不写文件 |
| `--report-md <PATH>` | 写 Markdown 报告；不传不写文件 |
| `--emit-ir <PATH>` | 额外写出 IR JSON，排查解析问题时用 |
| `--strict` | 契约外语法从告警升级为阻断 |
| `--styles <PATH>` | 外部样式文件或补丁；一般交付不需要 |
| `--no-header` | 通知不生成版头；`doc_number` 随之不必填（仅 gb9704） |
| `--no-record` | 通知不生成版记（仅 gb9704） |
| `--adopt-color <true\|false>` | 是否采纳源文件颜色意图，默认 `true` |
| `--adopt-font-family/-size/-bold/-italic` | 默认 `false`；源文件的字体/字号/加粗/斜体只是建议，默认不采纳 |

退出码：`0` 成功；`1` 参数、I/O 或致命错误；`2` 有阻断问题，docx 未生成。

### reference

| 参数 | 说明 |
|---|---|
| `--backend <ID>` | 样式来源：`general`（默认）、`compilation`、`gb9704` |
| `-o, --output <PATH>` | 输出路径，默认当前目录 `pandoc-reference.docx` |
| `--styles <PATH>` | 外部样式文件或补丁 |

用途：`pandoc 材料.md --reference-doc=pandoc-reference.docx -o 材料.docx`。注意 pandoc 的 `#` 对应 `Heading 1`，与 Charta 通用模板“`#` 是大标题”的口径不同。

## 选后端与元数据

| 体裁 | 后端 | 必填元数据 | 说明 |
|---|---|---|---|
| 正式通知 | `gb9704` | `doc_number`、`issuer`、`date` | 首个 `#` 即大标题；版头、版记、28 磅行距、页码；`organization`/`recipient`/`cc`/`attachments` 等可选；填 `signer` 改按上行文排版（发文字号居左空一字，签发人居右空一字） |
| 一般材料 | `general` | 无 | 首个 `#` 即大标题；`author`、`date`、`recipient` 都可选 |
| 多篇汇编 | `compilation` | `title`、`date` | 封面 + 目录 + 正文；`date_style: chinese` 可把封面日期写成汉字 |

通知的 `--no-header` 会取消版头，`doc_number` 变为非必填；`--no-record` 取消版记。关闭后仍提供对应专属元数据会得到 `metadata_ignored` 告警。

上行文（请示、报告）在元数据里加 `signer: 姓名`：发文字号自动改为居左空一字，右侧同一行排“签发人：姓名”（姓名用 3 号楷体）。不填 `signer` 就是下行文居中式版头；签发人目前只支持一人。

## 标题层级

| 源文件写法 | `gb9704` | `general` | `compilation` |
|---|---|---|---|
| `#` | 大标题（不编号） | 大标题（不编号） | 篇章标题（不编号，另起一页） |
| `##` | 一、 | 一、 | 一、 |
| `###` | （一） | （一） | （一） |
| `####` | 1. | 1. | 1. |
| `#####` | （1） | （1） | （1） |
| `######` | 超过四级，告警 | 超过四级，告警 | 超过四级，告警 |

- `gb9704` 和 `general` 都只把正文第一个 `#` 当大标题，不要再用元数据 `title`。
- `compilation` 的每个 `#` 是独立篇章，其后编号从 `一、` 重新开始。
- 自动编号：`gb9704` 和 `general` 默认关闭，`_charta.heading_numbering: true` 开启；`compilation` 默认开启，`false` 关闭。不开启时请自己写“一、”等序号。

## 输入能力与限制

Markdown：

- 支持标题、段落、`-`/`1.` 嵌套列表、GFM 管道表格、独立图片行、`>` 引用、`**加粗**`、`*斜体*`。
- 列表序号由后端生成：四级依次 `（1）`、`①`、`A.`、`a.`，第五级起按正文并告警。
- 图片写在独立一行，路径相对**源文件所在目录**；Markdown title 会作为题注（“图 N　…”）。支持 PNG/JPEG/GIF。
- 不支持链接、脚注、行内代码、围栏代码块、HTML、分隔线、任务列表等；默认降级为正文并产生 `unsupported_syntax` 告警，`--strict` 时变为阻断。
- 表格题注在 Markdown 里没有语法入口；需要题注请用 Typst 的 `#figure(table(...), caption: [...])`。

Typst：

- 元数据写在主文件第一个非空行：`#let charta-meta = (issuer: "昆山市人民政府", date: "2026-10-25")`。标题在正文用 `= 公文标题` 给。`#let` 不产生输出，源文件仍可被 Typst 直接编译。
- `=`/`==` 等标题与 Markdown 的 `#` 层级对应；字面量 `include` 会展开，`#image(...)`、`#figure(...)`、受限 `#table(...)` 可用。
- 循环、条件、`#set`、`#show`、`#import`、动态表格参数等不可静态确定的内容会降级并告警。

## 典型用法

```bash
# 正式通知（带审计报告）
charta convert 通知.md --backend gb9704 -o 通知.docx \
  --report 通知.report.json --report-md 通知.report.md

# 只留主体、不要版头版记
charta convert 通知.md --backend gb9704 --no-header --no-record

# 一般材料
charta convert 材料.md -o 材料.docx

# 文件汇编（封面 + 目录 + 多篇材料）
charta convert 汇编.md --backend compilation -o 汇编.docx

# Typst 源文件，自动按扩展名识别
charta convert 材料.typ -o 材料.docx

# 严格模式：把契约外语法当作阻断
charta convert 材料.md --strict --report 材料.report.json

# pandoc 参考样式
charta reference --backend general -o pandoc-reference.docx
pandoc 材料.md --reference-doc=pandoc-reference.docx -o 材料.docx

# 查看某后端需要哪些元数据
charta schema --backend compilation
```

## 交付前检查

- **退出码**：`2` 表示有阻断问题且 docx 未生成；先看标准输出的前几条阻断问题，或读 `--report` 生成的 JSON 的 `summary` / `issues`。
- **告警**：`metadata_ignored`、`unsupported_syntax`、`image_read_failed`、`heading_numbering_flattened` 等不一定阻断，但会影响内容完整性，交付时要说明。
- **目录页码**：`compilation` 的目录是 Word TOC 域，Word 打开时提示更新域；WPS 或未更新时目录为空，需要手动“更新域”。
- **字体**：文档写入方正 GBK 等字体名，目标机器未安装时 Word/WPS 会替换显示，版式可能变化。
- **页码**：汇编的封面和目录不显示页码，正文从第 1 页起；奇数页页码居右、偶数页居左。

## 注意事项

- `--report` / `--report-md` 是 opt-in，不传就不会生成报告文件；默认输出路径相对当前工作目录。
- `--styles` 只用于确有样式定制需求的场景；补丁只能改已有样式，写错样式 ID 会直接报错。
- 源文件路径、图片路径、`--output` 等相对路径都按“执行命令时的当前目录”解析；图片和 Typst include 则相对源文件目录。
- 通知的 `#` 是一级标题，通用/汇编的 `#` 是大标题或篇章标题；把 Markdown 从一种体裁换到另一种时，标题层级通常要整体调整。
- 通知的红头（发文机关标志）固定 36 pt 字高；机关名称超过版心宽度时按比例压窄字宽，保持单行不折行。
- 通知的落款和日期右空是正文里的行尾普通空格（16 pt 下 4 个 ≈ 1 字），段落带 `wordWrap=0`；要改位置就在 Word 里增删这些空格，别用全角空格。

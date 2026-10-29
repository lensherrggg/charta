# Changelog

本项目遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 的记录方式，版本号遵循语义化版本。

## [Unreleased]

### Changed

- 通知后端的大标题改为取正文第一个一级标题（`#`），不再读取元数据 `title`（仍提供会报告 `metadata_ignored`）；标题层级整体与通用/汇编对齐，`##` 起才是一级标题与编号起点，通知标题也写入 `outlineLvl`。

## [0.2.0] - 2026-10-29

### Changed

- 版记末节改为偶数页起始（`w:type="evenPage"`），需要时由 Word 自动补一页空白；末节使用独立空页脚，空白页和版记页都不再显示页码，符合 GB/T 9704-2012 7.5 的口径。
- 通知的署名与日期右空改为行尾普通空格：按样式 `right_chars`（署名 4 字、日期 2 字）换算，16 pt 下每字 4 个空格，段右缩进清零并写 `wordWrap=0`（Word/WPS 默认忽略右对齐段的行尾空格，关闭后才计入）。生成后可直接在 Word 里增删空格调整位置，不再依赖段右缩进。
- 后端 crate 由 `charta-backend-gb9704` 更名为 `charta-backend-cn`：它同时承载通知、通用材料和文件汇编，不再以单一国标命名；共享类型 `Gb9704Styles` / `Gb9704Options` / `Gb9704Template` 改为 `DocStyles` / `DocOptions` / `DocTemplate`，`Gb9704Backend` 仍专指通知后端。
- CLI 的 `--report` / `--report-md` 改为可选：只有显式给出路径时才写出 JSON / Markdown 报告，缺省只生成 docx；同时不再在标准输出中打印未生成的报告路径。
- Typst 元数据入口从 YAML front matter 改为主文件顶部的 `#let charta-meta = (...)` 字典字面量；`#let` 在 Typst 编译时没有输出，源文件可直接由 Typst 编译。旧写法会报告 `unsupported_syntax` 并跳过，提示迁移。
- CLI 默认后端改为 `general`（通用材料）；需要 GB/T 9704 通知时显式传 `--backend gb9704`，`schema` 命令的默认后端同步改为 `general`。
- `general` 通用材料模板改为以正文第一个一级标题作为大标题：不再读取元数据 `title`（仍提供会报告 `metadata_ignored`），`docProps` 的 `dc:title` 同步取该标题。`#` 使用方正小标宋大标题样式，`##`–`####` 依次对应黑体一级、楷体加粗二级、仿宋三级标题，自动编号相应从 `##` 起步。
- `general` 的 `author`、`date` 改为可选：只填一项时只生成对应署名行，两项都不填时不生成署名区，大标题后直接接主送机关或正文。
- `general` 中每个 `#` 开启新的标题编号区段：第一个区段复用样式数据的编号定义，之后每个 `#` 分配新的抽象编号/编号实例 ID，使其后 `##` 从 `一、` 重新开始；静态文本回退同样在每个 `#` 处清零计数。单个 `#` 的文档输出不变。
- GB/T 9704 列表改为四级静态文本序数：第一级 `（1）`、第二级 `①`、第三级 `A.`、第四级 `a.`，第五级起按正文段落渲染；不再生成 Word 项目符号或自动编号。
- GB/T 9704 标题三级编号 `1.` 与标题文字之间补一个空格（`w:suff=space`），其余级仍不加后缀空格。
- GB/T 9704 版记改为末节底端对齐，整块置于最后一页版心底部，不再被分页拆开。
- GB/T 9704 表格支持 `colspan`/`rowspan` 网格合并，不再报告 `table_merge_unsupported`。
- GB/T 9704 后端样式值外置到 `crates/charta-backend-gb9704/data/styles.json`；渲染流程不再内联字体、字号、颜色、页边距、行距、缩进、边框、表格列宽等样式字面量，静态列表序数按例外保留在流程逻辑中。
- GB/T 9704 后端按职责拆分为 `schema`、`styles`（含 `data/`）、`layout/{header,body,record,text,footer}`、`numbering`、`report` 与 `lib` 入口；对外 API 与输出保持不变。
- IR 格式升至 `1.1`：标题节点新增可选 `auto_number` 建议字段，`_charta.heading_numbering` 作为文档级开关。默认不自动编号，避免与手写序号重复。
- GB/T 9704 标题在显式请求时改用 Word 内置多级列表生成 `一、` / `（一）` / `1.` / `（1）`，编号定义写入 `numbering.xml`，增删标题会自动重排；首次报告 `heading_numbering_word`，超过四级报告 `heading_numbering_flattened`。样式数据缺少 `numbering.heading` 时回退静态文本并报告 `heading_numbering_static`。列表编号保持静态文本。
- GB/T 9704 后端嵌入 PNG/JPEG/GIF 图片，按版心等比缩放并居中；读取或解码失败报告 `image_read_failed` / `image_decode_failed` 并跳过。
- 一级至三级标题样式补充 `keepNext`，减少标题孤行。
- 从通知 schema 移除 `signer`：通知为下行文，不渲染签发人；若提供则报告 `metadata_ignored`。上行文口径留到请示/报告后端实现。

### Added

- `charta-docx-kit` 的 `Indent` 新增 `right_twips`，与 `right_chars` 一起输出 `w:right` / `w:rightChars`；`ParagraphFormatting` 新增 `word_wrap`，输出 `w:wordWrap`；`Docx` 新增 `final_section_footer` / `final_section_even_page`，可让末节从偶数页开始并使用空页脚。Word 清零字符单位缩进、或让右对齐段的行尾空格参与排版时，分别需要这两组属性。
- 通知的红头（发文机关标志）固定 36 pt 字高，机关名称超出版心宽度时按比例做字符缩放（`w:w`），保持单行不折行；`charta-docx-kit` 的 `RunProperties` 新增 `character_scale`。
- 新增 `skills/charta/SKILL.md`：给 AI 助手的使用技能，涵盖 CLI 命令、后端选择、元数据与输入约定、交付检查。
- 新增 `charta reference` 子命令与 `DocStyles::pandoc_reference_docx()`：把样式数据映射为 pandoc 内置样式 ID（`Title`、`Author`、`Date`、`BodyText`、`Heading1`–`3`、`BlockText`、`TableCaption`、`ImageCaption`、`TOCHeading`）并生成 `reference.docx`；另从正文、署名和三级标题派生 `FirstParagraph`、`Compact`、`Date`、`Heading4`–`9`。无对应角色的样式保留原 ID，`--styles` 补丁先生效再映射。
- 列表嵌套超过四级时产生 `list_nesting_flattened` 渲染告警。
- 有序列表的 `start` 现在用于起始序号，不再忽略。
- `charta-docx-kit` 支持 `gridSpan`/`vMerge` 和节垂直对齐；表格合并超限时报告 `table_rowspan_truncated` / `table_cells_overflow`。
- 设计文档补充 FR-BE-5/6：后端样式值须可外置为数据文件，格式由后端决定；GB/T 9704 后端已在 0.1.0 落地。
- `Gb9704Styles` 支持 `embedded` / `from_json` / `from_path` 加载；`Gb9704Backend` 支持 `try_new`、`try_with_options`、`with_styles`、`with_style_file`，样式数据缺失、字段未知、枚举非法或引用未定义样式时直接报错，不做静默回退。
- `charta-docx-kit` 支持图片：格式与尺寸识别、`Image` 注册、drawing XML、图片关系、`word/media` 部件和内容类型。
- 样式数据新增 `GbImage` 样式与图片最大显示宽高配置。
- `Gb9704Options` 新增 `header` / `record` 开关及 `without_header` / `without_record` 构造器，CLI 新增 `--no-header` / `--no-record`；关闭版头后 `doc_number` 不再必填，关闭版记后取消末页沉底分节。
- 关闭版头或版记时，仍提供对应专属元数据会报告 `metadata_ignored`，不静默丢弃。
- `ImageBlock` 新增可选 `caption` 字段（IR 1.1 兼容增项）；Markdown 图片 title 自动转为题注，GB/T 9704 后端在图片下方生成 `图 N` 静态编号并报告一次 `image_caption_static`。
- 样式数据新增 `GbImageCaption`：楷体四号、居中、段前 3 pt；图片题注与图片保持同页。
- 新增 `general` 通用材料后端与 `data/general-styles.json`：无版头/版记，标题与各级标题使用方正 GBK 字体，二级标题（`（一）`）加粗，三级及以下（`1.`、`（1）` 等）不加粗；`author`/`date` 生成标题下居中署名区，姓名与日期分两行，两字姓名中间加全角空格，`recipient` 可选；`strong` 用方正楷体_GBK 加粗，`emphasis` 用方正楷体_GBK 不加粗。
- `Gb9704Options` 新增 `template` 与 `Gb9704Template`；通知模板默认行为不变。
- 样式加载新增 `StyleSet::Notice` / `StyleSet::General` 与 `embedded_for` / `from_json_for` / `from_path_for`；用错必需样式集直接报错。`GeneralBackend` 新增 `try_with_styles`、`with_styles`、`with_style_file`。
- CLI 的 `--styles <PATH>` 支持局部补丁：`styles` 按样式 ID 写成对象即可只改指定样式，未写的部分沿用内置值；完整样式文件（`styles` 为数组）仍可整份替换。
- 样式补丁支持 `run.size_step`，按中文字号阶梯只调整单个样式（如正文大一号），也可用 `run.size_pt` 指定精确字号；二者互斥，未知样式 ID、未知字段、越界字号直接报错。
- `Gb9704Styles` 新增 `from_external_for`（自动识别完整文件/补丁）与 `from_json_with_patch`（补丁应用到指定基础数据）。
- `charta-docx-kit` 的编号定义新增 `chineseCounting` 格式、`multiLevelType`、`suff`（tab/space/nothing）和首行缩进，可生成规范的标题多级列表。
- 样式数据新增 `numbering.heading`：抽象编号/编号实例 ID、多级类型、四级 `numFmt`/`lvlText`/起始值/缩进/后缀，按 FR-BE-5 外置，并支持通过 `--styles` 补丁覆盖。
- 样式数据新增 `numbering.heading.restart`：`enabled` 控制下级标题编号是否随上级变更重置（默认 `true`），`never_from`（2–4）指定从第几级起不再重置；转换时写入 `w:lvlRestart w:val="0"`，可通过 `--styles` 补丁调整。
- `charta-docx-kit` 的编号层级新增 `restart`，可输出 `w:lvlRestart`（`0` 表示永不重置）。
- 新增 `compilation`（文件汇编）后端：封面（空 5 行、标题、空 7 行、日期）+ 目录页 + 通用模板正文。封面标题为方正小标宋_GBK 二号，日期为方正楷体_GBK 二号并支持 `date_style: arabic|chinese`；目录为 Word TOC 域（当前只收 `#` 级），条目用方正楷体_GBK 三号、无缩进、单倍行距、段前段后 6 磅、点线右对齐页码；`#` 为不编号的篇章标题，`##` 起编号并在每个 `#` 重开区段。封面行距 1.5 倍，目录标题中间 2 个全角空格并在标题下空一行三号字；封面和目录单独成节、不显示页码，每个 `#` 篇章用普通分页符另起一页，正文页码从 1 起连续编号，奇数页居右、偶数页居左。
- 通用模板（`general`）不再生成目录页；目录能力归入 `compilation` 后端。
- `general` 与 `compilation` 标题写入 `w:outlineLvl`，Word 导航窗格可按标题层级折叠跳转。
- 样式数据新增 `line_spacing`（`single` / `1.5` / `double`）与段落制表位 `tabs`（左/中/右对齐，`none` / `dot` 前导符）。
- `charta-docx-kit` 新增通用域（`Run::field_begin` / `instruction` / `field_separate` / `field_end`）、制表位、分页符与 `Docx::update_fields`。
- `charta-docx-kit` 分节新增页脚开关与 `w:pgNumType w:start`：可按节隐藏页码，或让某一节从 1 重新编号。
- Typst 前端支持按缩进嵌套的 `-` / `+` 列表，映射为 `ListBlock` / `ListItem.children`，不再拍平为正文。
- Typst 前端支持整行 `#image("path")` 与 `#figure(image(...), caption: [...])`，生成 `ImageBlock` 与题注；非字面量路径显式报告。
- Typst 前端支持受限 `#table(...)`：`columns` 整数/元组、字面量单元格、`table.header`、`table.cell` 的 `colspan`/`rowspan`/`align`，以及 `#figure(table(...), caption: [...])`；动态参数与未采纳的版式参数逐项告警。
- Typst 前端新增 `charta-meta` 字典字面量解析：字符串、整数/浮点、布尔、`none`、列表与嵌套字典；`(x)` 分组、`(x,)` 单元素列表、`(:)` 空字典遵循 Typst 语义，动态值与带单位数值阻断。
- Typst 前端支持 `#strong[...]` / `#emph[...]` 行内别名，与 `*...*` / `_..._` 共用 `strong`/`emphasis` 语义并支持嵌套。

## [0.1.0] - 2026-10-25

第一期 MVP。

### Added

- `charta`：语义 IR、前端/后端 trait、后端 schema 校验、角色注册表、转换管线、报告系统。
- `charta-md`：受控 Markdown 子集解析（标题、段落、列表、表格、图片、强调、引用、YAML front matter），契约外语法显式报告。
- `charta-typst`：只读 CST 子集解析（标题、内联 `#text` 样式意图、字面量 `include` 递归展开），循环/条件/`import`/`#set`/`#show` 降级并携带 span。
- `charta-docx-kit`：从零生成 OOXML 包（页面、分节、样式、段落/run、表格、编号、PAGE 域、奇偶页页脚、分隔线），不依赖外部模板。
- `charta-backend-gb9704`：GB/T 9704-2012 通知后端（版头、主体、版记、字体字号、28 磅行距、页码），带样式意图采纳/覆盖报告。
- `charta-cli`：单文档转换、报告输出、IR 导出、schema 查询、严格模式与样式采纳开关。
- 示例：`examples/notice.md`、`examples/notice.typ`、`examples/degraded.typ`。
- 文档：README、`docs/DESIGN.md`、`docs/VERSIONING.md`；设计文档补充 5.3.1 编号语义（列表、标题、后端与工具箱的编号职责边界）。
- 版本脚本：`scripts/bump-version.sh`。

### Notes

- 引擎版本 `0.1.0`；IR 格式版本 `1.0`；报告格式版本 `1.0`。
- 第一期只提供 GB/T 9704 通知后端；会议纪要、请示、函与企业公文后端在后续阶段实现。
- GB/T 9704 后端支持嵌入 PNG/JPEG/GIF 图片；Typst 列表、数学公式、原始块按降级或普通文本处理。

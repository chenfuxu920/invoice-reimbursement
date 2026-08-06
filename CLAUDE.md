<!-- superpowers-zh:begin (do not edit between these markers) -->
# Superpowers-ZH 中文增强版

本项目已安装 superpowers-zh 技能框架（20 个 skills）。

## 核心规则

1. **收到任务时，先检查是否有匹配的 skill** — 哪怕只有 1% 的可能性也要检查
2. **设计先于编码** — 收到功能需求时，先用 brainstorming skill 做需求分析
3. **测试先于实现** — 写代码前先写测试（TDD）
4. **验证先于完成** — 声称完成前必须运行验证命令

## 可用 Skills

Skills 位于 `.opencode/skills/` 目录，每个 skill 有独立的 `SKILL.md` 文件。

- **brainstorming**: 在任何创造性工作之前必须使用此技能——创建功能、构建组件、添加功能或修改行为。在实现之前先探索用户意图、需求和设计。
- **chinese-code-review**: 中文 review 沟通参考——话术模板、分级标注（必须修复/建议修改/仅供参考）、国内团队常见反模式应对。仅在用户显式 /chinese-code-review 时调用，不要根据上下文自动触发。
- **chinese-commit-conventions**: 中文 commit 与 changelog 配置参考——Conventional Commits 中文适配、commitlint/husky/commitizen 中文模板、conventional-changelog 中文配置。仅在用户显式 /chinese-commit-conventions 时调用，不要根据上下文自动触发。
- **chinese-documentation**: 中文文档排版参考——中英文空格、全半角标点、术语保留、链接格式、中文文案排版指北约定。仅在用户显式 /chinese-documentation 时调用，不要根据上下文自动触发。
- **chinese-git-workflow**: 国内 Git 平台配置参考——Gitee、Coding.net、极狐 GitLab、CNB 的 SSH/HTTPS/凭据/CI 接入差异与镜像同步配置。仅在用户显式 /chinese-git-workflow 时调用，不要根据上下文自动触发。
- **dispatching-parallel-agents**: 当面对 2 个以上可以独立进行、无共享状态或顺序依赖的任务时使用
- **executing-plans**: 当你有一份书面实现计划需要在单独的会话中执行，并设有审查检查点时使用
- **finishing-a-development-branch**: 当实现完成、所有测试通过、需要决定如何集成工作时使用——通过提供合并、PR 或清理等结构化选项来引导开发工作的收尾
- **mcp-builder**: MCP 服务器构建方法论 — 系统化构建生产级 MCP 工具，让 AI 助手连接外部能力
- **receiving-code-review**: 收到代码审查反馈后、实施建议之前使用，尤其当反馈不明确或技术上有疑问时——需要技术严谨性和验证，而非敷衍附和或盲目执行
- **requesting-code-review**: 完成任务、实现重要功能或合并前使用，用于验证工作成果是否符合要求
- **subagent-driven-development**: 当在当前会话中执行包含独立任务的实现计划时使用
- **systematic-debugging**: 遇到任何 bug、测试失败或异常行为时使用，在提出修复方案之前执行
- **test-driven-development**: 在实现任何功能或修复 bug 时使用，在编写实现代码之前
- **using-git-worktrees**: 当需要开始与当前工作区隔离的功能开发或执行实现计划之前使用——创建具有智能目录选择和安全验证的隔离 git 工作树
- **using-superpowers**: 在开始任何对话时使用——确立如何查找和使用技能，要求在任何响应（包括澄清性问题）之前调用 Skill 工具
- **verification-before-completion**: 在宣称工作完成、已修复或测试通过之前使用，在提交或创建 PR 之前——必须运行验证命令并确认输出后才能声称成功；始终用证据支撑断言
- **workflow-runner**: 在 Claude Code / OpenClaw / Cursor 中直接运行 agency-orchestrator YAML 工作流——无需 API key，使用当前会话的 LLM 作为执行引擎。当用户提供 .yaml 工作流文件或要求多角色协作完成任务时触发。
- **writing-plans**: 当你有规格说明或需求用于多步骤任务时使用，在动手写代码之前
- **writing-skills**: 当创建新技能、编辑现有技能或在部署前验证技能是否有效时使用

## 如何使用

当任务匹配某个 skill 时，使用 `Skill` 工具加载对应 skill 并严格遵循其流程。绝不要用 Read 工具读取 SKILL.md 文件。

如果你认为哪怕只有 1% 的可能性某个 skill 适用于你正在做的事情，你必须调用该 skill 检查。
<!-- superpowers-zh:end -->

## 项目背景

发票报销自动化桌面工具（Tauri 2.x + Vue 3 + Rust/PaddleOCR）

### 关键架构

- **src-tauri/src/parser/invoice_parser.rs** — 发票解析（区域分割 + 正则提取 + 坐标回退销售方）
- **src-tauri/src/parser/itinerary_parser.rs** — 行程单解析（OCR坐标表格解析 + parangi交叉验证）
- **src-tauri/src/pdf/invoice_pipeline.rs** — 发票/行程单解析入口、配对逻辑
- **src-tauri/src/ocr/engine.rs** — PaddleOCR v5 封装（PDFium RGBA→RGB已修复）
- **src-tauri/src/pdf/text_extractor.rs** — pdfplumber 文字提取封装（含 `extract_raw_words_debug` 调试接口）
- **src-tauri/src/pdf/debug_extract.rs** — PDF 提取调试界面后端（三引擎坐标统一：pdfplumber/zpdf/OCR）

### pdfplumber 依赖（自建 fork，必要时可直接改 pdfplumber 源码）

**依赖声明** (`src-tauri/Cargo.toml`):
```toml
pdfplumber = { git = "https://github.com/chenfuxu920/pdfplumber-rs.git", branch = "cjk-safe-lenient", optional = true }
```

**Fork 仓库**: https://github.com/chenfuxu920/pdfplumber-rs · 分支 `cjk-safe-lenient`

**为什么不用上游 crates.io 0.2.0**:
- 上游 0.2.0 tokenizer 遇内容流中的 `<<` 硬失败（`unexpected '<<' in content stream`），中国发票 PDF 普遍触发
- 上游 PR#214 修了 `<<`（`tokenize_lenient`），但 PR#206/208 破坏了 CJK CID→Unicode 映射（Identity-H 字体 CID 当 Unicode → 中文乱码 77.55%）
- jacob-cotten fork 所有分支也含 PR#206/208，同样回归
- **无现成上游版本兼得 `<<` 修复 + CJK 正确**，故自建 fork

**Fork 分支构成** (10 commits, 最新在上):
```
f9444db fix(parse): ASCII 0x20-0x7E 半角宽度（CJK CID 字体 /W 缺 ASCII 条目时 fallback 0.5×/DW）
0729db5 feat: CharEvent/Char/Word 暴露 color/render_mode/text_object_index/font_flags/stem_v
24ffb68 fix(parse): decode predefined CJK CMap bytes via encoding_rs（火车票 GBK-EUC-H CID 乱码）
369244b fix(parse): /Contents 间接引用→数组三层结构解析（gp-template 全电发票/dzfp）
dea587e fix(annotations): Square/FreeText 注解合成 Rect
c2ab510 fix(table): bar-rect 单边 + 按行成格，修复发票单元格提取
9e59889 fix(shapes): fill 路径自动闭合 + 近似矩形边界框提取
4315b0b fix(words): >= 语义 word split tolerance (PR#243, jacob-cotten)
93c14cb feat: tokenize_lenient <<修复 (PR#214 cherry-pick, 适配 0.2.0 API)
0a436bf fix: char bbox and word grouping (0.2.0 base, CJK 0% mismatch)
```
（merge commit `0bdfaa4` 合并了 `cjk-safe-lenient-full` 进 canonical 分支，含上述所有修复）

**Predefined CJK CMap 字节解码修复** (24ffb68): 火车票/部分行程单 PDF 用 `GBK-EUC-H` 编码 Type0 CID 字体且无 `/ToUnicode`，旧代码 `show_string_cid` 把 GBK 双字节当 2-byte CID，`emit_char_events` 走 `char::from_u32(cid)` fallback → 0xB9FA 解为 U+B9FA = "뻺"（韩文 Hangul），全文 91+11 个韩文乱码。修复：`CachedFont` 加 `encoding_name: Option<String>` 字段（Type0 分支用 `get_type0_encoding(fd)` 填充），`handle_tj`/`handle_tj_array` 在 `is_predefined_cjk_cmap(encoding_name)` 为真时分派到新函数 `show_string_predefined_cjk`，用 `encoding_rs` 解码整段字节为 Unicode 字符串（GBK/BIG5/UTF_16BE/SHIFT_JIS/EUC_JP/EUC_KR），每个 RawChar 的 `char_code` 直接是 Unicode 码点，让既有 `char::from_u32` fallback 正确解析。Identity-H 路径**未改**（仍走 `show_string_cid` + ToUnicode）。`/W` 宽度查找仍按 Unicode 码点查（数组按 CID 索引，Unicode 码点对 CJK CID 通常 miss → fallback `/DW`；CJK 字符碰巧 `/DW=1000` = 全角 = 正确，ASCII 字符过宽由 `f9444db` 修复，见下）。位置：`crates/pdfplumber-parse/src/{interpreter,text_renderer,cid_font}.rs` + `crates/pdfplumber-parse/Cargo.toml` 加 `encoding_rs = "0.8"`。项目内 `tests/train_ticket_cid_debug_test.rs` 用真实火车票 PDF 作回归检查（韩文音节+兼容字母必须为 0，CJK 主区 ≥ 50）。

**ASCII 半角宽度修复** (f9444db): 24ffb68 后 `show_string_predefined_cjk` 把字节解码为 Unicode 字符，但 `/W` 是 CID 索引的（如 `[7716 7716 500]` 的 7716 = "中" 的 Adobe CID），用 Unicode 码点（如 "中"=0x4E2D=20013）查 `/W` 永远 miss → fallback `/DW`（PDF 规范默认 1000 = 全角 em）。CJK 字符 `/DW=1000` 碰巧等于全角宽度（视觉正确），但 ASCII 字符（0x20-0x7E）也 fallback 到 1000 → 比真实半角宽度（~500）**2× 过宽**。同一 PDF 用 pymupdf 渲染（读嵌入 TTF 的 hmtx 表）得出 ASCII = 0.5× 字号，是 WPS/浏览器的 ground truth。修复在 `crates/pdfplumber-parse/src/cid_font.rs` 的 `CidFontMetrics::get_width`：`/W` miss 时对 ASCII 范围（0x20-0x7E）返回 `default_width * 0.5`，CJK 范围仍用 `default_width`。zpdf 0.9 渲染同样有此问题（`zpdf-font/lib.rs:631` Type0 分支不回退 hmtx），未修。项目内 `tests/ascii_width_test.rs` 用真实火车票 PDF 作回归（G878/Changshanan/Wuhan 的 per_char 宽度必须在半角范围内）。

**`/Contents` 间接引用→数组修复** (369244b): `gp-template`（税务电子发票模板）把 `/Contents` 写成 `28 0 R`，但 obj 28 解析出来是 `[29 0 R]`（间接引用包数组）。ISO 32000-1 §7.8.2 允许 `/Contents` 为 stream 或 stream 数组，但旧代码 `Reference` 分支 resolve 后直接 `as_stream()`，遇 `Reference→Array` 形状报 `/Contents is not a stream: An object does not have the expected type`，导致全电发票/dzfp 系列 PDF 文字与单元格全空。修复在 `crates/pdfplumber-parse/src/lopdf_backend.rs` 的 `get_page_content_bytes`：抽出 `decode_contents_array` helper，`Reference` 分支 resolve 后按 Stream/Array 分派。项目内 `tests/pdf_contents_array_regression.rs` 用真实 PDF1/PDF2 作回归检查。

**发票单元格提取修复** (c2ab510 + 9e59889): 中国发票/行程单表格线是细填充矩形（~0.75pt bar，`m+l+l+l+f` 路径），不是描边线。三处协同修复：
- `shapes.rs`: fill 路径自动闭合 + 近似矩形用边界框 → bar 成为 Rect
- `edges.rs`: `edges_from_rect` 检测细条（min 维 ≤ 2.0pt）→ 沿长轴发 1 条边（旧 4 边产出 2 重复长边 + 2 桩被 `edge_min_length` 过滤，删掉了角点线段）
- `table.rs`: `intersections_to_cells` 按行收集 x（竖边同时穿过上下行界）→ 避免幽灵 x 坐标把宽单元格切成缺角相邻对

**不合入的上游 PR 及原因**:
- PR#206 (Adobe-GB1/CNS1/Korea1 CID→Unicode 表): subset 字体 CID 是 glyph ID 不是 Adobe CID，查表产生乱码
- PR#208 (Identity-H CID fallback): CID 当 Unicode `char::from_u32`，对中文 CID 字体错误（注：本 fork 的 24ffb68 commit 仅对**预定义 CJK CMap** 编码字节通过 encoding_rs 解码；Identity-H 仍走 ToUnicode→char::from_u32 路径，不应用 PR#208）
- PR#215/216 (Arabic CID / CJK vertical vmtx): 在 PR#208 之后，依赖其改动

**修改 pdfplumber 源码的流程**:
1. Clone fork: `git clone -b cjk-safe-lenient git@github.com:chenfuxu920/pdfplumber-rs.git`
2. 改代码（表格/边/形状在 `crates/pdfplumber-core/src/{table,edges,shapes}.rs`；tokenizer/解释器在 `crates/pdfplumber-parse/src/{tokenizer,interpreter}.rs`；`/Contents` 解析与后端在 `crates/pdfplumber-parse/src/lopdf_backend.rs`）
3. `cargo check -p pdfplumber` 确认编译
4. 用 path 依赖测试: `pdfplumber = { path = "<local>/pdfplumber-rs/crates/pdfplumber" }`
5. 跑 `cargo test --features pdfplumber --test pdfplumber_cjk_fidelity_test --test debug_extract_test --test pdfplumber_cell_debug_test`
6. Push 到 fork: `git push origin cjk-safe-lenient`
7. 项目里 `cargo update -p pdfplumber` 更新 Cargo.lock

**关键测试** (CJK fidelity 5/5 必须全过):
- `pdfplumber_cjk_fidelity_test`: VAT 发票 mismatch 必须 0.00%（验证无 CID 回归）
- `debug_extract_test`: 验证 `<<` tokenizer 修复 + 坐标缩放
- `pdfplumber_cell_debug_test`: 发票单元格提取诊断（`verify_find_tables_text_population` 断言 7 列行程单表；`diagnose_*` 打印逐阶段证据）
- `pdf_contents_array_regression`: 全电发票/dzfp `/Contents` 间接引用→数组回归（PDF1 64 words/439 chars，PDF2 72 words/454 chars）
- `train_ticket_cid_debug_test`: 火车票 GBK-EUC-H CID 解码回归（韩文音节+兼容字母必须为 0，CJK 主区 ≥ 50）
- `ascii_width_test`: 火车票 ASCII 半角宽度回归（G878/Changshanan/Wuhan 的 per_char 宽度必须 ≈ 0.5× 字号，验证 `f9444db` 修复）

### 近期重大改进 (2026-05)

**行程单解析重写** (`itinerary_parser.rs` ~1100行):
- 主行/续行分离：按序号Y坐标±30%行高分 group
- 锚点构建：序号锚点 + 时间锚点 + 间隔填充（gap >1.8×行高时补金额锚点）
- 里程/金额合并列分割：检测表头"里程[公里]金额[元]"合并块，金额只用 X>header_x 数据
- 三重交叉验证（parangi纯文本 → OCR坐标结果）：
  1. 金额修正（OCR误读12.0→parangi修正12.90）
  2. Provider补全（OCR缺失"滴滴轻享"→parangi续行合并）
  3. 时间恢复（OCR乱码"成都A428"→parangi恢复"04-25 08:48"，含续行分钟提取）

**发票解析修复** (`invoice_parser.rs`):
- `extract_seller_by_coords`: 坐标感知销售方提取（取最右侧"名称："块）
- `extract_amount`: total区域失败时回退全文搜索
- pipeline: parangi提取seller为空时回退OCR重解析

### 已知限制
- 滴滴page2表头合并块"序号车型上车时间城市"导致provider/time边界过宽
- parangi对中文发票多列布局会乱序
- 部分OCR乱码时间（如"A428"、"042708"）无法从OCR本身恢复，依赖parangi交叉验证
- **发票号码/开票日期不在表格内**（find_tables 检测不到表头区），只能全文正则提取（`build_invoice_from_cells`），不要试图用单元格提取——历史已验证同格/分格标签定位均失败

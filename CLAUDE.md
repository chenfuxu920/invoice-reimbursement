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
- test_invoice_parser_with_templates 测试偶发超时（Tera模板编译耗时）

# 项目指令

## 前端界面修改：必须先加载 impeccable skill

凡是涉及前端界面（视觉、布局、交互、样式、组件外观、动效等）的修改，在编辑任何界面文件之前，必须先通过 Skill 工具加载 `impeccable` skill（skill 名：`impeccable`），并按其指导完成设计决策后再动手改代码。

- 触发范围：`src/` 下的 `.vue`、`.ts`、`.css` 等界面文件、`index.html`、`public/` 静态资源，以及任何影响 UI 呈现的改动（包括新增页面/组件）。
- 例外：仅修改业务逻辑、数据层、测试，或 Tauri 后端（`src-tauri/`）等不涉及界面呈现的改动，无需加载。

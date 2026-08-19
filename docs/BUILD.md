# 构建说明

## 前置条件

- **Node.js** 18+
- **Rust** stable（≥ 1.77，推荐使用 rustup 安装）
- **系统依赖**：
  - **Linux**: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`
    ```bash
    sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
    ```
  - **Windows**: 需安装 Visual Studio Build Tools（MSVC 工具链）
  - **macOS**: 需安装 Xcode Command Line Tools

## 开发模式

```bash
npm install
npm run tauri dev
```

此命令会同时启动 Vite 开发服务器和 Tauri 窗口，支持前端热更新。

## 生产构建

项目提供三个 npm 脚本：

```bash
# 便携版（免安装，release 配置）
npm run tauri:build

# 便携版（快速配置，关闭 LTO 加快编译）
npm run tauri:build:fast

# NSIS 安装包（含自动更新签名与安装程序）
npm run tauri:build:installer
```

构建产物：

| 目标 | 产物路径 |
|------|---------|
| Windows 便携版 | `src-tauri/target/release/portable/`（或 `release-fast/portable/`） |
| Windows NSIS 安装包 | `src-tauri/target/release/bundle/nsis/` |
| macOS / Linux 安装包（CI 产物） | `src-tauri/target/release/bundle/` 下对应 `dmg/`、`deb/`、`appimage/` |

### 仅构建前端

```bash
npm run build
```

前端产物输出到 `dist/` 目录。

### 仅构建 Rust 端

```bash
cd src-tauri
cargo build --release
```

可执行文件输出到 `src-tauri/target/release/`。

## Release 优化配置

`Cargo.toml` 中已配置 release profile 优化：

```toml
[profile.release]
opt-level = "z"       # 优化体积
lto = "fat"           # 链接时优化（fat LTO）
strip = true          # 剥离调试符号
codegen-units = 1     # 单编译单元，更好的优化

[profile.release-fast]
inherits = "release"
lto = false
codegen-units = 16
```

## 打包配置

Tauri 打包配置位于 `src-tauri/tauri.conf.json`，主要配置项：

- **identifier**: `com.invoice-reimbursement.app`
- **productName**: `InvoiceAssistant`
- **version**: `1.1.0`（与 `package.json`、`Cargo.toml` 同步）
- **窗口**: 1024×768（最小 800×600），居中显示，可调整大小
- **NSIS 安装程序**（Windows）: 支持简体中文/英文，both 安装模式（可选当前用户/所有用户）
- **DEB 包**（Linux）: 声明 webkit2gtk 和 GTK3 依赖
- **自动更新**: tauri-plugin-updater + minisign，GitHub Releases 分发

## 常见问题

### Linux 构建缺少依赖

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

### Windows 构建缺少 MSVC 工具链

安装 [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)，选择 "Desktop development with C++" 工作负载。

### macOS 构建缺少 Xcode 工具

```bash
xcode-select --install
```

### cargo build --release 太慢

release 构建由于启用了 fat LTO，首次构建可能需要较长时间（5-15 分钟），后续增量构建会快很多。开发调试请使用 `npm run tauri dev`；需要快速出包时使用 `npm run tauri:build:fast`。

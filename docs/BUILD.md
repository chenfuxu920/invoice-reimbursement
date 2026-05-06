# 构建说明

## 前置条件

- **Node.js** 18+
- **Rust** 1.75+（推荐使用 rustup 安装）
- **系统依赖**：
  - **Linux**: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`
    ```bash
    sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
    ```
  - **Windows**: 无额外依赖（需安装 Visual Studio Build Tools）
  - **macOS**: 无额外依赖（需安装 Xcode Command Line Tools）

## 开发模式

```bash
npm install
npm run tauri dev
```

此命令会同时启动 Vite 开发服务器和 Tauri 窗口，支持前端热更新。

## 生产构建

```bash
npm run tauri build
```

构建产物在 `src-tauri/target/release/bundle/` 下，按操作系统不同包含：

| 平台 | 产物路径 | 安装包格式 |
|------|---------|-----------|
| Windows | `bundle/nsis/` | `.exe` (NSIS 安装程序) |
| macOS | `bundle/dmg/` | `.dmg` |
| Linux | `bundle/deb/` / `bundle/appimage/` | `.deb` / `.AppImage` |

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
opt-level = "s"       # 优化体积
lto = true            # 链接时优化
strip = true          # 剥离调试符号
codegen-units = 1     # 单编译单元，更好的优化
```

## 打包配置

Tauri 打包配置位于 `src-tauri/tauri.conf.json`，主要配置项：

- **identifier**: `com.invoice-reimbursement.app`
- **productName**: `发票报销助手`
- **窗口**: 1024×768（最小 800×600），居中显示，可调整大小
- **NSIS 安装程序**（Windows）: 支持简体中文/英文，当前用户安装模式
- **DEB 包**（Linux）: 声明 webkit2gtk 和 GTK3 依赖

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

release 构建由于启用了 LTO，首次构建可能需要较长时间（5-15 分钟），后续增量构建会快很多。开发调试请使用 `npm run tauri dev`。

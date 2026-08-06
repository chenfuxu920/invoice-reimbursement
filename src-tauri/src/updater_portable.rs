use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

/// 便携版更新信息（portable_check_update 返回值）
#[derive(Serialize)]
pub struct PortableUpdateInfo {
    pub latest_version: String,
    pub has_update: bool,
    pub download_url: String,
    pub signature_url: String,
}

// 编译期嵌入 tauri.conf.json（src/updater_portable.rs → ../tauri.conf.json = src-tauri/tauri.conf.json）
const TAURI_CONF: &str = include_str!("../tauri.conf.json");
const REPO_LATEST_API: &str =
    "https://api.github.com/repos/chenfuxu920/invoice-reimbursement/releases/latest";

/// 便携版判定：文件名后缀只是快速路径；核心是「安装上下文检测」——
/// Tauri NSIS 安装版同目录必有 uninstall.exe（installer.nsi 固定 WriteUninstaller "$INSTDIR\uninstall.exe"），
/// 便携版没有。所以用户重命名 exe 不影响判断；安装版 exe 被拷出安装目录后
/// 失去安装上下文，判为便携也符合实际（已是单 exe，走便携更新路径正确）。
/// 便携版统一命名 `invoice-reimbursement_v{version}_portable.exe`（全英文，规避 GitHub 资产名编码问题）。
/// 注意：仅 Windows 有"便携 vs 安装"之分。Linux AppImage / macOS .app 本身就是免安装形态，
/// 且官方 updater 插件原生支持其更新，必须返回 false 走官方插件路径——
/// 否则会误判为便携版去 GitHub 找 `_portable.exe` 资产（该平台 release 没有，更新会失败）。
fn is_portable_exe() -> bool {
    if !cfg!(target_os = "windows") {
        return false;
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let name = exe
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    // 快速路径：明确命名标记（历史约定，本地 create-portable.ps1 与 CI 统一）
    if name.ends_with("_portable.exe") {
        return true;
    }
    // 安装上下文：同目录有 NSIS 卸载器 = 安装版
    let has_uninstaller = exe
        .parent()
        .map(|d| d.join("uninstall.exe").is_file())
        .unwrap_or(false);
    !has_uninstaller
}

/// 从 tauri.conf.json 读取 plugins.updater.pubkey（= base64(整段 .pub 文件文本)）
fn read_pubkey_b64() -> Result<String, String> {
    let conf: serde_json::Value = serde_json::from_str(TAURI_CONF)
        .map_err(|e| format!("tauri.conf.json 解析失败: {}", e))?;
    conf["plugins"]["updater"]["pubkey"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "tauri.conf.json 缺少 plugins.updater.pubkey".to_string())
}

fn http_client() -> Result<reqwest::Client, String> {
    // GitHub API 要求 User-Agent，否则 403
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("invoice-reimbursement-updater"),
    );
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| format!("HTTP client 创建失败: {}", e))
}

fn no_update() -> PortableUpdateInfo {
    PortableUpdateInfo {
        latest_version: env!("CARGO_PKG_VERSION").to_string(),
        has_update: false,
        download_url: String::new(),
        signature_url: String::new(),
    }
}

/// 检查便携版更新：非便携版返回 Err("NOT_PORTABLE")，前端据此回退官方插件路径
#[tauri::command]
pub async fn portable_check_update() -> Result<PortableUpdateInfo, String> {
    if !is_portable_exe() {
        return Err("NOT_PORTABLE".into());
    }

    let client = http_client()?;
    let resp = client
        .get(REPO_LATEST_API)
        .send()
        .await
        .map_err(|e| format!("GitHub API 请求失败: {}", e))?;

    // 404：无已发布 release（如仍是 draft）→ 视为无更新，不算错误
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(no_update());
    }
    if !resp.status().is_success() {
        return Err(format!("GitHub API 返回 {}", resp.status()));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("GitHub API 响应解析失败: {}", e))?;

    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|e| format!("当前版本解析失败: {}", e))?;
    let mut info = no_update();

    // tag_name 形如 "app-v1.0.1"
    if let Some(tag) = body["tag_name"].as_str() {
        let version_str = tag.strip_prefix("app-v").unwrap_or(tag);
        if let Ok(latest) = semver::Version::parse(version_str) {
            info.latest_version = latest.to_string();
            info.has_update = latest > current;
        }
    }

    // assets：_portable.exe 及其同名 .sig
    if let Some(assets) = body["assets"].as_array() {
        let mut exe_name = String::new();
        for asset in assets {
            let name = asset["name"].as_str().unwrap_or("");
            if name.ends_with("_portable.exe") {
                exe_name = name.to_string();
                info.download_url = asset["browser_download_url"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
            }
        }
        for asset in assets {
            let name = asset["name"].as_str().unwrap_or("");
            if let Some(stripped) = name.strip_suffix(".sig") {
                if stripped == exe_name {
                    info.signature_url = asset["browser_download_url"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                }
            }
        }
    }

    Ok(info)
}

/// 校验 exe 签名（下载后、返回前必须做）。
/// pubkey 字段 = base64(整段 .pub 文本) → 先 decode 成文本 → PublicKeyBox
fn verify_download(exe_path: &std::path::Path, sig_path: &std::path::Path) -> Result<(), String> {
    let pubkey_b64 = read_pubkey_b64()?;
    let pub_text = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(pubkey_b64.trim())
            .map_err(|e| format!("公钥 base64 解码失败: {}", e))?,
    )
    .map_err(|e| format!("公钥文本不是合法 UTF-8: {}", e))?;

    let pk = minisign::PublicKeyBox::from_string(&pub_text)
        .map_err(|e| format!("公钥解析失败: {}", e))?
        .into_public_key()
        .map_err(|e| format!("公钥解析失败: {}", e))?;
    let sig_box = minisign::SignatureBox::from_file(sig_path)
        .map_err(|e| format!("签名解析失败: {}", e))?;

    let exe_bytes = std::fs::read(exe_path).map_err(|e| format!("读取 exe 失败: {}", e))?;
    // allow_legacy=true 与 tauri 官方一致
    minisign::verify(
        &pk,
        &sig_box,
        std::io::Cursor::new(&exe_bytes),
        false,
        false,
        true,
    )
    .map_err(|e| format!("签名验证失败: {}", e))
}

/// 下载新 exe + 签名，验证通过后返回新 exe 本地路径。
/// 下载期间 emit `portable-update-progress` { downloaded, total }（绝对字节数）。
#[tauri::command]
pub async fn portable_download_update(
    app: AppHandle,
    download_url: String,
    signature_url: String,
) -> Result<String, String> {
    let client = http_client()?;
    let tmp = std::env::temp_dir();
    let exe_path = tmp.join(format!("invoice-update-{}.exe", uuid::Uuid::new_v4()));
    let sig_path = tmp.join(format!("invoice-update-{}.sig", uuid::Uuid::new_v4()));

    let result: Result<(), String> = async {
        // 1. 流式下载 exe
        let mut resp = client
            .get(&download_url)
            .send()
            .await
            .map_err(|e| format!("exe 下载请求失败: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("exe 下载返回 {}", resp.status()));
        }
        let total = resp.content_length().unwrap_or(0);
        let mut file = tokio::fs::File::create(&exe_path)
            .await
            .map_err(|e| format!("创建临时文件失败: {}", e))?;
        let mut downloaded: u64 = 0;
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| format!("下载中断: {}", e))?
        {
            downloaded += chunk.len() as u64;
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("写入临时文件失败: {}", e))?;
            let _ = app.emit(
                "portable-update-progress",
                serde_json::json!({ "downloaded": downloaded, "total": total }),
            );
        }
        file.flush()
            .await
            .map_err(|e| format!("写入临时文件失败: {}", e))?;
        drop(file);

        // 2. 下载签名（小文件）
        let sig_resp = client
            .get(&signature_url)
            .send()
            .await
            .map_err(|e| format!("签名下载请求失败: {}", e))?;
        if !sig_resp.status().is_success() {
            return Err(format!("签名下载返回 {}", sig_resp.status()));
        }
        let sig_bytes = sig_resp
            .bytes()
            .await
            .map_err(|e| format!("签名读取失败: {}", e))?;
        tokio::fs::write(&sig_path, sig_bytes)
            .await
            .map_err(|e| format!("写入签名文件失败: {}", e))?;

        // 3. 签名验证
        verify_download(&exe_path, &sig_path)?;
        Ok(())
    }
    .await;

    match result {
        Ok(()) => Ok(exe_path.to_string_lossy().to_string()),
        Err(e) => {
            // 验证失败/下载失败：清理临时文件
            let _ = std::fs::remove_file(&exe_path);
            let _ = std::fs::remove_file(&sig_path);
            Err(e)
        }
    }
}

/// 便携版安装：生成替换脚本（等待主进程退出 → 当前 exe 改名 .old → 覆盖新 exe → 启动新 exe → 清理），随后退出当前进程。
/// 脚本用 PowerShell -EncodedCommand 承载（UTF-16LE base64，天然规避中文路径在 .bat 里的编码问题），
/// .bat 本体保持纯 ASCII（@echo off + start /min），通过 cmd /c start /min 无弹窗独立进程运行。
#[tauri::command]
pub async fn portable_install(app: AppHandle, new_exe_path: String) -> Result<(), String> {
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("获取当前 exe 路径失败: {}", e))?;
    let new_exe = std::path::PathBuf::from(&new_exe_path);
    if !new_exe.is_file() {
        return Err(format!("新 exe 不存在: {}", new_exe_path));
    }

    let old_exe = format!("{}.old", current_exe.display());
    let bat_path = std::env::temp_dir().join(format!("invoice-update-{}.bat", uuid::Uuid::new_v4()));

    let script = format!(
        "Start-Sleep -Seconds 3\n\
         Move-Item -Force -LiteralPath '{cur}' -Destination '{old}'\n\
         Copy-Item -LiteralPath '{new}' -Destination '{cur}'\n\
         Start-Process -FilePath '{cur}'\n\
         Remove-Item -Force -LiteralPath '{old}','{new}','{bat}' -ErrorAction SilentlyContinue\n",
        cur = current_exe.display(),
        old = old_exe,
        new = new_exe.display(),
        bat = bat_path.display(),
    );
    let enc = base64::engine::general_purpose::STANDARD.encode(
        script
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect::<Vec<u8>>(),
    );
    let bat = format!(
        "@echo off\r\nstart /min \"\" powershell.exe -NoProfile -ExecutionPolicy Bypass -EncodedCommand {}\r\n",
        enc
    );
    std::fs::write(&bat_path, bat).map_err(|e| format!("写入替换脚本失败: {}", e))?;

    // 独立进程运行替换脚本（/min 最小化窗口，不阻塞本进程），路径加引号
    let bat_arg = format!("\"{}\"", bat_path.display());
    std::process::Command::new("cmd")
        .args(["/c", "start", "/min", "", &bat_arg])
        .spawn()
        .map_err(|e| {
            let _ = std::fs::remove_file(&bat_path);
            format!("启动替换脚本失败: {}", e)
        })?;

    // 稍后退出当前进程，给 IPC 响应留出发送时间；替换脚本随后接管
    let exit_handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        exit_handle.exit(0);
    });

    Ok(())
}

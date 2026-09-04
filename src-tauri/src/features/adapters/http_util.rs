// 带代理感知的 HTTP 客户端构造。
// 代理来源优先级：
//   1. 环境变量 HTTPS_PROXY / ALL_PROXY（大小写均支持）
//   2. Windows 系统代理（Clash/Mihomo/v2ray 等开启"系统代理"后写入的 WinINET 设置）
// 这样用户像平时一样打开代理软件，应用内请求即自动走代理，无需额外配置。
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

pub fn build_client() -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(20))
        // 信任系统根证书（代理 TLS 拦截等场景更兼容）
        .connect_timeout(Duration::from_secs(12));

    if let Some(proxy_url) = detect_proxy() {
        log::info!("HTTP client using proxy: {proxy_url}");
        if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
            builder = builder.proxy(proxy);
        }
    }

    builder.build().expect("build http client")
}

fn env_proxy() -> Option<String> {
    for key in ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy", "HTTP_PROXY", "http_proxy"] {
        if let Ok(v) = std::env::var(key) {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn detect_proxy() -> Option<String> {
    if let Some(e) = env_proxy() {
        return normalize(&e);
    }
    system_proxy().and_then(|p| normalize(&p))
}

/// 补全 scheme：裸 "127.0.0.1:7897" → "http://127.0.0.1:7897"
fn normalize(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with("http://") || raw.starts_with("https://") || raw.starts_with("socks5://") {
        Some(raw.to_string())
    } else {
        Some(format!("http://{raw}"))
    }
}

#[cfg(windows)]
fn system_proxy() -> Option<String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
    let key = hkcu.open_subkey_with_flags(path, KEY_READ).ok()?;
    let enabled: u32 = key.get_value("ProxyEnable").unwrap_or(0);
    if enabled == 0 {
        return None;
    }
    let server: String = key.get_value("ProxyServer").ok()?;
    let server = server.trim();
    if server.is_empty() {
        return None;
    }
    // 两种格式："127.0.0.1:7897" 或 "http=127.0.0.1:7897;https=127.0.0.1:7897"
    if let Some(part) = server.split(';').map(str::trim).find(|s| s.starts_with("https=")) {
        return Some(part.trim_start_matches("https=").to_string());
    }
    if let Some(part) = server.split(';').map(str::trim).find(|s| s.starts_with("http=")) {
        return Some(part.trim_start_matches("http=").to_string());
    }
    Some(server.to_string())
}

#[cfg(not(windows))]
fn system_proxy() -> Option<String> {
    None
}

// 平台适配器框架。
// 当前实现：bilibili（L2 网页登录 + 读取关注 + 取关）。
// 新增平台 = 在本模块加一个子模块并在 lib.rs 注册命令；能力描述见下方。
// AdapterInfo/adapter_capabilities 为前端平台矩阵预留（V0.6+ 动态展示）。
#![allow(dead_code)]

pub mod bilibili;
pub mod http_util;

/// 平台适配器能力标识（供前端决定 UI 可用性）
/// 未来新增平台时在此模块扩展即可，不影响核心代码
pub fn adapter_capabilities() -> Vec<AdapterInfo> {
    vec![
        AdapterInfo {
            id: "bilibili".to_string(),
            display_name: "哔哩哔哩".to_string(),
            supports_qr_login: true,
            supports_auto_import: true,
            supports_unfollow: true,
            risk_level: "caution".to_string(),
        },
        AdapterInfo {
            id: "manual".to_string(),
            display_name: "手动导入（CSV/JSON）".to_string(),
            supports_qr_login: false,
            supports_auto_import: false,
            supports_unfollow: false,
            risk_level: "safe".to_string(),
        },
    ]
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterInfo {
    pub id: String,
    pub display_name: String,
    pub supports_qr_login: bool,
    pub supports_auto_import: bool,
    pub supports_unfollow: bool,
    /// safe | caution | advanced（对应合规文档 L1/L2/L3）
    pub risk_level: String,
}

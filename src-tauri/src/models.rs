use serde::{Deserialize, Serialize};

// ==================== 持久化模型 ====================

/// 关注账号（对应 accounts 表）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub platform: String,
    pub platform_account_id: String,
    pub display_name: String,
    pub avatar_path: Option<String>,
    pub account_type: String,
    pub homepage_url: Option<String>,
    pub followed_at: Option<i64>,
    pub interaction_level: String,
    pub interaction_metrics: Option<String>,
    pub status: String,
    pub note: Option<String>,
    pub imported_at: i64,
    pub updated_at: i64,
}

/// 账号 + 其所属分类 id 列表（前端列表渲染用）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountWithCategories {
    #[serde(flatten)]
    pub account: Account,
    pub category_ids: Vec<String>,
}

/// 分类节点（对应 categories 表）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

// ==================== 查询过滤 ====================

/// 账号列表过滤条件（全部可选，AND 组合）
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AccountFilter {
    pub search: Option<String>,
    pub platforms: Vec<String>,
    pub account_types: Vec<String>,
    pub interaction_levels: Vec<String>,
    pub statuses: Vec<String>,
    pub followed_after: Option<i64>,
    pub followed_before: Option<i64>,
    pub category_id: Option<String>,
    pub uncategorized: Option<bool>,
    /// followed_at | interaction | name | imported_at
    pub sort: Option<String>,
    /// asc | desc
    pub order: Option<String>,
}

// ==================== 导入/导出模型 ====================

/// 导入文件中的原始行（字段宽松，允许别名与缺失）
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ImportRow {
    pub platform: Option<String>,
    #[serde(alias = "platform_account_id")]
    pub platform_account_id: Option<String>,
    #[serde(alias = "display_name")]
    pub display_name: Option<String>,
    #[serde(alias = "account_type")]
    pub account_type: Option<String>,
    #[serde(alias = "homepage_url")]
    pub homepage_url: Option<String>,
    #[serde(alias = "followed_at")]
    pub followed_at: Option<serde_json::Value>,
    #[serde(alias = "interaction_level")]
    pub interaction_level: Option<String>,
    pub note: Option<String>,
}

/// 校验规范化后的账号数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedAccount {
    pub platform: String,
    pub platform_account_id: String,
    pub display_name: String,
    pub account_type: String,
    pub homepage_url: Option<String>,
    pub followed_at: Option<i64>,
    pub interaction_level: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowError {
    /// 行号（CSV 含表头，从 2 开始；JSON 数组从 1 开始；0 表示文件级错误）
    pub row: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseResult {
    pub total_rows: usize,
    pub valid_count: usize,
    pub skipped_count: usize,
    pub errors: Vec<RowError>,
    pub accounts: Vec<NormalizedAccount>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub inserted: usize,
    pub updated: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NameCount {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewCounts {
    pub total: i64,
    pub uncategorized: i64,
    pub platforms: Vec<NameCount>,
    pub interaction: Vec<NameCount>,
}

/// 合法枚举
pub const ACCOUNT_TYPES: &[&str] = &[
    "personal",
    "official",
    "subscription_account",
    "video_account",
    "brand",
    "unknown",
];
pub const INTERACTION_LEVELS: &[&str] = &["high", "medium", "low", "none", "unknown"];

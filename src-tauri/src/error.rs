use serde::Serialize;
use thiserror::Error;

/// 全局类型化错误。通过 serde 序列化为 { code, message } 供前端识别。
#[derive(Debug, Error, Serialize)]
#[serde(tag = "code", content = "message")]
pub enum AppError {
    #[error("数据库错误: {0}")]
    #[serde(rename = "DB_ERROR")]
    Db(String),

    #[error("数据校验失败: {0}")]
    #[serde(rename = "VALIDATION_ERROR")]
    Validation(String),

    #[error("文件读写错误: {0}")]
    #[serde(rename = "FILE_ERROR")]
    File(String),

    #[error("未找到资源: {0}")]
    #[serde(rename = "NOT_FOUND")]
    NotFound(String),

    #[error("分类层级最多支持 5 级")]
    #[serde(rename = "CATEGORY_DEPTH")]
    CategoryDepth,

    #[error("同级下已存在同名分类")]
    #[serde(rename = "CATEGORY_DUPLICATE_NAME")]
    DuplicateName,

    #[error("不能将分类移动到其自身或其子分类下")]
    #[serde(rename = "CATEGORY_CYCLE")]
    CategoryCycle,

    #[error("该分类包含子分类，请先移除或删除子分类")]
    #[serde(rename = "CATEGORY_HAS_CHILDREN")]
    HasChildren,

    #[error("不支持的文件格式: {0}（仅支持 .csv / .json）")]
    #[serde(rename = "UNSUPPORTED_FORMAT")]
    UnsupportedFormat(String),

    #[error("网络请求失败: {0}")]
    #[serde(rename = "NETWORK_ERROR")]
    Network(String),
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        log::error!("rusqlite error: {e}");
        AppError::Db(e.to_string())
    }
}

impl From<csv::Error> for AppError {
    fn from(e: csv::Error) -> Self {
        AppError::File(format!("CSV 处理错误: {e}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Validation(format!("JSON 解析错误: {e}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::File(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        log::error!("reqwest error: {e}");
        AppError::Network(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

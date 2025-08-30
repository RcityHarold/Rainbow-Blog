use serde::{Deserialize, Serialize};

/// 标准API响应格式
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data,
            message: None,
        }
    }

    pub fn success_with_message(data: T, message: String) -> Self {
        Self {
            success: true,
            data,
            message: Some(message),
        }
    }
}

/// 错误响应格式
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ErrorResponse {
    pub fn new(code: String, message: String) -> Self {
        Self {
            success: false,
            error: ErrorDetail {
                code,
                message,
                details: None,
            },
        }
    }

    pub fn with_details(code: String, message: String, details: serde_json::Value) -> Self {
        Self {
            success: false,
            error: ErrorDetail {
                code,
                message,
                details: Some(details),
            },
        }
    }
}
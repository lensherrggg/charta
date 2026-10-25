use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON 序列化错误: {0}")]
    Json(#[from] serde_json::Error),

    #[error("前端 {frontend} 解析失败: {message}")]
    Frontend {
        frontend: &'static str,
        message: String,
    },

    #[error("后端 {backend} 渲染失败: {message}")]
    Backend {
        backend: &'static str,
        message: String,
    },

    #[error("IR 不合法: {0}")]
    Ir(String),

    #[error("参数不合法: {0}")]
    InvalidArgument(String),
}

impl Error {
    pub fn frontend(frontend: &'static str, message: impl Into<String>) -> Self {
        Self::Frontend {
            frontend,
            message: message.into(),
        }
    }

    pub fn backend(backend: &'static str, message: impl Into<String>) -> Self {
        Self::Backend {
            backend,
            message: message.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

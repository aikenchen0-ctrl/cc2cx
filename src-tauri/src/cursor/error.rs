use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CursorError {
    #[error("Cursor 配置错误: {0}")]
    Config(String),
    #[error("Cursor 设置文件 IO 错误: {0}")]
    Io(#[from] io::Error),
    #[error("Cursor 设置文件解析失败: {0}")]
    Parse(String),
    #[error("Cursor 设置文件存在外部修改，未执行恢复: {0}")]
    ExternalModification(String),
    #[error("Cursor 代理地址无效: {0}")]
    InvalidProxyUrl(String),
    #[error("Cursor 协议错误: {0}")]
    Protocol(String),
}

pub type Result<T> = std::result::Result<T, CursorError>;

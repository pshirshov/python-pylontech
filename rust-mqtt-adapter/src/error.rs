use thiserror::Error;

use crate::protocol::ProtocolError;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("invalid state: {0}")]
    InvalidState(String),
    #[error("mqtt disconnected: {0}")]
    MqttDisconnected(String),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("mqtt client error: {0}")]
    MqttClient(#[from] rumqttc::ClientError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("protocol error: {0}")]
    Protocol(#[from] ProtocolError),
}

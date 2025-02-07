use crate::BusMessage;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Error broadcasting: {0}")]
    BroadcastSend(#[from] tokio::sync::broadcast::error::SendError<BusMessage>),

    #[error("Error receiving broadcast: {0}")]
    BroadcastReceive(#[from] tokio::sync::broadcast::error::RecvError),

    #[error("Error: {0}")]
    General(String),

    #[error("HTTP error: {0}")]
    HttpError(#[from] http::Error),

    #[error("Nostr: {0}")]
    Nostr(#[from] nostr_proto::Error),

    #[error("I/O Error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid URI: {0}")]
    InvalidUri(#[from] http::uri::InvalidUri),

    #[error("Error sending mpsc: {0}")]
    MpscSend(#[from] tokio::sync::mpsc::error::SendError<BusMessage>),

    #[error("Bad integer: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    #[error("HTTP Request Error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("SerdeJson Error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("SQL: {0}")]
    Sql(#[from] rusqlite::Error),

    #[error("Tauri: {0}")]
    Tauri(#[from] tauri::Error),

    #[error("URL has empty hostname")]
    UrlHasEmptyHostname,

    #[error("URL has no hostname")]
    UrlHasNoHostname,

    #[error("Websocket: {0}")]
    Websocket(#[from] tungstenite::Error),
}

impl From<String> for Error {
    fn from(s: String) -> Error {
        Error::General(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Error {
        Error::General(s.to_string())
    }
}

pub mod jpeg;
pub mod png;

use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// 無効な画像フォーマット
    InvalidFormat(String),
    /// I/Oエラー
    Io(std::io::Error),
    /// パースエラー
    ParseError(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidFormat(msg) => write!(f, "Invalid format: {msg}"),
            Error::Io(err) => write!(f, "IO error: {err}"),
            Error::ParseError(msg) => write!(f, "Parse error: {msg}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<jpeg_decoder::Error> for Error {
    fn from(err: jpeg_decoder::Error) -> Self {
        Error::ParseError(format!("JPEG decode error: {err}"))
    }
}

impl From<jpeg_encoder::EncodingError> for Error {
    fn from(err: jpeg_encoder::EncodingError) -> Self {
        Error::ParseError(format!("JPEG encode error: {err}"))
    }
}

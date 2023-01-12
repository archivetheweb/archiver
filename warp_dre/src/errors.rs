use std::{fmt::Display, num::ParseIntError};

#[derive(Debug)]
pub enum WarpDREError {
    ArgumentError { arg: String },
    URLError { url: String },
    IOError(std::io::Error),
    ReqwestError(reqwest::Error),
    ParseIntError(ParseIntError),
}

impl Display for WarpDREError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WarpDREError::ArgumentError { arg } => write!(f, "argument not valid: {}", arg),
            WarpDREError::IOError(e) => write!(f, "io: {}", e),
            WarpDREError::ReqwestError(e) => write!(f, "reqwest: {}", e),
            WarpDREError::URLError { url } => write!(f, "invalid url: {}", url),
            WarpDREError::ParseIntError(e) => write!(f, "parse int error: {}", e),
        }
    }
}

impl WarpDREError {
    pub fn invalid_argument(arg: &str) -> WarpDREError {
        WarpDREError::ArgumentError {
            arg: arg.to_string(),
        }
    }
    pub fn invalid_url(url: &str) -> WarpDREError {
        WarpDREError::URLError {
            url: url.to_string(),
        }
    }
}

impl From<std::io::Error> for WarpDREError {
    fn from(e: std::io::Error) -> Self {
        WarpDREError::IOError(e)
    }
}

impl From<reqwest::Error> for WarpDREError {
    fn from(e: reqwest::Error) -> Self {
        WarpDREError::ReqwestError(e)
    }
}

impl From<std::num::ParseIntError> for WarpDREError {
    fn from(e: std::num::ParseIntError) -> Self {
        WarpDREError::ParseIntError(e)
    }
}

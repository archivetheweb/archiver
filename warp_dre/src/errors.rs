use thiserror::Error;

#[derive(Debug, Error)]
pub enum WarpDREError {
    #[error("warp_dre: argument not valid {0}")]
    ArgumentError(String),

    #[error("warp_dre::warp_gateway {0}")]
    WarpGatewayError(String),
}

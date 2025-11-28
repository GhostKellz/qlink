//! Error types for QLINK operations

use thiserror::Error;

/// Result type alias using QLINK's Error type
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for QLINK operations
#[derive(Error, Debug)]
pub enum Error {
    /// Camera-related errors
    #[error("Camera error: {0}")]
    Camera(String),

    /// Camera device not found
    #[error("Camera device not found: {0}")]
    CameraNotFound(String),

    /// Failed to capture frame from camera
    #[error("Frame capture failed: {0}")]
    FrameCapture(String),

    /// QR code decoding failed
    #[error("Failed to decode QR code: {0}")]
    QrDecode(String),

    /// No QR code found in frame
    #[error("No QR code found in frame")]
    NoQrCodeFound,

    /// QR code encoding failed
    #[error("Failed to encode QR code: {0}")]
    QrEncode(String),

    /// Invalid Keystone payload
    #[error("Invalid Keystone payload: {0}")]
    InvalidKeystonePayload(String),

    /// UR parsing error
    #[error("UR parsing error: {0}")]
    UrParse(String),

    /// CBOR encoding/decoding error
    #[error("CBOR error: {0}")]
    Cbor(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Image processing error
    #[error("Image processing error: {0}")]
    Image(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

// Implement From conversions for common error types

impl From<image::ImageError> for Error {
    fn from(e: image::ImageError) -> Self {
        Error::Image(e.to_string())
    }
}

// V4L errors are converted manually in camera module

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Other(format!("JSON error: {}", e))
    }
}

impl From<hex::FromHexError> for Error {
    fn from(e: hex::FromHexError) -> Self {
        Error::Other(format!("Hex decode error: {}", e))
    }
}

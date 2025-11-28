//! QLINK - Linux-first air-gapped QR bridge for Keystone Pro 3
//!
//! This library provides a high-performance interface for communicating with
//! Keystone Pro 3 hardware wallets via QR codes on Linux systems.
//!
//! # Features
//!
//! - **Camera Integration**: Direct V4L2 access for low-latency QR scanning
//! - **QR Processing**: Fast encoding/decoding with debouncing
//! - **Keystone Protocol**: Full UR registry support for wallet operations
//! - **Async-first**: Built on Tokio for non-blocking operations
//!
//! # Example
//!
//! ```no_run
//! use qlink::{QlinkScanner, ScanConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Initialize scanner with default camera
//!     let mut scanner = QlinkScanner::new(ScanConfig::default()).await?;
//!
//!     // Scan for Keystone wallet connect QR
//!     let payload = scanner.scan_keystone().await?;
//!
//!     println!("Scanned payload: {:?}", payload);
//!     Ok(())
//! }
//! ```

#![warn(missing_docs, rust_2024_compatibility)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod config;
pub mod error;
pub mod logging;
pub mod metrics;
pub mod output;
pub mod qr;

#[cfg(feature = "camera")]
#[cfg_attr(docsrs, doc(cfg(feature = "camera")))]
pub mod camera;

pub mod keystone;

// Re-exports for convenience
pub use error::{Error, Result};

#[cfg(feature = "camera")]
pub use camera::{Camera, CameraConfig, CameraDevice};

pub use config::{ApiOptions, CameraOptions, LogRotation, LoggingOptions, QlinkConfig};
pub use keystone::{KeystoneMessage, KeystoneMetadata, KeystonePayload, PayloadEncoding};
pub use qr::{QrDecoder, QrEncoder, QrPayload};

/// High-level scanner interface combining camera + QR + Keystone
#[cfg(feature = "camera")]
pub struct QlinkScanner {
    /// The camera device
    pub camera: Camera,
    decoder: QrDecoder,
}

#[cfg(feature = "camera")]
impl QlinkScanner {
    /// Create a new scanner with the given configuration
    pub async fn new(config: ScanConfig) -> Result<Self> {
        let camera = Camera::open(config.camera_config).await?;
        let decoder = QrDecoder::new();

        Ok(Self { camera, decoder })
    }

    /// Scan once and return the first QR code found
    pub async fn scan_once(&mut self) -> Result<QrPayload> {
        let frame = self.camera.capture_frame().await?;
        self.decoder.decode(&frame)
    }

    /// Scan continuously for Keystone-specific QR codes, including multi-part UR streams
    pub async fn scan_keystone(&mut self) -> Result<KeystonePayload> {
        use crate::keystone::multipart::MultiPartDecoder;

        let mut multipart_decoder = MultiPartDecoder::new();

        loop {
            match self.scan_once().await {
                Ok(qr) => {
                    match KeystonePayload::try_from(qr.clone()) {
                        Ok(payload) => return Ok(payload),
                        Err(Error::InvalidKeystonePayload(_)) | Err(Error::UrParse(_)) => {
                            if let Some(text) = qr.as_str() {
                                if text.starts_with("ur:") {
                                    match multipart_decoder.receive(text) {
                                        Ok(progress) => {
                                            if progress.complete {
                                                match multipart_decoder.result() {
                                                    Ok(payload) => return Ok(payload),
                                                    Err(err) => tracing::warn!(
                                                        "Failed to finalize multi-part UR: {err}"
                                                    ),
                                                }
                                            } else {
                                                tracing::debug!(
                                                    parts_received = progress.parts_received,
                                                    percentage = progress.percentage,
                                                    message = %progress.message(),
                                                    "Keystone multi-part progress",
                                                );
                                            }
                                        }
                                        Err(err) => {
                                            tracing::warn!("Failed to process UR fragment: {err}");
                                        }
                                    }
                                }
                            }
                        }
                        Err(Error::NoQrCodeFound) => {
                            // Decoder could not find QR data in frame; continue scanning
                        }
                        Err(other) => {
                            tracing::warn!("Failed to decode QR payload: {other}");
                        }
                    }
                }
                Err(Error::NoQrCodeFound) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(90)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

/// Configuration for QR scanning operations
#[derive(Debug, Clone)]
pub struct ScanConfig {
    /// Camera configuration
    pub camera_config: CameraConfig,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            camera_config: CameraConfig::default(),
        }
    }
}

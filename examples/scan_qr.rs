//! Scan QR codes from a webcam
//!
//! Usage: cargo run --example scan_qr

use qlink::{CameraConfig, QlinkScanner, ScanConfig};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging with INFO level
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    println!("QLINK Camera Scanner");
    println!("====================\n");

    // List available cameras
    println!("Available cameras:");
    match qlink::camera::list_devices() {
        Ok(devices) => {
            for dev in &devices {
                println!("  [{}] {} ({})", dev.index, dev.name, dev.path);
            }
        }
        Err(e) => {
            eprintln!("Error listing cameras: {}", e);
            return Ok(());
        }
    }

    println!("\nInitializing scanner...");

    // Create scanner with default config (or use CameraConfig::facecam() for Elgato)
    let config = ScanConfig {
        camera_config: CameraConfig::qr_optimized(),
    };

    let mut scanner = QlinkScanner::new(config).await?;

    println!("✓ Scanner initialized: {}", scanner.camera.info().name);
    println!("\nScanning for QR codes (Ctrl+C to stop)...\n");

    loop {
        match scanner.scan_once().await {
            Ok(qr) => {
                println!("✓ QR Code detected!");
                if let Some(text) = qr.as_str() {
                    println!("  Content: {}", text);

                    // Try to parse as Keystone payload
                    match qlink::KeystonePayload::try_from(qr) {
                        Ok(keystone) => {
                            println!("  Type: Keystone UR");
                            println!("  UR Type: {}", keystone.ur_type);
                        }
                        Err(_) => {
                            println!("  Type: Generic QR");
                        }
                    }
                } else {
                    println!("  Content: Binary data ({} bytes)", qr.as_bytes().len());
                }
                println!();

                // Pause briefly after successful scan
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
            Err(qlink::Error::NoQrCodeFound) => {
                // Normal - keep scanning
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }
}

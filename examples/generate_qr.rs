//! Generate a QR code and save it to a file
//!
//! Usage: cargo run --example generate_qr

use qlink::{QrEncoder, QrPayload};

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let encoder = QrEncoder::new();

    // Generate a simple QR code
    let payload = QrPayload::from_string("Hello from QLINK!".to_string());
    let qr_image = encoder.encode(&payload)?;

    // Save to file
    qr_image.save("qr_output.png")?;

    println!("✓ QR code generated and saved to qr_output.png");

    // Generate a Keystone-style UR QR code
    let ur_string = "ur:crypto-account/0102030405";
    let ur_payload = QrPayload::from_string(ur_string.to_string());
    let ur_qr = encoder.encode(&ur_payload)?;

    ur_qr.save("qr_ur_example.png")?;
    println!("✓ UR QR code generated and saved to qr_ur_example.png");
    println!("  Content: {}", ur_string);

    Ok(())
}

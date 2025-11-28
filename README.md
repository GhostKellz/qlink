# QLINK

> Linux-first air-gapped QR bridge for Keystone Pro 3 hardware wallet

[![Rust](https://img.shields.io/badge/rust-1.85%2B-blue.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**QLINK** is a high-performance Rust library that enables seamless, secure communication between Linux desktops and the **Keystone Pro 3** hardware wallet via QR codes. Built with V4L2 for optimal camera performance on Linux, QLINK makes air-gapped crypto operations fluid and developer-friendly.

## Features

- **🎥 Native V4L2 Camera Support** - Direct Linux camera access with zero overhead
- **⚡ Fast QR Processing** - Continuous scanning optimized for Keystone workflows
- **🔐 Air-Gapped Security** - Private keys never leave your hardware wallet
- **🦀 Pure Rust** - Memory-safe, fast, and built for the Rust 2024 edition
- **📦 Library-First** - Easy integration into wallets and Tauri apps
- **🌐 Multi-Chain Ready** - Ethereum, Hedera, Solana, XRP, Stellar, Polygon, and more

## Quick Start

### Prerequisites

- Arch Linux (or any Linux with V4L2 support)
- Rust 1.85+ with 2024 edition support
- A V4L2-compatible webcam (Elgato Facecam recommended)
- Keystone Pro 3 hardware wallet

### Installation

Add QLINK to your \`Cargo.toml\`:

\`\`\`toml
[dependencies]
qlink = { git = "https://github.com/ghostkellz/qlink" }
\`\`\`

Or for local development:

\`\`\`bash
git clone https://github.com/ghostkellz/qlink
cd qlink
cargo build --release
\`\`\`

### Basic Usage

#### Generate a QR Code

\`\`\`rust
use qlink::{QrEncoder, QrPayload};

fn main() -> anyhow::Result<()> {
    let encoder = QrEncoder::new();
    let payload = QrPayload::from_string("ur:crypto-account/...".to_string());
    let qr_image = encoder.encode(&payload)?;

    qr_image.save("keystone_connect.png")?;
    Ok(())
}
\`\`\`

#### Scan QR Codes from Webcam

\`\`\`rust
use qlink::{QlinkScanner, ScanConfig, CameraConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize scanner with default camera
    let config = ScanConfig {
        camera_config: CameraConfig::facecam(), // Or ::default()
    };

    let mut scanner = QlinkScanner::new(config).await?;

    // Scan for a Keystone QR code
    let keystone_payload = scanner.scan_keystone().await?;

    println!("Received from Keystone: {:?}", keystone_payload);
    Ok(())
}
\`\`\`

## Examples

Run the included examples:

\`\`\`bash
# Generate QR codes
cargo run --example generate_qr

# Scan from webcam
cargo run --example scan_qr
\`\`\`

## Roadmap

### Phase 1: Core Library ✅ (Current)
- [x] V4L2 camera integration
- [x] QR encode/decode
- [x] Basic UR protocol support
- [x] Multi-chain message types

### Phase 2: Protocol Completeness 🚧
- [ ] Full CBOR encoding/decoding
- [ ] Multi-part QR codes
- [ ] Real Keystone firmware testing

### Phase 3: SDK & Daemon 📅
- [ ] Extract \`qlink-sdk\` crate
- [ ] Optional daemon for browser extensions

## License

MIT License

## Author

**Christopher Kelley** <ckelley@ghostkellz.sh>  
CK Technology

---

**QLINK** - Making air-gapped crypto fluid on Linux 🔐

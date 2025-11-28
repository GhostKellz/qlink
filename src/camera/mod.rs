//! V4L2 camera interface for Linux
//!
//! Provides low-latency access to webcams via the Video4Linux2 API.
//! Optimized for continuous QR code scanning with the Elgato Facecam.

mod config;
mod device;

pub use config::{CameraConfig, PixelFormat};
pub use device::{Camera, CameraDevice};

use crate::error::{Error, Result};

/// List available V4L2 camera devices
pub fn list_devices() -> Result<Vec<CameraDevice>> {
    let mut devices = Vec::new();

    // Enumerate /dev/video* devices
    for i in 0..10 {
        let path = format!("/dev/video{}", i);

        match v4l::Device::new(i) {
            Ok(dev) => {
                if let Ok(caps) = dev.query_caps() {
                    // Only include capture devices
                    if caps
                        .capabilities
                        .contains(v4l::capability::Flags::VIDEO_CAPTURE)
                    {
                        devices.push(CameraDevice {
                            index: i,
                            path: path.clone(),
                            name: caps.card,
                            driver: caps.driver,
                            bus_info: caps.bus,
                        });
                    }
                }
            }
            Err(_) => continue,
        }
    }

    if devices.is_empty() {
        return Err(Error::CameraNotFound(
            "No V4L2 capture devices found".to_string(),
        ));
    }

    Ok(devices)
}

/// Find a camera device by name (case-insensitive substring match)
pub fn find_device_by_name(name: &str) -> Result<CameraDevice> {
    let devices = list_devices()?;
    let name_lower = name.to_lowercase();

    devices
        .into_iter()
        .find(|d| d.name.to_lowercase().contains(&name_lower))
        .ok_or_else(|| Error::CameraNotFound(format!("No device matching '{}'", name)))
}

/// Find the Elgato Facecam (convenience function)
pub fn find_facecam() -> Result<CameraDevice> {
    find_device_by_name("facecam")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_devices() {
        // This test will only pass if V4L2 devices are available
        match list_devices() {
            Ok(devices) => {
                println!("Found {} camera(s)", devices.len());
                for dev in devices {
                    println!("  - {} at {}", dev.name, dev.path);
                }
            }
            Err(e) => {
                println!("No cameras found (expected on CI): {}", e);
            }
        }
    }
}

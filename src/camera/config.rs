//! Camera configuration

use serde::{Deserialize, Serialize};

/// Camera configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    /// Camera device index (e.g., 0 for /dev/video0)
    /// If None, will auto-detect the first available camera
    pub device_index: Option<usize>,

    /// Camera device name to search for (e.g., "Facecam")
    /// If set, this takes priority over device_index
    pub device_name: Option<String>,

    /// Frame width in pixels
    pub width: u32,

    /// Frame height in pixels
    pub height: u32,

    /// Frames per second
    pub fps: u32,

    /// Pixel format (MJPEG recommended for performance)
    pub format: PixelFormat,

    /// Number of V4L2 buffers to keep mapped (higher = smoother but more memory)
    pub buffer_count: u32,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            device_index: None, // Auto-detect
            device_name: None,
            width: 1920, // Full HD for best QR decode quality
            height: 1080,
            fps: 30,
            format: PixelFormat::Mjpeg,
            buffer_count: 4,
        }
    }
}

impl CameraConfig {
    /// Preset for Elgato Facecam
    pub fn facecam() -> Self {
        Self {
            device_name: Some("Facecam".to_string()),
            width: 1920,
            height: 1080,
            fps: 30,
            format: PixelFormat::Mjpeg,
            buffer_count: 6,
            ..Default::default()
        }
    }

    /// Preset for lower-end webcams (lower resolution/fps for compatibility)
    pub fn compatible() -> Self {
        Self {
            width: 640,
            height: 480,
            fps: 15,
            format: PixelFormat::Yuyv,
            buffer_count: 4,
            ..Default::default()
        }
    }

    /// Preset optimized for QR scanning (high resolution, moderate FPS)
    pub fn qr_optimized() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 15, // Lower FPS to reduce CPU usage
            format: PixelFormat::Mjpeg,
            buffer_count: 5,
            ..Default::default()
        }
    }
}

/// Pixel format for camera capture
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelFormat {
    /// Motion JPEG (compressed, recommended for high resolution)
    Mjpeg,
    /// YUYV 4:2:2 (uncompressed, better compatibility)
    Yuyv,
    /// RGB24 (uncompressed, high bandwidth)
    Rgb24,
}

impl PixelFormat {
    /// Convert to V4L2 FourCC code
    pub fn to_fourcc(self) -> v4l::FourCC {
        match self {
            PixelFormat::Mjpeg => v4l::FourCC::new(b"MJPG"),
            PixelFormat::Yuyv => v4l::FourCC::new(b"YUYV"),
            PixelFormat::Rgb24 => v4l::FourCC::new(b"RGB3"),
        }
    }

    /// Canonical string representation for configuration files
    pub fn as_str(self) -> &'static str {
        match self {
            PixelFormat::Mjpeg => "mjpeg",
            PixelFormat::Yuyv => "yuyv",
            PixelFormat::Rgb24 => "rgb24",
        }
    }

    /// Parse from a user-provided string (case-insensitive)
    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "mjpeg" | "mjpg" => Some(PixelFormat::Mjpeg),
            "yuyv" => Some(PixelFormat::Yuyv),
            "rgb" | "rgb24" => Some(PixelFormat::Rgb24),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CameraConfig::default();
        assert_eq!(config.width, 1920);
        assert_eq!(config.height, 1080);
    }

    #[test]
    fn test_facecam_preset() {
        let config = CameraConfig::facecam();
        assert_eq!(config.device_name, Some("Facecam".to_string()));
        assert_eq!(config.format, PixelFormat::Mjpeg);
    }

    #[test]
    fn test_pixel_format_fourcc() {
        assert_eq!(PixelFormat::Mjpeg.to_fourcc(), v4l::FourCC::new(b"MJPG"));
        assert_eq!(PixelFormat::Yuyv.to_fourcc(), v4l::FourCC::new(b"YUYV"));
    }

    #[test]
    fn test_pixel_format_from_str() {
        assert_eq!(PixelFormat::from_str("MJPEG"), Some(PixelFormat::Mjpeg));
        assert_eq!(PixelFormat::from_str("yuyv"), Some(PixelFormat::Yuyv));
        assert_eq!(PixelFormat::from_str("rgb24"), Some(PixelFormat::Rgb24));
        assert!(PixelFormat::from_str("invalid").is_none());
    }
}

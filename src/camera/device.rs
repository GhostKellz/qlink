//! Camera device implementation

use crate::camera::{CameraConfig, find_device_by_name, list_devices};
use crate::error::{Error, Result};
use image::{DynamicImage, ImageBuffer};
use serde::{Deserialize, Serialize};
use std::mem;
use std::sync::Arc;
use tokio::sync::Mutex;
use v4l::buffer::Type;
use v4l::io::traits::CaptureStream;
use v4l::prelude::*;
use v4l::video::Capture;

/// Information about a camera device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraDevice {
    /// Device index (e.g., 0 for /dev/video0)
    pub index: usize,
    /// Device path (e.g., "/dev/video0")
    pub path: String,
    /// Device name (e.g., "Elgato Facecam")
    pub name: String,
    /// Driver name
    pub driver: String,
    /// Bus information
    pub bus_info: String,
}

/// Internal camera resources guarded by a mutex so frames can be captured from async contexts
struct CameraInner {
    /// Memory-mapped V4L2 stream kept warm between captures
    stream: MmapStream<'static>,
    /// Owning handle to the V4L device. Drop order ensures the stream is released first.
    _device: Box<Device>,
}

/// Camera handle for capturing frames
pub struct Camera {
    inner: Arc<Mutex<CameraInner>>,
    config: CameraConfig,
    info: CameraDevice,
}

impl Camera {
    /// Open a camera with the given configuration
    pub async fn open(config: CameraConfig) -> Result<Self> {
        // Determine which device to open
        let device_info = if let Some(ref name) = config.device_name {
            // Find by name
            find_device_by_name(name)?
        } else if let Some(index) = config.device_index {
            // Use specific index
            let devices = list_devices()?;
            devices
                .into_iter()
                .find(|d| d.index == index)
                .ok_or_else(|| {
                    Error::CameraNotFound(format!("Device /dev/video{} not found", index))
                })?
        } else {
            // Auto-detect first available
            let devices = list_devices()?;
            devices
                .into_iter()
                .next()
                .ok_or_else(|| Error::CameraNotFound("No cameras available".to_string()))?
        };

        tracing::info!(
            "Opening camera: {} at {}",
            device_info.name,
            device_info.path
        );

        // Open the device
        let dev = Device::new(device_info.index)
            .map_err(|e| Error::Camera(format!("Failed to open device: {}", e)))?;

        // Set format
        let mut fmt = dev
            .format()
            .map_err(|e| Error::Camera(format!("Failed to get format: {}", e)))?;

        fmt.width = config.width;
        fmt.height = config.height;
        fmt.fourcc = config.format.to_fourcc();

        dev.set_format(&fmt)
            .map_err(|e| Error::Camera(format!("Failed to set format: {}", e)))?;

        // Set frame rate
        let mut params = dev
            .params()
            .map_err(|e| Error::Camera(format!("Failed to get params: {}", e)))?;

        params.interval = v4l::Fraction::new(1, config.fps);

        dev.set_params(&params)
            .map_err(|e| Error::Camera(format!("Failed to set params: {}", e)))?;

        tracing::info!(
            "Camera configured: {}x{} @ {} fps ({})",
            fmt.width,
            fmt.height,
            config.fps,
            String::from_utf8_lossy(&fmt.fourcc.repr)
        );

        // Promote the device to a boxed handle so we can safely extend its lifetime for the stream.
        // SAFETY: The boxed device outlives the mmap stream and both are dropped together inside CameraInner.
        let device = Box::new(dev);
        let static_device: &'static Device =
            unsafe { mem::transmute::<&Device, &'static Device>(device.as_ref()) };

        let buffer_count = config.buffer_count.max(2);

        let stream = MmapStream::with_buffers(static_device, Type::VideoCapture, buffer_count)
            .map_err(|e| Error::FrameCapture(format!("Failed to create stream: {}", e)))?;

        Ok(Self {
            inner: Arc::new(Mutex::new(CameraInner {
                stream,
                _device: device,
            })),
            config,
            info: device_info,
        })
    }

    /// Get camera device information
    pub fn info(&self) -> &CameraDevice {
        &self.info
    }

    /// Get camera configuration
    pub fn config(&self) -> &CameraConfig {
        &self.config
    }

    /// Capture a single frame
    pub async fn capture_frame(&self) -> Result<DynamicImage> {
        let mut inner = self.inner.lock().await;

        let (buf, _meta) = inner
            .stream
            .next()
            .map_err(|e| Error::FrameCapture(format!("Failed to capture: {}", e)))?;

        // Decode the frame based on format
        let img = self.decode_frame(buf)?;

        Ok(img)
    }

    /// Decode a frame buffer into an image
    fn decode_frame(&self, buf: &[u8]) -> Result<DynamicImage> {
        match self.config.format {
            crate::camera::config::PixelFormat::Mjpeg => {
                // MJPEG is already compressed JPEG
                image::load_from_memory_with_format(buf, image::ImageFormat::Jpeg)
                    .map_err(|e| Error::Image(format!("MJPEG decode failed: {}", e)))
            }
            crate::camera::config::PixelFormat::Yuyv => {
                // Convert YUYV to RGB
                self.yuyv_to_rgb(buf)
            }
            crate::camera::config::PixelFormat::Rgb24 => {
                // Already RGB24
                ImageBuffer::from_raw(self.config.width, self.config.height, buf.to_vec())
                    .map(DynamicImage::ImageRgb8)
                    .ok_or_else(|| Error::Image("Failed to create RGB image".to_string()))
            }
        }
    }

    /// Convert YUYV to RGB
    fn yuyv_to_rgb(&self, yuyv: &[u8]) -> Result<DynamicImage> {
        let width = self.config.width as usize;
        let height = self.config.height as usize;
        let mut rgb = vec![0u8; width * height * 3];

        for y in 0..height {
            for x in 0..(width / 2) {
                let yuyv_idx = (y * width * 2) + (x * 4);
                let rgb_idx = (y * width * 3) + (x * 6);

                if yuyv_idx + 3 >= yuyv.len() {
                    break;
                }

                let y0 = yuyv[yuyv_idx] as i32;
                let u = yuyv[yuyv_idx + 1] as i32 - 128;
                let y1 = yuyv[yuyv_idx + 2] as i32;
                let v = yuyv[yuyv_idx + 3] as i32 - 128;

                // Convert YUV to RGB (first pixel)
                let r0 = (y0 + ((v * 1436) >> 10)).clamp(0, 255) as u8;
                let g0 = (y0 - ((u * 352 + v * 731) >> 10)).clamp(0, 255) as u8;
                let b0 = (y0 + ((u * 1814) >> 10)).clamp(0, 255) as u8;

                // Second pixel
                let r1 = (y1 + ((v * 1436) >> 10)).clamp(0, 255) as u8;
                let g1 = (y1 - ((u * 352 + v * 731) >> 10)).clamp(0, 255) as u8;
                let b1 = (y1 + ((u * 1814) >> 10)).clamp(0, 255) as u8;

                if rgb_idx + 5 < rgb.len() {
                    rgb[rgb_idx] = r0;
                    rgb[rgb_idx + 1] = g0;
                    rgb[rgb_idx + 2] = b0;
                    rgb[rgb_idx + 3] = r1;
                    rgb[rgb_idx + 4] = g1;
                    rgb[rgb_idx + 5] = b1;
                }
            }
        }

        ImageBuffer::from_raw(self.config.width, self.config.height, rgb)
            .map(DynamicImage::ImageRgb8)
            .ok_or_else(|| Error::Image("Failed to create RGB image from YUYV".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_camera_open() {
        // This test will only work if a camera is available
        let config = CameraConfig::default();
        match Camera::open(config).await {
            Ok(camera) => {
                println!("Opened camera: {}", camera.info().name);
            }
            Err(e) => {
                println!("No camera available (expected on CI): {}", e);
            }
        }
    }
}

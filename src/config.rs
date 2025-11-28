//! QLINK runtime configuration handling

use crate::camera::{CameraConfig, PixelFormat};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Top-level configuration structure persisted to disk or environment
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct QlinkConfig {
    /// Camera capture configuration overrides
    pub camera: CameraOptions,
    /// Logging configuration
    pub logging: LoggingOptions,
    /// Local API daemon configuration
    pub api: ApiOptions,
}

impl Default for QlinkConfig {
    fn default() -> Self {
        Self {
            camera: CameraOptions::default(),
            logging: LoggingOptions::default(),
            api: ApiOptions::default(),
        }
    }
}

impl QlinkConfig {
    /// Load configuration from an explicit path or fall back to discovered defaults.
    pub fn load(explicit_path: Option<&Path>) -> Result<Self> {
        let mut config = if let Some(path) = explicit_path {
            Self::from_file(path)?
        } else if let Some(path) = Self::discover_file()? {
            tracing::info!("Using configuration file: {}", path.display());
            Self::from_file(&path)?
        } else {
            tracing::debug!("No qlink.toml / qlink.yaml found, using defaults");
            Self::default()
        };

        config.apply_env_overrides();
        Ok(config)
    }

    /// Attempt to locate a configuration file in common locations.
    fn discover_file() -> Result<Option<PathBuf>> {
        let cwd =
            env::current_dir().map_err(|e| Error::Config(format!("Failed to read cwd: {e}")))?;
        for candidate in ["qlink.toml", "qlink.yaml", "qlink.yml"] {
            let path = cwd.join(candidate);
            if path.exists() {
                return Ok(Some(path));
            }
        }

        if let Some(xdg_config) = env::var_os("XDG_CONFIG_HOME") {
            let base = PathBuf::from(xdg_config).join("qlink");
            for candidate in ["config.toml", "config.yaml"] {
                let path = base.join(candidate);
                if path.exists() {
                    return Ok(Some(path));
                }
            }
        }

        Ok(None)
    }

    /// Read configuration from a concrete file path.
    pub fn from_file(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("Failed to read {}: {e}", path.display())))?;

        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "toml" => toml::from_str(&contents).map_err(|e| {
                Error::Config(format!("Failed to parse TOML {}: {e}", path.display()))
            }),
            "yaml" | "yml" => serde_yaml::from_str(&contents).map_err(|e| {
                Error::Config(format!("Failed to parse YAML {}: {e}", path.display()))
            }),
            other => Err(Error::Config(format!(
                "Unsupported config format '{}', expected toml/yaml",
                other
            ))),
        }
    }

    /// Apply environment variable overrides after file/default loading.
    fn apply_env_overrides(&mut self) {
        self.camera.apply_env_overrides();
        self.logging.apply_env_overrides();
        self.api.apply_env_overrides();
    }

    /// Produce a fully resolved camera configuration ready to open the V4L2 device.
    pub fn camera_config(&self) -> Result<CameraConfig> {
        self.camera.to_camera_config()
    }
}

/// User-friendly camera overrides that are merged on top of `CameraConfig::default()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraOptions {
    /// Override for the numeric camera index (e.g. `/dev/video2`).
    pub device_index: Option<usize>,
    /// Override for the camera name substring match.
    pub device_name: Option<String>,
    /// Override for desired frame width in pixels.
    pub width: Option<u32>,
    /// Override for desired frame height in pixels.
    pub height: Option<u32>,
    /// Override for desired frames per second.
    pub fps: Option<u32>,
    /// Override for pixel format string (mjpeg/yuyv/rgb24).
    pub format: Option<String>,
    /// Override for number of V4L2 buffers to allocate.
    pub buffer_count: Option<u32>,
}

impl Default for CameraOptions {
    fn default() -> Self {
        Self {
            device_index: None,
            device_name: None,
            width: None,
            height: None,
            fps: None,
            format: None,
            buffer_count: None,
        }
    }
}

impl CameraOptions {
    pub(crate) fn apply_env_overrides(&mut self) {
        if let Ok(name) = env::var("QLINK_CAMERA_DEVICE") {
            self.device_name = Some(name);
            self.device_index = None;
        }
        if let Ok(index) = env::var("QLINK_CAMERA_INDEX") {
            if let Ok(parsed) = index.parse::<usize>() {
                self.device_index = Some(parsed);
                self.device_name = None;
            }
        }
        if let Ok(width) = env::var("QLINK_CAMERA_WIDTH") {
            self.width = width.parse::<u32>().ok();
        }
        if let Ok(height) = env::var("QLINK_CAMERA_HEIGHT") {
            self.height = height.parse::<u32>().ok();
        }
        if let Ok(fps) = env::var("QLINK_CAMERA_FPS") {
            self.fps = fps.parse::<u32>().ok();
        }
        if let Ok(format) = env::var("QLINK_CAMERA_FORMAT") {
            self.format = Some(format);
        }
        if let Ok(buffers) = env::var("QLINK_CAMERA_BUFFERS") {
            self.buffer_count = buffers.parse::<u32>().ok();
        }
    }

    /// Merge overrides onto the default camera configuration.
    pub fn to_camera_config(&self) -> Result<CameraConfig> {
        let mut config = CameraConfig::default();

        if let Some(name) = &self.device_name {
            config.device_name = Some(name.clone());
            config.device_index = None;
        }

        if let Some(index) = self.device_index {
            config.device_index = Some(index);
            if self.device_name.is_none() {
                config.device_name = None;
            }
        }

        if let Some(width) = self.width {
            config.width = width;
        }

        if let Some(height) = self.height {
            config.height = height;
        }

        if let Some(fps) = self.fps {
            config.fps = fps.max(1);
        }

        if let Some(format) = &self.format {
            config.format = PixelFormat::from_str(format).ok_or_else(|| {
                Error::Config(format!(
                    "Unknown pixel format '{}'. Use mjpeg, yuyv, or rgb24",
                    format
                ))
            })?;
        }

        if let Some(buffers) = self.buffer_count {
            config.buffer_count = buffers.max(2);
        }

        Ok(config)
    }
}

/// Structured logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingOptions {
    /// Default log level (overridable via `QLINK_LOG_LEVEL`)
    pub level: String,
    /// Optional log file path for teeing structured logs
    pub file: Option<PathBuf>,
    /// Force ANSI colors in stdout logging
    pub color: bool,
    /// Enable periodic metrics summaries over tracing
    pub metrics: bool,
    /// Interval in seconds for emitting aggregated metrics when enabled
    pub metrics_interval_secs: u64,
    /// Optional log rotation strategy applied to `file`
    pub rotation: Option<LogRotation>,
    /// Optional bind address for exposing runtime metrics over HTTP (e.g., "127.0.0.1:9900")
    pub metrics_endpoint: Option<String>,
    /// Output format for the metrics endpoint (`json` or `prometheus`)
    pub metrics_format: MetricsFormat,
}

impl Default for LoggingOptions {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
            color: true,
            metrics: false,
            metrics_interval_secs: 60,
            rotation: None,
            metrics_endpoint: None,
            metrics_format: MetricsFormat::Json,
        }
    }
}

impl LoggingOptions {
    pub(crate) fn apply_env_overrides(&mut self) {
        if let Ok(level) = env::var("QLINK_LOG_LEVEL") {
            self.level = level;
        }
        if let Ok(file) = env::var("QLINK_LOG_FILE") {
            self.file = Some(PathBuf::from(file));
        }
        if let Ok(color) = env::var("QLINK_LOG_COLOR") {
            match color.to_ascii_lowercase().as_str() {
                "0" | "false" | "off" => self.color = false,
                "1" | "true" | "on" => self.color = true,
                _ => {}
            }
        }
        if let Ok(metrics) = env::var("QLINK_LOG_METRICS") {
            match metrics.to_ascii_lowercase().as_str() {
                "1" | "true" | "on" => self.metrics = true,
                "0" | "false" | "off" => self.metrics = false,
                _ => {}
            }
        }
        if let Ok(interval) = env::var("QLINK_LOG_METRICS_INTERVAL") {
            if let Ok(value) = interval.parse::<u64>() {
                self.metrics_interval_secs = value.max(5);
            }
        }
        if let Ok(rotation) = env::var("QLINK_LOG_ROTATION") {
            if let Some(parsed) = LogRotation::from_str(&rotation) {
                self.rotation = Some(parsed);
            }
        }
        if let Ok(endpoint) = env::var("QLINK_METRICS_ENDPOINT") {
            self.metrics_endpoint = Some(endpoint);
        }
        if let Ok(format) = env::var("QLINK_METRICS_FORMAT") {
            if let Ok(parsed) = format.parse::<MetricsFormat>() {
                self.metrics_format = parsed;
            }
        }
    }
}

/// Supported log rotation policies for file sinks
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogRotation {
    /// Rotate log files once per hour
    Hourly,
    /// Rotate log files once per day
    Daily,
}

impl LogRotation {
    fn from_str(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "hourly" => Some(Self::Hourly),
            "daily" => Some(Self::Daily),
            _ => None,
        }
    }
}

/// Supported serialization formats for the metrics endpoint
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MetricsFormat {
    /// Emit metrics as structured JSON
    Json,
    /// Emit metrics in Prometheus text exposition format
    Prometheus,
}

impl MetricsFormat {
    /// Parse a metrics format identifier (case-insensitive) from a string slice.
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "json" => Some(Self::Json),
            "prometheus" => Some(Self::Prometheus),
            _ => None,
        }
    }
}

impl FromStr for MetricsFormat {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        Self::parse(value).ok_or_else(|| {
            format!("Unsupported metrics format '{value}', expected 'json' or 'prometheus'")
        })
    }
}

/// Local API binding configuration for the forthcoming daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ApiOptions {
    /// Bind address for the local API server
    pub bind_address: String,
    /// Bind port for the local API server
    pub port: u16,
    /// Optional shared-secret token for API access
    pub token: Option<String>,
    /// List of allowed browser origins for CORS enforcement
    pub allowed_origins: Vec<String>,
    /// Optional Unix domain socket path for streaming structured events
    pub unix_socket: Option<PathBuf>,
}

impl Default for ApiOptions {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 9233,
            token: None,
            allowed_origins: Vec::new(),
            unix_socket: None,
        }
    }
}

impl ApiOptions {
    pub(crate) fn apply_env_overrides(&mut self) {
        if let Ok(addr) = env::var("QLINK_BIND_ADDRESS") {
            self.bind_address = addr;
        }
        if let Ok(port) = env::var("QLINK_BIND_PORT") {
            if let Ok(parsed) = port.parse::<u16>() {
                self.port = parsed;
            }
        }
        if let Ok(token) = env::var("QLINK_API_TOKEN") {
            self.token = Some(token);
        }
        if let Ok(origins) = env::var("QLINK_ALLOWED_ORIGINS") {
            self.allowed_origins = origins
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if let Ok(socket) = env::var("QLINK_UNIX_SOCKET") {
            if socket.trim().is_empty() {
                self.unix_socket = None;
            } else {
                self.unix_socket = Some(PathBuf::from(socket));
            }
        }
    }

    /// Socket address helper for binding servers
    pub fn socket_address(&self) -> String {
        format!("{}:{}", self.bind_address, self.port)
    }
}

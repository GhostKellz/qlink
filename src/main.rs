//! QLINK daemon entrypoint

#[cfg(not(feature = "camera"))]
compile_error!("qlinkd requires the `camera` feature");

use clap::Parser;
use qlink::config::MetricsFormat;
#[cfg(target_family = "unix")]
use qlink::output::unix::UnixBroadcast;
use qlink::output::{RenderedKeystone, render_keystone_payload};
use qlink::{
    Error, KeystonePayload, QlinkConfig, QlinkScanner, Result, ScanConfig, camera, logging, metrics,
};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

#[cfg(feature = "simulator")]
use qlink::simulator;

#[derive(Parser, Debug)]
#[command(
    name = "qlinkd",
    version,
    about = "Linux-first air-gapped QR bridge daemon"
)]
struct Cli {
    /// Optional configuration file (toml/yaml). Defaults to qlink.{toml,yaml} in cwd/XDG config.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Override camera by name (takes precedence over config file)
    #[arg(long, value_name = "NAME")]
    device: Option<String>,

    /// Override camera by index (/dev/videoN)
    #[arg(long, value_name = "INDEX")]
    device_index: Option<usize>,

    /// Perform a single frame capture and print raw QR contents
    #[arg(long)]
    scan_once: bool,

    /// Output results as formatted JSON instead of human-readable text
    #[arg(long)]
    json: bool,

    /// Continuously watch for Keystone payloads and print each as it is decoded
    #[arg(long)]
    watch: bool,

    /// Enable metrics output regardless of configuration file settings
    #[arg(long)]
    metrics: bool,

    /// Override metrics endpoint bind address (e.g. 127.0.0.1:9900)
    #[arg(long, value_name = "ADDR")]
    metrics_bind: Option<String>,

    /// Override metrics endpoint format (`json` or `prometheus`)
    #[arg(long, value_name = "FORMAT")]
    metrics_format: Option<String>,

    /// Publish structured events over the specified Unix domain socket path
    #[arg(long, value_name = "PATH")]
    unix_socket: Option<PathBuf>,

    /// List detected cameras and exit
    #[arg(long)]
    list_cameras: bool,

    /// Replay prerecorded UR fragments instead of using a live camera (simulator feature)
    #[cfg(feature = "simulator")]
    #[arg(long, value_name = "PATH")]
    simulator: Option<PathBuf>,
}

struct OutputSinks {
    json: bool,
    #[cfg(target_family = "unix")]
    unix: Option<Arc<UnixBroadcast>>,
}

impl OutputSinks {
    fn new(json: bool, #[cfg(target_family = "unix")] unix: Option<Arc<UnixBroadcast>>) -> Self {
        Self {
            json,
            #[cfg(target_family = "unix")]
            unix,
        }
    }

    fn json(&self) -> bool {
        self.json
    }

    fn emit_keystone(&self, rendered: &RenderedKeystone) -> Result<()> {
        if self.json {
            println!("{}", serde_json::to_string_pretty(&rendered.json)?);
        } else {
            for line in &rendered.human {
                println!("{line}");
            }
        }
        self.send_unix_value(&rendered.json)?;
        Ok(())
    }

    fn send_unix_value(&self, value: &Value) -> Result<()> {
        #[cfg(target_family = "unix")]
        if let Some(unix) = &self.unix {
            unix.send_value(value)?;
        }

        #[cfg(not(target_family = "unix"))]
        let _ = value;

        Ok(())
    }

    fn emit_error(&self, message: &str) -> Result<()> {
        if self.json {
            let payload = json!({ "error": message });
            println!("{}", serde_json::to_string_pretty(&payload)?);
            self.send_unix_value(&payload)?;
        } else {
            println!("Failed to decode Keystone message: {message}");
            #[cfg(target_family = "unix")]
            if let Some(unix) = &self.unix {
                unix.send_error(message)?;
            }
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.list_cameras {
        list_cameras()?;
        return Ok(());
    }

    let mut config = QlinkConfig::load(cli.config.as_deref())?;

    if let Some(ref name) = cli.device {
        config.camera.device_name = Some(name.clone());
        config.camera.device_index = None;
    }

    if let Some(index) = cli.device_index {
        config.camera.device_index = Some(index);
        config.camera.device_name = None;
    }

    if cli.metrics {
        config.logging.metrics = true;
    }

    if let Some(ref bind) = cli.metrics_bind {
        config.logging.metrics_endpoint = Some(bind.clone());
        config.logging.metrics = true;
    }

    if let Some(ref format) = cli.metrics_format {
        config.logging.metrics_format = format.parse::<MetricsFormat>().map_err(Error::Config)?;
    }

    logging::init(&config.logging)?;

    let metrics_enabled = config.logging.metrics || config.logging.metrics_endpoint.is_some();
    if metrics_enabled {
        metrics::enable(config.logging.metrics_interval_secs);
        if let Some(ref endpoint) = config.logging.metrics_endpoint {
            let addr = SocketAddr::from_str(endpoint).map_err(|e| {
                Error::Config(format!("Invalid metrics endpoint '{endpoint}': {e}"))
            })?;
            metrics::spawn_http_endpoint(addr, config.logging.metrics_format)?;
        }
    }

    #[cfg(target_family = "unix")]
    let unix_socket_path = cli
        .unix_socket
        .clone()
        .or_else(|| config.api.unix_socket.clone());

    #[cfg(target_family = "unix")]
    let unix_broadcast = if let Some(path) = unix_socket_path {
        Some(UnixBroadcast::bind(&path).await?)
    } else {
        None
    };

    #[cfg(target_family = "unix")]
    let sinks = OutputSinks::new(cli.json, unix_broadcast);

    #[cfg(not(target_family = "unix"))]
    let sinks = OutputSinks::new(cli.json);

    #[cfg(feature = "simulator")]
    if let Some(path) = cli.simulator.clone() {
        simulator::run(&path, cli.watch, &sinks).await?;
        return Ok(());
    }

    let camera_config = config.camera_config()?;
    info!(?camera_config, "Starting QLINK scanner");

    let scan_config = ScanConfig { camera_config };
    let mut scanner = QlinkScanner::new(scan_config).await?;

    if cli.scan_once {
        handle_scan_once(&mut scanner, &sinks).await
    } else {
        handle_keystone_scan(&mut scanner, &sinks, cli.watch).await
    }
}

fn list_cameras() -> Result<()> {
    match camera::list_devices() {
        Ok(devices) => {
            if devices.is_empty() {
                println!("No V4L2 cameras detected");
            } else {
                println!("Discovered cameras:");
                for dev in devices {
                    println!("  [{}] {} ({})", dev.index, dev.name, dev.path);
                }
            }
            Ok(())
        }
        Err(err) => Err(err),
    }
}

async fn handle_scan_once(scanner: &mut QlinkScanner, sinks: &OutputSinks) -> Result<()> {
    let started = Instant::now();
    let qr = scanner.scan_once().await?;

    if sinks.json() {
        let mut root = json!({
            "qr": {
                "text": qr.as_str(),
                "bytes_hex": hex::encode(qr.as_bytes()),
                "byte_length": qr.as_bytes().len(),
            }
        });

        match KeystonePayload::try_from(qr.clone()) {
            Ok(payload) => {
                metrics::record(started.elapsed(), true, Some(&payload.ur_type));
                let rendered = render_keystone_payload(&payload);
                if let Some(obj) = root.as_object_mut() {
                    obj.insert("keystone".to_string(), rendered.json.clone());
                }
                sinks.send_unix_value(&rendered.json)?;
            }
            Err(Error::InvalidKeystonePayload(_)) | Err(Error::UrParse(_)) => {
                if let Some(obj) = root.as_object_mut() {
                    obj.insert("keystone".to_string(), Value::Null);
                }
            }
            Err(err) => return Err(err),
        }

        println!("{}", serde_json::to_string_pretty(&root)?);
        return Ok(());
    }

    if let Some(text) = qr.as_str() {
        println!("QR text: {text}");
    } else {
        println!("QR binary payload ({} bytes)", qr.as_bytes().len());
    }

    match KeystonePayload::try_from(qr.clone()) {
        Ok(payload) => {
            metrics::record(started.elapsed(), true, Some(&payload.ur_type));
            println!();
            let rendered = render_keystone_payload(&payload);
            sinks.emit_keystone(&rendered)?;
        }
        Err(Error::InvalidKeystonePayload(_)) | Err(Error::UrParse(_)) => {
            println!("No Keystone payload detected in frame");
        }
        Err(err) => return Err(err),
    }

    Ok(())
}

async fn handle_keystone_scan(
    scanner: &mut QlinkScanner,
    sinks: &OutputSinks,
    watch: bool,
) -> Result<()> {
    println!("Waiting for Keystone QR sequence...");

    let mut backpressure: u64 = 0;
    let mut last_started: Option<Instant> = None;

    loop {
        let started = Instant::now();
        if let Some(previous) = last_started {
            metrics::record_frame_interval(started.saturating_duration_since(previous));
        }
        last_started = Some(started);

        match scanner.scan_keystone().await {
            Ok(payload) => {
                metrics::record(started.elapsed(), true, Some(&payload.ur_type));
                backpressure = 0;
                metrics::record_backpressure(backpressure);
                let rendered = render_keystone_payload(&payload);
                sinks.emit_keystone(&rendered)?;
            }
            Err(err) => {
                metrics::record(started.elapsed(), false, None);
                backpressure = backpressure.saturating_add(1);
                metrics::record_backpressure(backpressure);
                let message = err.to_string();
                sinks.emit_error(&message)?;

                if !watch {
                    return Err(err);
                }
            }
        }

        if !watch {
            break;
        }
    }

    Ok(())
}

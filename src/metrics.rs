//! Lightweight runtime metrics aggregation for the qlink daemon

use crate::config::MetricsFormat;
use crate::error::{Error, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{self, MissedTickBehavior};
use tracing::{info, warn};

static METRICS: OnceLock<Arc<MetricsInner>> = OnceLock::new();
static LAST_SNAPSHOT: OnceLock<Mutex<Option<Snapshot>>> = OnceLock::new();

/// Enable periodic metrics emission with the provided interval in seconds.
pub fn enable(interval_secs: u64) {
    let interval = interval_secs.max(5);
    let inner = Arc::clone(METRICS.get_or_init(|| Arc::new(MetricsInner::new(interval))));
    inner.update_interval(interval);
    inner.ensure_task();
}

/// Record the outcome of a scan attempt for aggregation.
pub fn record(duration: Duration, success: bool, ur_type: Option<&str>) {
    if let Some(inner) = METRICS.get() {
        inner.record(duration, success, ur_type);
    }
}

/// Record the observed interval between successive scan attempts.
pub fn record_frame_interval(interval: Duration) {
    if let Some(inner) = METRICS.get() {
        inner.record_frame_interval(interval);
    }
}

/// Register the current backpressure level (e.g., consecutive failures) of the watch loop.
pub fn record_backpressure(level: u64) {
    if let Some(inner) = METRICS.get() {
        inner.record_backpressure(level);
    }
}

/// Spawn a lightweight HTTP endpoint that exposes the latest metrics snapshot.
pub fn spawn_http_endpoint(addr: SocketAddr, format: MetricsFormat) -> Result<()> {
    let std_listener = std::net::TcpListener::bind(addr).map_err(Error::Io)?;
    std_listener.set_nonblocking(true).map_err(Error::Io)?;
    let listener = TcpListener::from_std(std_listener).map_err(Error::Io)?;

    tokio::spawn(async move {
        if let Err(err) = run_http_listener(listener, format).await {
            tracing::error!(target: "qlink::metrics", error = %err, "metrics endpoint error");
        }
    });

    Ok(())
}

struct MetricsInner {
    state: Mutex<MetricsState>,
    interval_secs: AtomicU64,
    task_spawned: AtomicBool,
}

impl MetricsInner {
    fn new(interval_secs: u64) -> Self {
        Self {
            state: Mutex::new(MetricsState::new()),
            interval_secs: AtomicU64::new(interval_secs.max(5)),
            task_spawned: AtomicBool::new(false),
        }
    }

    fn update_interval(&self, interval_secs: u64) {
        self.interval_secs
            .store(interval_secs.max(5), Ordering::Relaxed);
    }

    fn ensure_task(self: &Arc<Self>) {
        if self
            .task_spawned
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let runner = Arc::clone(self);
            tokio::spawn(async move {
                runner.run().await;
            });
        }
    }

    fn record(&self, duration: Duration, success: bool, ur_type: Option<&str>) {
        let mut state = self.state.lock().expect("metrics mutex poisoned");
        state.total_scans += 1;
        if success {
            state.successes += 1;
            state.success_duration += duration;
        } else {
            state.failures += 1;
        }

        if let Some(kind) = ur_type {
            let entry = state
                .per_type
                .entry(kind.to_string())
                .or_insert_with(TypeCounters::default);
            if success {
                entry.successes += 1;
                entry.success_duration += duration;
            } else {
                entry.failures += 1;
            }
        }
    }

    fn record_frame_interval(&self, interval: Duration) {
        let mut state = self.state.lock().expect("metrics mutex poisoned");
        state.frame_interval_total += interval;
        state.frame_interval_samples += 1;
        if interval > state.frame_interval_max {
            state.frame_interval_max = interval;
        }
        state.last_frame_interval = Some(interval);
    }

    fn record_backpressure(&self, level: u64) {
        let mut state = self.state.lock().expect("metrics mutex poisoned");
        state.backpressure_level = level;
        if level > state.backpressure_peak {
            state.backpressure_peak = level;
        }
    }

    fn snapshot_current(&self) -> Snapshot {
        let state = self.state.lock().expect("metrics mutex poisoned");
        state.clone_snapshot()
    }

    async fn run(self: Arc<Self>) {
        let mut current_secs = self.interval_secs.load(Ordering::Relaxed).max(5);
        loop {
            let mut ticker = time::interval(Duration::from_secs(current_secs));
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            // Align the ticker so the first report happens after a full interval
            ticker.tick().await;

            loop {
                ticker.tick().await;
                let snapshot = self.snapshot_and_reset();
                store_snapshot(&snapshot);
                log_snapshot(&snapshot);

                let next_secs = self.interval_secs.load(Ordering::Relaxed).max(5);
                if next_secs != current_secs {
                    current_secs = next_secs;
                    break;
                }
            }
        }
    }

    fn snapshot_and_reset(&self) -> Snapshot {
        let mut state = self.state.lock().expect("metrics mutex poisoned");
        state.snapshot_and_reset()
    }
}

struct MetricsState {
    total_scans: u64,
    successes: u64,
    failures: u64,
    success_duration: Duration,
    per_type: HashMap<String, TypeCounters>,
    last_reset: Instant,
    frame_interval_total: Duration,
    frame_interval_samples: u64,
    frame_interval_max: Duration,
    last_frame_interval: Option<Duration>,
    backpressure_level: u64,
    backpressure_peak: u64,
}

impl MetricsState {
    fn new() -> Self {
        Self {
            total_scans: 0,
            successes: 0,
            failures: 0,
            success_duration: Duration::ZERO,
            per_type: HashMap::new(),
            last_reset: Instant::now(),
            frame_interval_total: Duration::ZERO,
            frame_interval_samples: 0,
            frame_interval_max: Duration::ZERO,
            last_frame_interval: None,
            backpressure_level: 0,
            backpressure_peak: 0,
        }
    }

    fn snapshot_and_reset(&mut self) -> Snapshot {
        let elapsed = self.last_reset.elapsed();
        let per_type = self
            .per_type
            .drain()
            .map(|(ur_type, counters)| TypeSnapshot {
                ur_type,
                successes: counters.successes,
                failures: counters.failures,
                success_duration: counters.success_duration,
            })
            .collect();

        let frame_interval_avg = if self.frame_interval_samples > 0 {
            let divisor = if self.frame_interval_samples > u32::MAX as u64 {
                u32::MAX
            } else {
                self.frame_interval_samples as u32
            };
            self.frame_interval_total.checked_div(divisor)
        } else {
            None
        };

        let frame_interval_max = if self.frame_interval_samples > 0 {
            Some(self.frame_interval_max)
        } else {
            None
        };

        let snapshot = Snapshot {
            total_scans: self.total_scans,
            successes: self.successes,
            failures: self.failures,
            success_duration: self.success_duration,
            elapsed,
            per_type,
            frame_interval_avg,
            frame_interval_max,
            last_frame_interval: self.last_frame_interval,
            backpressure_level: self.backpressure_level,
            backpressure_peak: self.backpressure_peak,
        };

        self.total_scans = 0;
        self.successes = 0;
        self.failures = 0;
        self.success_duration = Duration::ZERO;
        self.last_reset = Instant::now();
        self.frame_interval_total = Duration::ZERO;
        self.frame_interval_samples = 0;
        self.frame_interval_max = Duration::ZERO;
        self.backpressure_peak = self.backpressure_level;

        snapshot
    }

    fn clone_snapshot(&self) -> Snapshot {
        let per_type = self
            .per_type
            .iter()
            .map(|(ur_type, counters)| TypeSnapshot {
                ur_type: ur_type.clone(),
                successes: counters.successes,
                failures: counters.failures,
                success_duration: counters.success_duration,
            })
            .collect();

        let frame_interval_avg = if self.frame_interval_samples > 0 {
            let divisor = if self.frame_interval_samples > u32::MAX as u64 {
                u32::MAX
            } else {
                self.frame_interval_samples as u32
            };
            self.frame_interval_total.checked_div(divisor)
        } else {
            None
        };

        let frame_interval_max = if self.frame_interval_samples > 0 {
            Some(self.frame_interval_max)
        } else {
            None
        };

        Snapshot {
            total_scans: self.total_scans,
            successes: self.successes,
            failures: self.failures,
            success_duration: self.success_duration,
            elapsed: self.last_reset.elapsed(),
            per_type,
            frame_interval_avg,
            frame_interval_max,
            last_frame_interval: self.last_frame_interval,
            backpressure_level: self.backpressure_level,
            backpressure_peak: self.backpressure_peak,
        }
    }
}

#[derive(Default)]
struct TypeCounters {
    successes: u64,
    failures: u64,
    success_duration: Duration,
}

#[derive(Clone)]
struct Snapshot {
    total_scans: u64,
    successes: u64,
    failures: u64,
    success_duration: Duration,
    elapsed: Duration,
    per_type: Vec<TypeSnapshot>,
    frame_interval_avg: Option<Duration>,
    frame_interval_max: Option<Duration>,
    last_frame_interval: Option<Duration>,
    backpressure_level: u64,
    backpressure_peak: u64,
}

#[derive(Clone)]
struct TypeSnapshot {
    ur_type: String,
    successes: u64,
    failures: u64,
    success_duration: Duration,
}

impl TypeSnapshot {
    fn avg_latency_ms(&self) -> f64 {
        if self.successes == 0 {
            0.0
        } else {
            self.success_duration.as_secs_f64() * 1_000.0 / self.successes as f64
        }
    }
}

fn log_snapshot(snapshot: &Snapshot) {
    let avg_ms = if snapshot.successes == 0 {
        0.0
    } else {
        snapshot.success_duration.as_secs_f64() * 1_000.0 / snapshot.successes as f64
    };

    let success_rate = if snapshot.total_scans == 0 {
        0.0
    } else {
        (snapshot.successes as f64 / snapshot.total_scans as f64) * 100.0
    };

    let frame_avg_ms = snapshot
        .frame_interval_avg
        .map(|d| d.as_secs_f64() * 1_000.0);
    let frame_max_ms = snapshot
        .frame_interval_max
        .map(|d| d.as_secs_f64() * 1_000.0);
    let frame_last_ms = snapshot
        .last_frame_interval
        .map(|d| d.as_secs_f64() * 1_000.0);

    info!(
        target: "qlink::metrics",
        interval_secs = snapshot.elapsed.as_secs(),
        total_scans = snapshot.total_scans,
        success_count = snapshot.successes,
        failure_count = snapshot.failures,
        avg_latency_ms = avg_ms,
        success_rate = format_args!("{success_rate:.1}%"),
        frame_interval_avg_ms = frame_avg_ms,
        frame_interval_max_ms = frame_max_ms,
        frame_interval_last_ms = frame_last_ms,
        backpressure_level = snapshot.backpressure_level,
        backpressure_peak = snapshot.backpressure_peak,
        "Scan metrics window"
    );

    if !snapshot.per_type.is_empty() {
        let breakdown = format_breakdown(&snapshot.per_type);
        info!(
            target: "qlink::metrics",
            breakdown,
            "Per-type metrics"
        );
    }
}

fn format_breakdown(entries: &[TypeSnapshot]) -> String {
    entries
        .iter()
        .map(|entry| {
            let avg_ms = entry.avg_latency_ms();
            if entry.failures > 0 {
                format!(
                    "{}: {} ok / {} err (avg {:.1} ms)",
                    entry.ur_type, entry.successes, entry.failures, avg_ms
                )
            } else {
                format!(
                    "{}: {} ok (avg {:.1} ms)",
                    entry.ur_type, entry.successes, avg_ms
                )
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn store_snapshot(snapshot: &Snapshot) {
    let lock = LAST_SNAPSHOT.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = lock.lock() {
        *guard = Some(snapshot.clone());
    }
}

fn latest_snapshot() -> Option<Snapshot> {
    let lock = LAST_SNAPSHOT.get_or_init(|| Mutex::new(None));
    match lock.lock() {
        Ok(guard) => (*guard).clone(),
        Err(_) => None,
    }
}

fn snapshot_fallback() -> Option<Snapshot> {
    METRICS.get().map(|inner| inner.snapshot_current())
}

async fn run_http_listener(listener: TcpListener, format: MetricsFormat) -> Result<()> {
    loop {
        let (stream, addr) = match listener.accept().await {
            Ok(pair) => pair,
            Err(err) => {
                warn!(target: "qlink::metrics", error = %err, "metrics accept failed");
                time::sleep(Duration::from_millis(250)).await;
                continue;
            }
        };

        let peer = addr;
        tokio::spawn(async move {
            if let Err(err) = handle_connection(stream, format).await {
                tracing::debug!(target: "qlink::metrics", peer = %peer, error = %err, "metrics connection closed");
            }
        });
    }
}

async fn handle_connection(mut stream: TcpStream, format: MetricsFormat) -> Result<()> {
    let mut buffer = [0u8; 1024];
    let _ = stream.read(&mut buffer).await.map_err(Error::Io)?;

    let snapshot = latest_snapshot().or_else(|| {
        let fresh = snapshot_fallback();
        if let Some(ref snapshot) = fresh {
            store_snapshot(snapshot);
        }
        fresh
    });

    let (status_line, content_type, body) = match snapshot {
        Some(snapshot) => match format {
            MetricsFormat::Json => {
                let payload = snapshot_to_http(&snapshot);
                let body = serde_json::to_vec(&payload)?;
                ("HTTP/1.1 200 OK\r\n", Some("application/json"), body)
            }
            MetricsFormat::Prometheus => {
                let body = render_prometheus(&snapshot).into_bytes();
                (
                    "HTTP/1.1 200 OK\r\n",
                    Some("text/plain; version=0.0.4"),
                    body,
                )
            }
        },
        None => ("HTTP/1.1 204 No Content\r\n", None, Vec::new()),
    };

    let mut response = Vec::with_capacity(128 + body.len());
    response.extend_from_slice(status_line.as_bytes());
    response.extend_from_slice(b"Connection: close\r\n");
    response.extend_from_slice(b"Cache-Control: no-store\r\n");
    if let Some(content_type) = content_type {
        response.extend_from_slice(b"Content-Type: ");
        response.extend_from_slice(content_type.as_bytes());
        response.extend_from_slice(b"\r\n");
    }
    let length_header = format!("Content-Length: {}\r\n\r\n", body.len());
    response.extend_from_slice(length_header.as_bytes());
    response.extend_from_slice(&body);

    stream.write_all(&response).await.map_err(Error::Io)?;
    stream.shutdown().await.map_err(Error::Io)?;

    Ok(())
}

#[derive(Serialize)]
struct HttpMetrics {
    window_secs: u64,
    total_scans: u64,
    successes: u64,
    failures: u64,
    success_rate: f64,
    avg_latency_ms: f64,
    frame_intervals: Option<FrameIntervalMetrics>,
    backpressure: BackpressureMetrics,
    per_type: Vec<HttpTypeMetrics>,
}

#[derive(Serialize)]
struct FrameIntervalMetrics {
    avg_ms: f64,
    max_ms: f64,
    last_ms: f64,
}

#[derive(Serialize)]
struct BackpressureMetrics {
    current: u64,
    peak: u64,
}

#[derive(Serialize)]
struct HttpTypeMetrics {
    ur_type: String,
    successes: u64,
    failures: u64,
    avg_latency_ms: f64,
}

fn snapshot_to_http(snapshot: &Snapshot) -> HttpMetrics {
    let avg_latency_ms = if snapshot.successes == 0 {
        0.0
    } else {
        snapshot.success_duration.as_secs_f64() * 1_000.0 / snapshot.successes as f64
    };

    let success_rate = if snapshot.total_scans == 0 {
        0.0
    } else {
        snapshot.successes as f64 * 100.0 / snapshot.total_scans as f64
    };

    let frame_intervals = snapshot.frame_interval_avg.map(|avg| FrameIntervalMetrics {
        avg_ms: avg.as_secs_f64() * 1_000.0,
        max_ms: snapshot.frame_interval_max.unwrap_or(avg).as_secs_f64() * 1_000.0,
        last_ms: snapshot.last_frame_interval.unwrap_or(avg).as_secs_f64() * 1_000.0,
    });

    let per_type = snapshot
        .per_type
        .iter()
        .map(|entry| HttpTypeMetrics {
            ur_type: entry.ur_type.clone(),
            successes: entry.successes,
            failures: entry.failures,
            avg_latency_ms: entry.avg_latency_ms(),
        })
        .collect();

    HttpMetrics {
        window_secs: snapshot.elapsed.as_secs(),
        total_scans: snapshot.total_scans,
        successes: snapshot.successes,
        failures: snapshot.failures,
        success_rate,
        avg_latency_ms,
        frame_intervals,
        backpressure: BackpressureMetrics {
            current: snapshot.backpressure_level,
            peak: snapshot.backpressure_peak,
        },
        per_type,
    }
}

fn render_prometheus(snapshot: &Snapshot) -> String {
    let mut output = String::new();
    let avg_latency_seconds = if snapshot.successes == 0 {
        0.0
    } else {
        snapshot.success_duration.as_secs_f64() / snapshot.successes as f64
    };

    let success_rate = if snapshot.total_scans == 0 {
        0.0
    } else {
        snapshot.successes as f64 / snapshot.total_scans as f64
    };

    let _ = writeln!(
        &mut output,
        "# HELP qlink_window_seconds Duration of the aggregation window in seconds"
    );
    let _ = writeln!(&mut output, "# TYPE qlink_window_seconds gauge");
    let _ = writeln!(
        &mut output,
        "qlink_window_seconds {}",
        snapshot.elapsed.as_secs()
    );

    let _ = writeln!(
        &mut output,
        "# HELP qlink_scans_total Total scans observed during the window"
    );
    let _ = writeln!(&mut output, "# TYPE qlink_scans_total counter");
    let _ = writeln!(&mut output, "qlink_scans_total {}", snapshot.total_scans);

    let _ = writeln!(
        &mut output,
        "# HELP qlink_scan_successes Successful scans in the window"
    );
    let _ = writeln!(&mut output, "# TYPE qlink_scan_successes counter");
    let _ = writeln!(&mut output, "qlink_scan_successes {}", snapshot.successes);

    let _ = writeln!(
        &mut output,
        "# HELP qlink_scan_failures Failed scans in the window"
    );
    let _ = writeln!(&mut output, "# TYPE qlink_scan_failures counter");
    let _ = writeln!(&mut output, "qlink_scan_failures {}", snapshot.failures);

    let _ = writeln!(
        &mut output,
        "# HELP qlink_scan_success_rate Success ratio for the window"
    );
    let _ = writeln!(&mut output, "# TYPE qlink_scan_success_rate gauge");
    let _ = writeln!(&mut output, "qlink_scan_success_rate {:.6}", success_rate);

    let _ = writeln!(
        &mut output,
        "# HELP qlink_scan_latency_avg_seconds Average scan latency for successful scans"
    );
    let _ = writeln!(&mut output, "# TYPE qlink_scan_latency_avg_seconds gauge");
    let _ = writeln!(
        &mut output,
        "qlink_scan_latency_avg_seconds {:.6}",
        avg_latency_seconds
    );

    if let Some(avg) = snapshot.frame_interval_avg {
        let _ = writeln!(
            &mut output,
            "# HELP qlink_frame_interval_seconds Frame interval statistics"
        );
        let _ = writeln!(&mut output, "# TYPE qlink_frame_interval_seconds gauge");
        let _ = writeln!(
            &mut output,
            "qlink_frame_interval_seconds{{stat=\"avg\"}} {:.6}",
            avg.as_secs_f64()
        );
        if let Some(max) = snapshot.frame_interval_max {
            let _ = writeln!(
                &mut output,
                "qlink_frame_interval_seconds{{stat=\"max\"}} {:.6}",
                max.as_secs_f64()
            );
        }
        if let Some(last) = snapshot.last_frame_interval {
            let _ = writeln!(
                &mut output,
                "qlink_frame_interval_seconds{{stat=\"last\"}} {:.6}",
                last.as_secs_f64()
            );
        }
    }

    let _ = writeln!(
        &mut output,
        "# HELP qlink_backpressure_level Current backpressure level"
    );
    let _ = writeln!(&mut output, "# TYPE qlink_backpressure_level gauge");
    let _ = writeln!(
        &mut output,
        "qlink_backpressure_level {}",
        snapshot.backpressure_level
    );

    let _ = writeln!(
        &mut output,
        "# HELP qlink_backpressure_peak Historical peak backpressure within the window"
    );
    let _ = writeln!(&mut output, "# TYPE qlink_backpressure_peak gauge");
    let _ = writeln!(
        &mut output,
        "qlink_backpressure_peak {}",
        snapshot.backpressure_peak
    );

    if !snapshot.per_type.is_empty() {
        let _ = writeln!(
            &mut output,
            "# HELP qlink_scans_by_type_total Total scans by UR type"
        );
        let _ = writeln!(&mut output, "# TYPE qlink_scans_by_type_total counter");
        for entry in &snapshot.per_type {
            let label = escape_label(&entry.ur_type);
            let _ = writeln!(
                &mut output,
                "qlink_scans_by_type_total{{ur_type=\"{}\",result=\"success\"}} {}",
                label, entry.successes
            );
            let _ = writeln!(
                &mut output,
                "qlink_scans_by_type_total{{ur_type=\"{}\",result=\"failure\"}} {}",
                label, entry.failures
            );
            let _ = writeln!(
                &mut output,
                "qlink_scan_latency_avg_seconds_by_type{{ur_type=\"{}\"}} {:.6}",
                label,
                entry.avg_latency_ms() / 1_000.0
            );
        }
    }

    output
}

fn escape_label(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('"', "\\\"")
}

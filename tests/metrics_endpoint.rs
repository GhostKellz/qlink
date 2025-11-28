use std::io::Result as IoResult;
use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use qlink::config::MetricsFormat;
use qlink::metrics;

fn next_free_port() -> IoResult<SocketAddr> {
    let listener = StdTcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    drop(listener);
    Ok(addr)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_endpoint_serves_json_metrics() {
    metrics::enable(10);
    metrics::record(Duration::from_millis(125), true, Some("eth-sign"));
    metrics::record_frame_interval(Duration::from_millis(480));
    metrics::record_backpressure(2);

    let addr = next_free_port().expect("allocate port");
    metrics::spawn_http_endpoint(addr, MetricsFormat::Json).expect("spawn json endpoint");

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut stream = TcpStream::connect(addr).await.expect("connect json");
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .expect("write request");

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read response");

    let response = String::from_utf8_lossy(&buf);
    assert!(
        response.starts_with("HTTP/1.1 200"),
        "unexpected status: {response}"
    );

    let split: Vec<&str> = response.splitn(2, "\r\n\r\n").collect();
    assert_eq!(split.len(), 2, "invalid HTTP response format");
    let body = split[1];
    let payload: Value = serde_json::from_str(body).expect("parse json metrics");

    let total_scans = payload["total_scans"].as_u64().unwrap_or_default();
    assert!(total_scans >= 1, "expected at least one recorded scan");

    let successes = payload["successes"].as_u64().unwrap_or_default();
    assert!(successes >= 1, "expected at least one successful scan");

    let backpressure = payload["backpressure"]["current"]
        .as_u64()
        .unwrap_or_default();
    assert!(backpressure >= 2, "expected backpressure level to be set");

    let per_type = payload["per_type"].as_array().expect("per_type array");
    assert!(
        per_type.iter().any(|entry| entry["ur_type"] == "eth-sign"),
        "expected eth-sign type metrics"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_endpoint_serves_prometheus_metrics() {
    metrics::enable(10);
    // Ensure we accumulate a second record so the exported counters grow.
    metrics::record(Duration::from_millis(220), false, None);
    metrics::record_backpressure(3);

    let addr = next_free_port().expect("allocate port");
    metrics::spawn_http_endpoint(addr, MetricsFormat::Prometheus).expect("spawn prom endpoint");

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut stream = TcpStream::connect(addr).await.expect("connect prom");
    stream
        .write_all(b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .expect("write request");

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read response");

    let response = String::from_utf8_lossy(&buf);
    assert!(
        response.starts_with("HTTP/1.1 200"),
        "unexpected status: {response}"
    );
    assert!(
        response.contains("qlink_scans_total"),
        "missing prometheus counters"
    );
    assert!(
        response.contains("qlink_backpressure_level"),
        "missing backpressure gauge"
    );
}

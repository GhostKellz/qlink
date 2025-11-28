//! Unix domain socket broadcast for structured scan events

#![cfg(target_family = "unix")]

use crate::error::{Error, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

const CHANNEL_CAPACITY: usize = 128;

/// Broadcasts structured events to all connected Unix domain socket clients.
pub struct UnixBroadcast {
    sender: broadcast::Sender<Arc<String>>,
    _accept_task: JoinHandle<()>,
    socket_path: PathBuf,
}

impl UnixBroadcast {
    /// Bind to the provided Unix domain socket path and spawn the accept loop.
    pub async fn bind(path: &Path) -> Result<Arc<Self>> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| {
                    Error::Io(std::io::Error::new(
                        e.kind(),
                        format!("Failed to create directory {}: {e}", parent.display()),
                    ))
                })?;
            }
        }

        if path.exists() {
            tokio::fs::remove_file(path).await.map_err(|e| {
                Error::Io(std::io::Error::new(
                    e.kind(),
                    format!("Failed to remove existing socket {}: {e}", path.display()),
                ))
            })?;
        }

        let listener = UnixListener::bind(path).map_err(|e| {
            Error::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to bind Unix socket {}: {e}", path.display()),
            ))
        })?;

        let (sender, _) = broadcast::channel::<Arc<String>>(CHANNEL_CAPACITY);
        let sender_clone = sender.clone();
        let path_buf = path.to_path_buf();

        let accept_task = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _addr)) => {
                        let rx = sender_clone.subscribe();
                        tokio::spawn(handle_client(stream, rx));
                    }
                    Err(err) => {
                        tracing::warn!("Unix socket accept error: {err}");
                    }
                }
            }
        });

        Ok(Arc::new(Self {
            sender,
            _accept_task: accept_task,
            socket_path: path_buf,
        }))
    }

    /// Broadcast a JSON value to all connected listeners.
    pub fn send_value(&self, value: &Value) -> Result<()> {
        let payload = serde_json::to_string(value)?;
        self.send_raw(payload)
    }

    /// Broadcast a pre-serialized JSON string to all connected listeners.
    pub fn send_raw(&self, payload: String) -> Result<()> {
        self.sender
            .send(Arc::new(payload))
            .map(|_| ())
            .map_err(|err| {
                Error::Other(format!("Failed to broadcast to Unix socket clients: {err}"))
            })
    }

    /// Broadcast an error message payload to listeners.
    pub fn send_error(&self, message: &str) -> Result<()> {
        let payload = json!({
            "error": message,
        });
        self.send_value(&payload)
    }
}

impl Drop for UnixBroadcast {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_file(&self.socket_path) {
            tracing::debug!(
                "Failed to cleanup Unix socket {}: {}",
                self.socket_path.display(),
                err
            );
        }
    }
}

async fn handle_client(mut stream: UnixStream, mut rx: broadcast::Receiver<Arc<String>>) {
    while let Ok(payload) = rx.recv().await {
        if let Err(err) = stream.write_all(payload.as_bytes()).await {
            tracing::debug!("Unix socket client write error: {err}");
            break;
        }
        if let Err(err) = stream.write_all(b"\n").await {
            tracing::debug!("Unix socket client newline error: {err}");
            break;
        }
        if let Err(err) = stream.flush().await {
            tracing::debug!("Unix socket flush error: {err}");
            break;
        }
    }
}

// Needed because we use json! macro
use serde_json::json;

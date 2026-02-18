//! MCP transports — stdio (Content-Length framed) and HTTP.

use crate::mcp::types::{JsonRpcRequest, JsonRpcResponse};
use crate::mcp::McpError;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// Stdio transport using Content-Length framing (MCP spec).
pub struct StdioTransport {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    next_id: std::sync::atomic::AtomicU64,
}

impl StdioTransport {
    pub fn spawn(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Self, McpError> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .envs(env)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

        let mut child = cmd.spawn().map_err(|_| McpError::SpawnFailed)?;
        let stdin = child.stdin.take().ok_or(McpError::SpawnFailed)?;
        let stdout = child.stdout.take().ok_or(McpError::SpawnFailed)?;

        Ok(Self {
            child,
            stdin,
            stdout,
            next_id: std::sync::atomic::AtomicU64::new(1),
        })
    }

    /// Send a JSON-RPC request and read the response (Content-Length framed).
    pub async fn send_request(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, McpError> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: method.to_string(),
            params,
        };

        let body = serde_json::to_vec(&request)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());

        self.stdin.write_all(header.as_bytes()).await?;
        self.stdin.write_all(&body).await?;
        self.stdin.flush().await?;

        let response = self.read_response().await?;
        if let Some(error) = response.error {
            return Err(McpError::ServerError(error.message));
        }
        response.result.ok_or(McpError::EmptyResult)
    }

    /// Send a notification (no id, no response expected).
    pub async fn send_notification(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<(), McpError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: method.to_string(),
            params,
        };

        let body = serde_json::to_vec(&request)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());

        self.stdin.write_all(header.as_bytes()).await?;
        self.stdin.write_all(&body).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    /// Read one response: Content-Length line, then exactly N bytes.
    async fn read_response(&mut self) -> Result<JsonRpcResponse, McpError> {
        let mut content_length: Option<usize> = None;
        let mut line = Vec::new();
        loop {
            let mut buf = [0u8; 1];
            while self.stdout.read(&mut buf).await? == 1 {
                if buf[0] == b'\n' {
                    break;
                }
                line.push(buf[0]);
            }
            let s = String::from_utf8_lossy(&line);
            let s = s.trim();
            if s.is_empty() {
                break;
            }
            if let Some(stripped) = s.strip_prefix("Content-Length:") {
                let n: usize = stripped
                    .trim()
                    .parse()
                    .map_err(|_| McpError::ServerError("Invalid Content-Length".to_string()))?;
                content_length = Some(n);
            }
            line.clear();
        }

        let n =
            content_length.ok_or(McpError::ServerError("Missing Content-Length".to_string()))?;
        let mut body = vec![0u8; n];
        let mut read = 0;
        let timeout = Duration::from_secs(30);
        let start = std::time::Instant::now();
        while read < n && start.elapsed() < timeout {
            let got = self.stdout.read(&mut body[read..]).await?;
            if got == 0 {
                break;
            }
            read += got;
        }
        if read < n {
            return Err(McpError::Timeout);
        }

        let response: JsonRpcResponse = serde_json::from_slice(&body)?;
        Ok(response)
    }

    pub async fn shutdown(&mut self) {
        let _ = self.child.kill().await;
    }
}

/// HTTP transport: POST JSON-RPC to {url}/message, response in body.
/// Retries with exponential backoff (1s, 2s, 4s) on connection failure.
pub struct HttpTransport {
    base_url: String,
    client: reqwest::Client,
    next_id: std::sync::atomic::AtomicU64,
}

impl HttpTransport {
    pub fn new(base_url: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
            next_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    pub async fn send_request(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, McpError> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: method.to_string(),
            params,
        };

        let url = format!("{}/message", self.base_url);
        let mut last_err = None;
        for (attempt, delay_ms) in [1000_u64, 2000, 4000].iter().enumerate() {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(*delay_ms)).await;
            }
            match self.client.post(&url).json(&request).send().await {
                Ok(resp) => {
                    let body = resp
                        .text()
                        .await
                        .map_err(|e| McpError::ServerError(e.to_string()))?;
                    let response: JsonRpcResponse =
                        serde_json::from_str(&body).map_err(McpError::Json)?;
                    if let Some(error) = response.error {
                        return Err(McpError::ServerError(error.message));
                    }
                    return response.result.ok_or(McpError::EmptyResult);
                }
                Err(e) => {
                    last_err = Some(McpError::ServerError(e.to_string()));
                }
            }
        }
        Err(last_err.unwrap_or_else(|| {
            McpError::ServerError("HTTP request failed after retries".to_string())
        }))
    }

    pub async fn send_notification(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<(), McpError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: method.to_string(),
            params,
        };
        let url = format!("{}/message", self.base_url);
        self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| McpError::ServerError(e.to_string()))?;
        Ok(())
    }

    #[allow(clippy::unused_async)]
    pub async fn shutdown(&mut self) {
        // No-op for HTTP
        let _ = self;
    }
}

//! MCP manager — spawn servers, list tools, dispatch tool calls.

use crate::config::schema::{McpConfig, McpTransport};
use crate::mcp::transport::{HttpTransport, StdioTransport};
use crate::mcp::types::McpToolDefinition;
use crate::mcp::McpError;
use crate::tools::ToolResult;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use tracing;

const MCP_TOOL_OUTPUT_MAX_LEN: usize = 100_000;
const MCP_PREFIX_SEP: &str = "__";

enum TransportImpl {
    Stdio(StdioTransport),
    Http(HttpTransport),
}

impl TransportImpl {
    async fn send_request(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, McpError> {
        match self {
            TransportImpl::Stdio(t) => t.send_request(method, params).await,
            TransportImpl::Http(t) => t.send_request(method, params).await,
        }
    }

    async fn shutdown(&mut self) {
        match self {
            TransportImpl::Stdio(t) => t.shutdown().await,
            TransportImpl::Http(t) => t.shutdown().await,
        }
    }
}

struct McpServer {
    name: String,
    transport: TransportImpl,
    tools: Vec<McpToolDefinition>,
}

/// Manages MCP server connections and tool dispatch.
pub struct McpManager {
    servers: HashMap<String, McpServer>,
    /// Prefixed tool names (server__tool) for quick lookup
    prefixed_names: std::collections::HashSet<String>,
}

impl McpManager {
    /// Initialize MCP servers from any serializable config (e.g. from lib or binary Config).
    /// Use this when the config type may differ between crate boundaries.
    pub async fn from_config_any(config: &impl Serialize) -> Self {
        let value = serde_json::to_value(config).unwrap_or_else(|_| Value::Object(Map::default()));
        let mcp_config: McpConfig = serde_json::from_value(value).unwrap_or_default();
        Self::from_config(&mcp_config).await
    }

    /// Initialize MCP servers from config. Skips disabled or failing servers.
    pub async fn from_config(config: &McpConfig) -> Self {
        let mut servers = HashMap::new();
        let mut prefixed_names = std::collections::HashSet::new();

        for (name, server_config) in &config.servers {
            if !server_config.enabled {
                continue;
            }
            match &server_config.transport {
                McpTransport::Stdio => {
                    let command = match &server_config.command {
                        Some(cmd) => cmd.clone(),
                        None => {
                            tracing::warn!("MCP server {name} missing command, skipping");
                            continue;
                        }
                    };
                    let args = server_config.args.clone();
                    let env = server_config.env.clone();
                    match Self::init_stdio_server(name, &command, &args, &env).await {
                        Ok(server) => {
                            for t in &server.tools {
                                let prefixed = format!("{name}{MCP_PREFIX_SEP}{}", t.name);
                                prefixed_names.insert(prefixed);
                            }
                            servers.insert(name.clone(), server);
                        }
                        Err(e) => {
                            tracing::warn!("MCP server {name} failed to initialize: {e}");
                        }
                    }
                }
                McpTransport::Http => {
                    let url = match &server_config.url {
                        Some(u) if !u.is_empty() => u.clone(),
                        _ => {
                            tracing::warn!("MCP server {name} HTTP missing url, skipping");
                            continue;
                        }
                    };
                    match Self::init_http_server(name, &url).await {
                        Ok(server) => {
                            for t in &server.tools {
                                let prefixed = format!("{name}{MCP_PREFIX_SEP}{}", t.name);
                                prefixed_names.insert(prefixed);
                            }
                            servers.insert(name.clone(), server);
                        }
                        Err(e) => {
                            tracing::warn!("MCP server {name} HTTP failed to initialize: {e}");
                        }
                    }
                }
            }
        }

        Self {
            servers,
            prefixed_names,
        }
    }

    async fn init_stdio_server(
        name: &str,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<McpServer, McpError> {
        let mut transport = StdioTransport::spawn(command, args, env)?;

        let init_params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "zeroclaw",
                "version": "0.1.0"
            }
        });
        let _ = transport
            .send_request("initialize", Some(init_params))
            .await?;
        transport
            .send_notification("notifications/initialized", None)
            .await?;

        let tools_result = transport.send_request("tools/list", None).await?;
        let tools: Vec<McpToolDefinition> = serde_json::from_value(
            tools_result
                .get("tools")
                .cloned()
                .unwrap_or_else(|| Value::Array(vec![])),
        )?;

        tracing::info!("MCP server {name} initialized with {} tools", tools.len());

        Ok(McpServer {
            name: name.to_string(),
            transport: TransportImpl::Stdio(transport),
            tools,
        })
    }

    async fn init_http_server(name: &str, url: &str) -> Result<McpServer, McpError> {
        let transport = HttpTransport::new(url);

        let init_params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "zeroclaw",
                "version": "0.1.0"
            }
        });
        let _ = transport
            .send_request("initialize", Some(init_params))
            .await?;
        transport
            .send_notification("notifications/initialized", None)
            .await?;

        let tools_result = transport.send_request("tools/list", None).await?;
        let tools: Vec<McpToolDefinition> = serde_json::from_value(
            tools_result
                .get("tools")
                .cloned()
                .unwrap_or_else(|| Value::Array(vec![])),
        )?;

        tracing::info!(
            "MCP server {name} (HTTP) initialized with {} tools",
            tools.len()
        );

        Ok(McpServer {
            name: name.to_string(),
            transport: TransportImpl::Http(transport),
            tools,
        })
    }

    /// Returns true if the tool name is an MCP tool (server__tool).
    pub fn is_mcp_tool(&self, name: &str) -> bool {
        self.prefixed_names.contains(name)
    }

    /// Tool definitions in OpenAI function-calling format for merging with built-in tools.
    pub fn tool_definitions_openai(&self) -> Vec<Value> {
        let mut out = Vec::new();
        for (server_name, server) in &self.servers {
            for t in &server.tools {
                let prefixed_name = format!("{server_name}{MCP_PREFIX_SEP}{}", t.name);
                let description = t.description.as_deref().unwrap_or("MCP tool").to_string();
                out.push(serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": prefixed_name,
                        "description": description,
                        "parameters": t.input_schema
                    }
                }));
            }
        }
        out
    }

    /// Call an MCP tool by prefixed name (e.g. github__create_issue).
    pub async fn call_tool(
        &mut self,
        prefixed_name: &str,
        arguments: Value,
    ) -> Result<ToolResult, McpError> {
        let (server_name, tool_name) = Self::parse_prefixed_name(prefixed_name)?;
        let server = self
            .servers
            .get_mut(&server_name)
            .ok_or_else(|| McpError::ServerNotFound(server_name.clone()))?;

        let params = serde_json::json!({
            "name": tool_name,
            "arguments": arguments,
        });

        let result = server
            .transport
            .send_request("tools/call", Some(params))
            .await?;
        let mcp_result: crate::mcp::types::McpToolResult = serde_json::from_value(result)?;

        let output: String = mcp_result
            .content
            .iter()
            .filter_map(|c| c.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n");

        let output = if output.len() > MCP_TOOL_OUTPUT_MAX_LEN {
            format!(
                "{}...\n[truncated to {} bytes]",
                &output[..MCP_TOOL_OUTPUT_MAX_LEN],
                MCP_TOOL_OUTPUT_MAX_LEN
            )
        } else {
            output
        };

        Ok(ToolResult {
            success: !mcp_result.is_error,
            output,
            error: if mcp_result.is_error {
                Some("MCP tool returned error".to_string())
            } else {
                None
            },
        })
    }

    fn parse_prefixed_name(prefixed: &str) -> Result<(String, String), McpError> {
        let parts: Vec<&str> = prefixed.splitn(2, MCP_PREFIX_SEP).collect();
        match parts.as_slice() {
            [server, tool] if !server.is_empty() && !tool.is_empty() => {
                Ok(((*server).to_string(), (*tool).to_string()))
            }
            _ => Err(McpError::InvalidPrefixedName(prefixed.to_string())),
        }
    }

    /// Shutdown all MCP server processes.
    pub async fn shutdown(&mut self) {
        for server in self.servers.values_mut() {
            server.transport.shutdown().await;
        }
    }
}

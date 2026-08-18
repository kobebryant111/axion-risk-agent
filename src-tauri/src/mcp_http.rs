//! 精简 Streamable HTTP MCP 客户端（initialize / tools/list / tools/call）。
//! 兼容企查查、天眼查等远程 MCP Server。

use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, rename = "inputSchema")]
    pub input_schema: Value,
}

pub struct McpHttpClient {
    url: String,
    authorization: String,
    client: Client,
    session_id: Option<String>,
    next_id: AtomicU64,
}

impl McpHttpClient {
    pub fn new(url: &str, authorization: &str) -> Result<Self, String> {
        Self::with_timeouts(url, authorization, 20, 90)
    }

    /// 较短超时，用于准入一键分析等「可失败但不可卡死」的探测。
    pub fn with_timeouts(
        url: &str,
        authorization: &str,
        connect_secs: u64,
        timeout_secs: u64,
    ) -> Result<Self, String> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(connect_secs))
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            url: url.trim().to_string(),
            authorization: authorization.trim().to_string(),
            client,
            session_id: None,
            next_id: AtomicU64::new(1),
        })
    }

    pub fn initialize(&mut self) -> Result<Value, String> {
        let result = self.rpc(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "axiom-risk-agent",
                    "version": "0.1.0"
                }
            }),
        )?;
        // 通知服务端初始化完成（无 id）
        let _ = self.notify(
            "notifications/initialized",
            json!({}),
        );
        Ok(result)
    }

    pub fn list_tools(&mut self) -> Result<Vec<McpToolInfo>, String> {
        let result = self.rpc("tools/list", json!({}))?;
        let tools = result
            .get("tools")
            .cloned()
            .unwrap_or_else(|| json!([]));
        serde_json::from_value(tools).map_err(|e| format!("解析 tools/list 失败: {e}"))
    }

    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        self.rpc(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments
            }),
        )
    }

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    fn headers(&self) -> Result<HeaderMap, String> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/event-stream"),
        );
        if !self.authorization.is_empty() {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&self.authorization)
                    .map_err(|e| format!("无效 Authorization: {e}"))?,
            );
        }
        if let Some(sid) = &self.session_id {
            headers.insert(
                HeaderName::from_static("mcp-session-id"),
                HeaderValue::from_str(sid).map_err(|e| format!("无效 session id: {e}"))?,
            );
        }
        Ok(headers)
    }

    fn capture_session(&mut self, resp: &reqwest::blocking::Response) {
        if let Some(v) = resp.headers().get("mcp-session-id") {
            if let Ok(s) = v.to_str() {
                if !s.is_empty() {
                    self.session_id = Some(s.to_string());
                }
            }
        }
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        let body = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        let headers = self.headers()?;
        let resp = self
            .client
            .post(&self.url)
            .headers(headers)
            .json(&body)
            .send()
            .map_err(|e| format!("MCP notify 失败: {e}"))?;
        self.capture_session(&resp);
        // 通知可无 body / 可 202
        Ok(())
    }

    fn rpc(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id();
        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        let headers = self.headers()?;
        let resp = self
            .client
            .post(&self.url)
            .headers(headers)
            .json(&body)
            .send()
            .map_err(|e| format!("MCP 请求失败 ({method}): {e}"))?;

        self.capture_session(&resp);
        let status = resp.status();
        let ctype = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let text = resp.text().map_err(|e| e.to_string())?;

        if !status.is_success() {
            return Err(format!("MCP HTTP {status} ({method}): {text}"));
        }

        let msg = if ctype.contains("text/event-stream") {
            parse_sse_jsonrpc(&text, id)?
        } else {
            parse_jsonrpc_message(&text, id)?
        };

        if let Some(err) = msg.get("error") {
            let message = err
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(format!("MCP error ({method}): {message} | {err}"));
        }

        Ok(msg.get("result").cloned().unwrap_or(Value::Null))
    }
}

fn parse_jsonrpc_message(text: &str, expect_id: u64) -> Result<Value, String> {
    let v: Value =
        serde_json::from_str(text.trim()).map_err(|e| format!("MCP JSON 解析失败: {e}; body={text}"))?;
    if v.get("id").and_then(|i| i.as_u64()) == Some(expect_id)
        || v.get("id").and_then(|i| i.as_i64()) == Some(expect_id as i64)
        || v.get("id").and_then(|i| i.as_str()) == Some(&expect_id.to_string())
    {
        return Ok(v);
    }
    // 部分网关可能返回数组
    if let Some(arr) = v.as_array() {
        for item in arr {
            if matches_id(item, expect_id) {
                return Ok(item.clone());
            }
        }
    }
    // 若只有一条且带 result/error，直接用
    if v.get("result").is_some() || v.get("error").is_some() {
        return Ok(v);
    }
    Err(format!("MCP 响应未匹配 id={expect_id}: {text}"))
}

fn parse_sse_jsonrpc(text: &str, expect_id: u64) -> Result<Value, String> {
    let mut data_buf = String::new();
    let mut last_err = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            let chunk = rest.trim();
            if chunk.is_empty() {
                continue;
            }
            if !data_buf.is_empty() {
                data_buf.push('\n');
            }
            data_buf.push_str(chunk);
            continue;
        }
        if line.trim().is_empty() && !data_buf.is_empty() {
            match try_parse_rpc_data(&data_buf, expect_id) {
                Ok(v) => return Ok(v),
                Err(e) => {
                    last_err = e;
                    data_buf.clear();
                }
            }
        }
    }
    if !data_buf.is_empty() {
        return try_parse_rpc_data(&data_buf, expect_id);
    }
    if !last_err.is_empty() {
        return Err(last_err);
    }
    Err(format!("SSE 中未找到 JSON-RPC 响应: {text}"))
}

fn try_parse_rpc_data(data: &str, expect_id: u64) -> Result<Value, String> {
    let v: Value = serde_json::from_str(data.trim())
        .map_err(|e| format!("SSE data JSON 解析失败: {e}; data={data}"))?;
    if matches_id(&v, expect_id) || v.get("result").is_some() || v.get("error").is_some() {
        return Ok(v);
    }
    Err(format!("SSE data 非目标响应: {data}"))
}

fn matches_id(v: &Value, expect_id: u64) -> bool {
    v.get("id").and_then(|i| i.as_u64()) == Some(expect_id)
        || v.get("id").and_then(|i| i.as_i64()) == Some(expect_id as i64)
        || v.get("id").and_then(|i| i.as_str()) == Some(&expect_id.to_string())
}

/// 规范化 Authorization：企查查需要 Bearer；天眼查兼容裸 Key 或 Bearer。
pub fn normalize_auth(provider: &str, api_key: &str) -> String {
    let k = api_key.trim();
    if k.is_empty() {
        return String::new();
    }
    if k.to_ascii_lowercase().starts_with("bearer ") {
        return k.to_string();
    }
    match provider {
        "qcc" => format!("Bearer {k}"),
        // 天眼查文档示例多为裸 Key；同时不少客户端用 Bearer 也能过
        "tyc" => k.to_string(),
        _ => format!("Bearer {k}"),
    }
}

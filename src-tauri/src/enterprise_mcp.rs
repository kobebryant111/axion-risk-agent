//! 企查查 / 天眼查 MCP 配置与调用封装。

use crate::db;
use crate::llm::crypto;
use crate::mcp_http::{normalize_auth, McpHttpClient, McpToolInfo};
use crate::models::{
    EnterpriseMcpSettings, EnterpriseMcpSettingsSave, EnterpriseMcpTestResult,
};
use rusqlite::Connection;
use serde_json::{json, Value};

const KEY_QCC_ENABLED: &str = "enterprise.qcc_enabled";
const KEY_QCC_API_KEY: &str = "enterprise.qcc_api_key_enc";
const KEY_TYC_ENABLED: &str = "enterprise.tyc_enabled";
const KEY_TYC_API_KEY: &str = "enterprise.tyc_api_key_enc";

/// 风控相关企查查 MCP Server（默认接入这些，避免一次拉满全部积分消耗面）
pub const QCC_SERVERS: &[(&str, &str)] = &[
    ("company", "https://agent.qcc.com/mcp/company/stream"),
    ("risk", "https://agent.qcc.com/mcp/risk/stream"),
    ("operation", "https://agent.qcc.com/mcp/operation/stream"),
    ("executive", "https://agent.qcc.com/mcp/executive/stream"),
    ("regulation", "https://agent.qcc.com/mcp/regulation/stream"),
    ("case", "https://agent.qcc.com/mcp/case/stream"),
];

pub const TYC_MCP_URL: &str = "https://mcp.tianyancha.com/mcp";

#[derive(Debug, Clone)]
pub struct EnterpriseMcpConfig {
    pub qcc_enabled: bool,
    pub qcc_api_key: String,
    pub tyc_enabled: bool,
    pub tyc_api_key: String,
}

impl EnterpriseMcpConfig {
    pub fn qcc_ready(&self) -> bool {
        self.qcc_enabled && !self.qcc_api_key.trim().is_empty()
    }
    pub fn tyc_ready(&self) -> bool {
        self.tyc_enabled && !self.tyc_api_key.trim().is_empty()
    }
}

pub fn load_config(conn: &Connection) -> Result<EnterpriseMcpConfig, String> {
    let qcc_enabled = db::get_setting(conn, KEY_QCC_ENABLED)?
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let tyc_enabled = db::get_setting(conn, KEY_TYC_ENABLED)?
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let qcc_api_key = match db::get_setting(conn, KEY_QCC_API_KEY)? {
        Some(enc) if !enc.is_empty() => crypto::decrypt(&enc).unwrap_or_default(),
        _ => String::new(),
    };
    let tyc_api_key = match db::get_setting(conn, KEY_TYC_API_KEY)? {
        Some(enc) if !enc.is_empty() => crypto::decrypt(&enc).unwrap_or_default(),
        _ => String::new(),
    };
    Ok(EnterpriseMcpConfig {
        qcc_enabled,
        qcc_api_key,
        tyc_enabled,
        tyc_api_key,
    })
}

pub fn to_public(cfg: &EnterpriseMcpConfig) -> EnterpriseMcpSettings {
    EnterpriseMcpSettings {
        qcc_enabled: cfg.qcc_enabled,
        qcc_has_api_key: !cfg.qcc_api_key.is_empty(),
        qcc_ready: cfg.qcc_ready(),
        tyc_enabled: cfg.tyc_enabled,
        tyc_has_api_key: !cfg.tyc_api_key.is_empty(),
        tyc_ready: cfg.tyc_ready(),
    }
}

pub fn save_config(
    conn: &Connection,
    input: &EnterpriseMcpSettingsSave,
) -> Result<EnterpriseMcpSettings, String> {
    let current = load_config(conn)?;
    let qcc_enabled = input.qcc_enabled.unwrap_or(current.qcc_enabled);
    let tyc_enabled = input.tyc_enabled.unwrap_or(current.tyc_enabled);

    db::set_setting(conn, KEY_QCC_ENABLED, if qcc_enabled { "1" } else { "0" })?;
    db::set_setting(conn, KEY_TYC_ENABLED, if tyc_enabled { "1" } else { "0" })?;

    if let Some(key) = &input.qcc_api_key {
        let key = key.trim();
        if key.is_empty() {
            db::set_setting(conn, KEY_QCC_API_KEY, "")?;
        } else if key != "********" {
            db::set_setting(conn, KEY_QCC_API_KEY, &crypto::encrypt(key)?)?;
        }
    }
    if let Some(key) = &input.tyc_api_key {
        let key = key.trim();
        if key.is_empty() {
            db::set_setting(conn, KEY_TYC_API_KEY, "")?;
        } else if key != "********" {
            db::set_setting(conn, KEY_TYC_API_KEY, &crypto::encrypt(key)?)?;
        }
    }

    Ok(to_public(&load_config(conn)?))
}

fn qcc_url(server: &str) -> Result<&'static str, String> {
    QCC_SERVERS
        .iter()
        .find(|(k, _)| *k == server)
        .map(|(_, u)| *u)
        .ok_or_else(|| {
            format!(
                "未知企查查 server: {server}。可选: {}",
                QCC_SERVERS
                    .iter()
                    .map(|(k, _)| *k)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

pub fn open_qcc(cfg: &EnterpriseMcpConfig, server: &str) -> Result<McpHttpClient, String> {
    if !cfg.qcc_ready() {
        return Err("企查查 MCP 未就绪：请在设置中启用并填写 API Key".into());
    }
    let url = qcc_url(server)?;
    let auth = normalize_auth("qcc", &cfg.qcc_api_key);
    let mut client = McpHttpClient::new(url, &auth)?;
    client.initialize()?;
    Ok(client)
}

pub fn open_tyc(cfg: &EnterpriseMcpConfig) -> Result<McpHttpClient, String> {
    if !cfg.tyc_ready() {
        return Err("天眼查 MCP 未就绪：请在设置中启用并填写 API Key".into());
    }
    let auth = normalize_auth("tyc", &cfg.tyc_api_key);
    let mut client = McpHttpClient::new(TYC_MCP_URL, &auth)?;
    // 天眼查部分环境裸 Key 失败时可试 Bearer
    match client.initialize() {
        Ok(_) => Ok(client),
        Err(e1) => {
            let auth2 = format!("Bearer {}", cfg.tyc_api_key.trim());
            let mut client2 = McpHttpClient::new(TYC_MCP_URL, &auth2)?;
            client2
                .initialize()
                .map_err(|e2| format!("天眼查 MCP 初始化失败: {e1}；Bearer 重试: {e2}"))?;
            Ok(client2)
        }
    }
}

pub fn test_provider(cfg: &EnterpriseMcpConfig, provider: &str) -> EnterpriseMcpTestResult {
    match provider {
        "qcc" => {
            if !cfg.qcc_ready() {
                return EnterpriseMcpTestResult {
                    ok: false,
                    message: "请先启用企查查并填写 API Key".into(),
                    tool_count: 0,
                };
            }
            match open_qcc(cfg, "company").and_then(|mut c| c.list_tools()) {
                Ok(tools) => EnterpriseMcpTestResult {
                    ok: true,
                    message: format!(
                        "企查查 MCP 连通成功（company 服务，{} 个工具）",
                        tools.len()
                    ),
                    tool_count: tools.len() as u32,
                },
                Err(e) => EnterpriseMcpTestResult {
                    ok: false,
                    message: e,
                    tool_count: 0,
                },
            }
        }
        "tyc" => {
            if !cfg.tyc_ready() {
                return EnterpriseMcpTestResult {
                    ok: false,
                    message: "请先启用天眼查并填写 API Key".into(),
                    tool_count: 0,
                };
            }
            match open_tyc(cfg).and_then(|mut c| c.list_tools()) {
                Ok(tools) => EnterpriseMcpTestResult {
                    ok: true,
                    message: format!("天眼查 MCP 连通成功（{} 个工具）", tools.len()),
                    tool_count: tools.len() as u32,
                },
                Err(e) => EnterpriseMcpTestResult {
                    ok: false,
                    message: e,
                    tool_count: 0,
                },
            }
        }
        other => EnterpriseMcpTestResult {
            ok: false,
            message: format!("未知 provider: {other}"),
            tool_count: 0,
        },
    }
}

pub fn list_qcc_tools(cfg: &EnterpriseMcpConfig, server: &str) -> Result<Vec<McpToolInfo>, String> {
    let mut client = open_qcc(cfg, server)?;
    client.list_tools()
}

pub fn list_tyc_tools(cfg: &EnterpriseMcpConfig) -> Result<Vec<McpToolInfo>, String> {
    let mut client = open_tyc(cfg)?;
    client.list_tools()
}

pub fn call_qcc(
    cfg: &EnterpriseMcpConfig,
    server: &str,
    tool: &str,
    arguments: Value,
) -> Result<Value, String> {
    let mut client = open_qcc(cfg, server)?;
    let result = client.call_tool(tool, arguments)?;
    Ok(summarize_tool_result(result))
}

pub fn call_tyc(
    cfg: &EnterpriseMcpConfig,
    tool: &str,
    arguments: Value,
) -> Result<Value, String> {
    let mut client = open_tyc(cfg)?;
    let result = client.call_tool(tool, arguments)?;
    Ok(summarize_tool_result(result))
}

/// 把 MCP tools/call 结果压成对 LLM 友好的 JSON（截断过长文本）。
fn summarize_tool_result(result: Value) -> Value {
    let text = if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
        let mut parts = Vec::new();
        for item in content {
            if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                    parts.push(t.to_string());
                }
            } else {
                parts.push(item.to_string());
            }
        }
        parts.join("\n")
    } else {
        result.to_string()
    };

    let truncated = if text.chars().count() > 12000 {
        let t: String = text.chars().take(12000).collect();
        format!("{t}\n…(已截断)")
    } else {
        text
    };

    // 尽量解析为 JSON，便于前端/模型使用
    if let Ok(v) = serde_json::from_str::<Value>(&truncated) {
        json!({"ok": true, "data": v})
    } else {
        json!({"ok": true, "text": truncated})
    }
}

pub fn tools_brief(tools: &[McpToolInfo], limit: usize) -> Value {
    let rows: Vec<Value> = tools
        .iter()
        .take(limit)
        .map(|t| {
            json!({
                "name": t.name,
                "description": truncate_str(&t.description, 160),
            })
        })
        .collect();
    json!({
        "count": tools.len(),
        "shown": rows.len(),
        "tools": rows
    })
}

fn truncate_str(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        format!("{t}…")
    } else {
        t
    }
}

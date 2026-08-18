pub mod crypto;

use crate::db;
use crate::models::{LlmConfigPublic, LlmConfigSave, LlmTestResult};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const KEY_BASE_URL: &str = "llm.base_url";
const KEY_API_KEY: &str = "llm.api_key_enc";
const KEY_MODEL: &str = "llm.model";
const KEY_ENABLED: &str = "llm.enabled";

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub enabled: bool,
}

impl LlmConfig {
    pub fn is_ready(&self) -> bool {
        self.enabled && !self.api_key.is_empty() && !self.model.is_empty() && !self.base_url.is_empty()
    }

    pub fn chat_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        if base.ends_with("/chat/completions") {
            base.to_string()
        } else if base.ends_with("/v1") {
            format!("{base}/chat/completions")
        } else {
            format!("{base}/v1/chat/completions")
        }
    }
}

pub fn load_config(conn: &Connection) -> Result<LlmConfig, String> {
    let base_url = db::get_setting(conn, KEY_BASE_URL)?
        .unwrap_or_else(|| "https://api.openai.com/v1".into());
    let model = db::get_setting(conn, KEY_MODEL)?.unwrap_or_else(|| "gpt-4o-mini".into());
    let enabled = db::get_setting(conn, KEY_ENABLED)?
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let api_key = match db::get_setting(conn, KEY_API_KEY)? {
        Some(enc) if !enc.is_empty() => crypto::decrypt(&enc)?,
        _ => String::new(),
    };
    Ok(LlmConfig {
        base_url,
        api_key,
        model,
        enabled,
    })
}

pub fn save_config(conn: &Connection, input: &LlmConfigSave) -> Result<LlmConfigPublic, String> {
    let current = load_config(conn)?;
    let base_url = input
        .base_url
        .clone()
        .unwrap_or(current.base_url)
        .trim()
        .trim_end_matches('/')
        .to_string();
    let model = input.model.clone().unwrap_or(current.model);
    let enabled = input.enabled.unwrap_or(current.enabled);

    db::set_setting(conn, KEY_BASE_URL, &base_url)?;
    db::set_setting(conn, KEY_MODEL, &model)?;
    db::set_setting(conn, KEY_ENABLED, if enabled { "1" } else { "0" })?;

    if let Some(key) = &input.api_key {
        let key = key.trim();
        if key.is_empty() {
            db::set_setting(conn, KEY_API_KEY, "")?;
        } else if key != "********" {
            db::set_setting(conn, KEY_API_KEY, &crypto::encrypt(key)?)?;
        }
    }

    public_view(&load_config(conn)?)
}

pub fn public_view(cfg: &LlmConfig) -> Result<LlmConfigPublic, String> {
    Ok(LlmConfigPublic {
        base_url: cfg.base_url.clone(),
        model: cfg.model.clone(),
        enabled: cfg.enabled,
        has_api_key: !cfg.api_key.is_empty(),
        ready: cfg.is_ready(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub function: ToolFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Serialize)]
pub struct ToolDef {
    #[serde(rename = "type")]
    pub type_: &'static str,
    pub function: ToolFunctionDef,
}

#[derive(Debug, Serialize)]
pub struct ToolFunctionDef {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [ToolDef]>,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    #[serde(default)]
    error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: String,
}

pub fn chat_completion(
    cfg: &LlmConfig,
    messages: &[ChatMessage],
    tools: Option<&[ToolDef]>,
) -> Result<ChatMessage, String> {
    chat_completion_timeout(cfg, messages, tools, 90)
}

/// 文档全量导入等长任务用更长超时（秒）
pub fn chat_completion_timeout(
    cfg: &LlmConfig,
    messages: &[ChatMessage],
    tools: Option<&[ToolDef]>,
    timeout_secs: u64,
) -> Result<ChatMessage, String> {
    if !cfg.is_ready() {
        return Err("LLM 未配置或未启用：请先在「设置」填写 API Base、Model 与 API Key".into());
    }

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| e.to_string())?;

    let body = ChatRequest {
        model: &cfg.model,
        messages,
        tools,
        temperature: 0.2,
    };

    let url = cfg.chat_url();
    let resp = client
        .post(&url)
        .bearer_auth(&cfg.api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                format!(
                    "LLM 请求超时（{timeout_secs}s）。网关或模型处理过慢；可在对方平台调大超时，或缩小文档后再试。地址: {url}"
                )
            } else if e.is_connect() {
                format!("无法连接 LLM 服务（{url}）: {e}")
            } else {
                format!("LLM 请求失败: {e}（{url}）")
            }
        })?;

    let status = resp.status();
    let text = resp.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        if status.as_u16() == 503 || status.as_u16() == 502 || status.as_u16() == 429 {
            return Err(format!(
                "LLM HTTP {status}: 网关过载或限流（Service Unavailable）。将自动分片/重试；若仍失败请稍后再试。详情: {text}"
            ));
        }
        return Err(format!("LLM HTTP {status}: {text}"));
    }

    let parsed: ChatResponse =
        serde_json::from_str(&text).map_err(|e| format!("LLM 响应解析失败: {e}; body={text}"))?;
    if let Some(err) = parsed.error {
        return Err(format!("LLM API error: {}", err.message));
    }
    parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message)
        .ok_or_else(|| "LLM 无 choices 返回".into())
}

fn is_retryable_llm_error(err: &str) -> bool {
    let e = err.to_lowercase();
    e.contains("503")
        || e.contains("502")
        || e.contains("429")
        || e.contains("unavailable")
        || e.contains("超时")
        || e.contains("timeout")
        || e.contains("error sending request")
}

/// 带重试：应对网关 503/超时等瞬时失败
pub fn chat_completion_timeout_retry(
    cfg: &LlmConfig,
    messages: &[ChatMessage],
    tools: Option<&[ToolDef]>,
    timeout_secs: u64,
    retries: u32,
) -> Result<ChatMessage, String> {
    let mut last = String::new();
    for attempt in 0..=retries {
        match chat_completion_timeout(cfg, messages, tools, timeout_secs) {
            Ok(msg) => return Ok(msg),
            Err(e) => {
                last = e;
                if attempt < retries && is_retryable_llm_error(&last) {
                    let wait = 2u64.saturating_mul(attempt as u64 + 1);
                    std::thread::sleep(std::time::Duration::from_secs(wait));
                    continue;
                }
                break;
            }
        }
    }
    if is_retryable_llm_error(&last) {
        Err(format!(
            "{last}\n（已自动重试 {retries} 次仍失败：多为网关过载/限流，请稍后重试或换更稳的 API）"
        ))
    } else {
        Err(last)
    }
}

pub fn test_connection(cfg: &LlmConfig) -> LlmTestResult {
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: Some("你是连接测试助手，只回复 ok。".into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
        ChatMessage {
            role: "user".into(),
            content: Some("ping".into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
    ];
    match chat_completion(cfg, &messages, None) {
        Ok(msg) => LlmTestResult {
            ok: true,
            message: format!(
                "连通成功 · model={} · reply={}",
                cfg.model,
                msg.content.unwrap_or_default().chars().take(80).collect::<String>()
            ),
        },
        Err(e) => LlmTestResult {
            ok: false,
            message: e,
        },
    }
}

//! 风险吹哨：对 Tavily 全网搜结果做 AI 研判与分级。

use crate::llm::{self, ChatMessage, LlmConfig};
use crate::models::Partner;
use crate::sources::RawHit;
use serde::Deserialize;
use std::collections::HashMap;

pub const AI_RULE_ID: &str = "AI-TAVILY";
pub const AI_RULE_SET_VERSION: &str = "ai-tavily-v1";

#[derive(Debug, Clone)]
pub struct AiGrade {
    pub relevant: bool,
    pub level: u8,
    pub analysis: String,
    pub risk_point: String,
    pub legal_basis: String,
}

#[derive(Debug, Deserialize)]
struct AiGradeItem {
    #[serde(default)]
    key: String,
    #[serde(default)]
    relevant: bool,
    #[serde(default)]
    level: u8,
    #[serde(default)]
    analysis: String,
    #[serde(default)]
    risk_point: String,
    #[serde(default)]
    legal_basis: String,
}

#[derive(Debug, Clone)]
struct PendingItem {
    key: String,
    partner_name: String,
    partner_type_label: String,
    hit: RawHit,
}

/// 对已归属到机构的 Tavily 命中做一次（或少量）批量 AI 研判。
/// 返回 key(= partner_id + '|' + evidence 指纹) → 分级结果。
/// LLM 未就绪或失败时返回空 map，由调用方回退规则引擎。
pub fn grade_tavily_hits(
    cfg: &LlmConfig,
    partners_by_id: &HashMap<String, &Partner>,
    hits_by_partner: &HashMap<String, Vec<RawHit>>,
) -> (HashMap<String, AiGrade>, Option<String>) {
    if !cfg.is_ready() {
        return (
            HashMap::new(),
            Some("LLM 未就绪：Tavily 结果将回退规则定级".into()),
        );
    }

    let mut pending: Vec<PendingItem> = Vec::new();
    for (pid, hits) in hits_by_partner {
        let Some(partner) = partners_by_id.get(pid) else {
            continue;
        };
        for hit in hits {
            let key = item_key(pid, hit);
            if pending.iter().any(|p| p.key == key) {
                continue;
            }
            pending.push(PendingItem {
                key,
                partner_name: partner.name.clone(),
                partner_type_label: partner.partner_type_label.clone(),
                hit: hit.clone(),
            });
        }
    }

    if pending.is_empty() {
        return (HashMap::new(), None);
    }

    // 控制 token：单次最多 12 条
    let batch: Vec<&PendingItem> = pending.iter().take(12).collect();
    match call_llm_grade(cfg, &batch) {
        Ok(map) => (map, None),
        Err(e) => (HashMap::new(), Some(format!("AI 研判失败，已回退规则定级: {e}"))),
    }
}

pub fn item_key(partner_id: &str, hit: &RawHit) -> String {
    format!(
        "{}|{}|{}",
        partner_id,
        hit.url.trim(),
        hit.title.chars().take(80).collect::<String>()
    )
}

fn call_llm_grade(
    cfg: &LlmConfig,
    items: &[&PendingItem],
) -> Result<HashMap<String, AiGrade>, String> {
    let payload: Vec<serde_json::Value> = items
        .iter()
        .map(|it| {
            let body: String = it.hit.body.chars().take(360).collect();
            serde_json::json!({
                "key": it.key,
                "partner": it.partner_name,
                "partnerType": it.partner_type_label,
                "title": it.hit.title,
                "summary": it.hit.summary,
                "url": it.hit.url,
                "published": it.hit.event_time,
                "credibility": it.hit.credibility,
                "content": body,
            })
        })
        .collect();

    let system = r#"你是消费金融「合作机构风险吹哨」分析员。根据全网检索到的公开网页信息，判断是否构成对该合作机构的风险信号，并给出四级预警。

分级标准（必须遵守）：
1级：重大违法违规、监管行政处罚/立案、恶性舆情或群体事件，需立即处置上报
2级：明确违规或较大负面（如暴力催收、严重投诉爆发、失信执行等），需限期处置
3级：需持续关注的风险苗头或一般负面报道
4级：弱相关或信息价值有限，可留档跟踪
若不相关、无法核实指向该机构、或纯广告软文：relevant=false，level 可填 0

硬约束：
- 只依据给定材料，禁止编造未出现的事实、链接或处罚文号
- 必须逐条输出，key 与输入完全一致
- 只输出 JSON 数组，不要 markdown 代码围栏，不要额外说明

输出项字段：
[{"key":"...","relevant":true,"level":1,"analysis":"50-120字研判","risk_point":"风险点简称","legal_basis":"可引用的法规/监管口径，未知则写「待人工复核」"}]"#;

    let user = format!(
        "请对以下 {} 条全网搜结果逐条研判并分级：\n{}",
        items.len(),
        serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "[]".into())
    );

    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: Some(system.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
        ChatMessage {
            role: "user".into(),
            content: Some(user),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
    ];

    let reply = llm::chat_completion_timeout_retry(cfg, &messages, None, 90, 1)?;
    let text = reply.content.unwrap_or_default();
    let arr = extract_json_array(&text)?;
    let parsed: Vec<AiGradeItem> =
        serde_json::from_str(&arr).map_err(|e| format!("AI 分级 JSON 解析失败: {e}"))?;

    let mut out = HashMap::new();
    for item in parsed {
        let key = item.key.trim().to_string();
        if key.is_empty() {
            continue;
        }
        let level = item.level.min(4);
        let relevant = item.relevant && (1..=4).contains(&level);
        out.insert(
            key,
            AiGrade {
                relevant,
                level: if relevant { level } else { 0 },
                analysis: truncate(&item.analysis, 200),
                risk_point: if item.risk_point.trim().is_empty() {
                    "全网舆情风险".into()
                } else {
                    truncate(&item.risk_point, 40)
                },
                legal_basis: if item.legal_basis.trim().is_empty() {
                    "AI研判，待人工复核".into()
                } else {
                    truncate(&item.legal_basis, 80)
                },
            },
        );
    }

    // 对未返回的条目不写入 map，调用方回退规则
    Ok(out)
}

fn extract_json_array(text: &str) -> Result<String, String> {
    let start = text.find('[').ok_or_else(|| "AI 未返回 JSON 数组".to_string())?;
    let end = text
        .rfind(']')
        .ok_or_else(|| "AI JSON 数组不完整".to_string())?;
    if end < start {
        return Err("AI JSON 数组不完整".into());
    }
    Ok(text[start..=end].to_string())
}

fn truncate(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        format!("{t}…")
    } else {
        t
    }
}

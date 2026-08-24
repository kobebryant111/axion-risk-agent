use crate::db;
use crate::enterprise_mcp::{self, EnterpriseMcpConfig};
use crate::llm::{self, ChatMessage, LlmConfig, ToolCall, ToolDef};
use crate::models::{
    AgentChatRequest, AgentChatResponse, CustomRuleInput, PartnerType,
};
use crate::pipeline;
use crate::rules;
use crate::state::AppState;
use serde_json::json;
use std::sync::Mutex;
use uuid::Uuid;

pub fn tool_defs(mcp: &EnterpriseMcpConfig, search_ready: bool) -> Vec<ToolDef> {
    let mut tools = vec![
        ToolDef {
            type_: "function",
            function: llm::ToolFunctionDef {
                name: "list_partners",
                description: "列出已导入的合作机构名单与监测词库",
                parameters: json!({"type":"object","properties":{}}),
            },
        },
        ToolDef {
            type_: "function",
            function: llm::ToolFunctionDef {
                name: "list_risk_clues",
                description: "列出风险吹哨线索，可按等级过滤",
                parameters: json!({
                    "type":"object",
                    "properties":{
                        "max_level":{"type":"integer","description":"只返回等级<=该值的线索，如2表示1/2级"}
                    }
                }),
            },
        },
        ToolDef {
            type_: "function",
            function: llm::ToolFunctionDef {
                name: "list_rules",
                description: "查询规则库，可按机构类型过滤",
                parameters: json!({
                    "type":"object",
                    "properties":{
                        "partner_type":{"type":"string","description":"common/loan/guarantee/traffic/payment/data/collection"},
                        "query":{"type":"string","description":"按规则ID或风险点模糊匹配"}
                    }
                }),
            },
        },
        ToolDef {
            type_: "function",
            function: llm::ToolFunctionDef {
                name: "override_rule",
                description: "启用/停用规则或覆盖等级（内置基线不可删除，只能覆盖）",
                parameters: json!({
                    "type":"object",
                    "properties":{
                        "rule_id":{"type":"string"},
                        "enabled":{"type":"boolean"},
                        "level":{"type":"integer","minimum":1,"maximum":4}
                    },
                    "required":["rule_id"]
                }),
            },
        },
        ToolDef {
            type_: "function",
            function: llm::ToolFunctionDef {
                name: "create_custom_rule",
                description: "新增一条自定义监测规则",
                parameters: json!({
                    "type":"object",
                    "properties":{
                        "partner_type":{"type":"string"},
                        "risk_point":{"type":"string"},
                        "level":{"type":"integer","minimum":1,"maximum":4},
                        "keywords":{"type":"array","items":{"type":"string"}},
                        "legal_basis":{"type":"string"}
                    },
                    "required":["partner_type","risk_point","level","keywords"]
                }),
            },
        },
        ToolDef {
            type_: "function",
            function: llm::ToolFunctionDef {
                name: "delete_custom_rule",
                description: "删除自定义规则（不能删内置基线）",
                parameters: json!({
                    "type":"object",
                    "properties":{"rule_id":{"type":"string"}},
                    "required":["rule_id"]
                }),
            },
        },
        ToolDef {
            type_: "function",
            function: llm::ToolFunctionDef {
                name: "run_whistle_batch",
                description: "对合作机构执行风险吹哨跑批（可选百度全网搜 + 规则定级）。可传 partner_ids 指定机构，不传则全量。",
                parameters: json!({
                    "type":"object",
                    "properties":{
                        "partner_ids":{"type":"array","items":{"type":"string"},"description":"可选，机构 ID 列表；空或不传=全部"}
                    }
                }),
            },
        },
        ToolDef {
            type_: "function",
            function: llm::ToolFunctionDef {
                name: "get_dashboard_summary",
                description: "获取总览 KPI：有效线索、1/2级数量、待复核等",
                parameters: json!({"type":"object","properties":{}}),
            },
        },
        ToolDef {
            type_: "function",
            function: llm::ToolFunctionDef {
                name: "analyze_finance_demo",
                description: "经营守护：基于已保存的真实财报评估报告作答。没有财报时请提示用户先上传，不要编造演示数据。",
                parameters: json!({
                    "type":"object",
                    "properties":{
                        "partner_name":{"type":"string","description":"机构名称，可选"}
                    }
                }),
            },
        },
        ToolDef {
            type_: "function",
            function: llm::ToolFunctionDef {
                name: "run_admission_review",
                description: "准入瞭望：对拟合作/续约机构做一票否决/重大违规/关注事项研判；可拉取企查查天眼查；无证据时结论为待核验",
                parameters: json!({
                    "type":"object",
                    "properties":{
                        "partner_id":{"type":"string"},
                        "partner_name":{"type":"string"},
                        "partner_type":{"type":"string","description":"loan|guarantee|traffic|payment|data|collection"},
                        "uscc":{"type":"string"},
                        "scenario":{"type":"string","description":"准入|续约|再审"}
                    }
                }),
            },
        },
    ];

    if search_ready {
        tools.push(ToolDef {
            type_: "function",
            function: llm::ToolFunctionDef {
                name: "baidu_web_search",
                description: "百度千帆全网搜索。query 写成问句并带上新闻用词，例如「桔子数科最近有没有爆雷停摆跑路」。不要只搜机构名，也不要只问「负面信息」。",
                parameters: json!({
                    "type":"object",
                    "properties":{
                        "query":{"type":"string","description":"机构简称+一类负面事件词，如 桔子数科最近有没有爆雷停摆跑路资金池。不要用 OR，不要只写负面信息"},
                        "max_results":{"type":"integer","description":"1-5，默认5"}
                    },
                    "required":["query"]
                }),
            },
        });
    }

    if mcp.qcc_ready() {
        tools.push(ToolDef {
            type_: "function",
            function: llm::ToolFunctionDef {
                name: "qcc_list_tools",
                description: "列出企查查 MCP 某服务下的可用工具名（先 list 再 call）",
                parameters: json!({
                    "type":"object",
                    "properties":{
                        "server":{
                            "type":"string",
                            "description":"company|risk|operation|executive|regulation|case，默认 company"
                        }
                    }
                }),
            },
        });
        tools.push(ToolDef {
            type_: "function",
            function: llm::ToolFunctionDef {
                name: "qcc_call_tool",
                description: "调用企查查 MCP 工具查询企业工商/风险/经营等真实数据（勿编造）",
                parameters: json!({
                    "type":"object",
                    "properties":{
                        "server":{"type":"string","description":"company|risk|operation|executive|regulation|case"},
                        "tool":{"type":"string","description":"MCP 工具名，如 get_company_registration_info"},
                        "arguments":{"type":"object","description":"工具参数，通常含企业名称或统一社会信用代码"}
                    },
                    "required":["server","tool","arguments"]
                }),
            },
        });
    }

    if mcp.tyc_ready() {
        tools.push(ToolDef {
            type_: "function",
            function: llm::ToolFunctionDef {
                name: "tyc_list_tools",
                description: "列出天眼查 MCP 可用工具（先 list 再 call；工具较多时只返回摘要）",
                parameters: json!({"type":"object","properties":{}}),
            },
        });
        tools.push(ToolDef {
            type_: "function",
            function: llm::ToolFunctionDef {
                name: "tyc_call_tool",
                description: "调用天眼查 MCP 工具查询企业工商/风险等真实数据（勿编造）",
                parameters: json!({
                    "type":"object",
                    "properties":{
                        "tool":{"type":"string","description":"MCP 工具名"},
                        "arguments":{"type":"object","description":"工具参数"}
                    },
                    "required":["tool","arguments"]
                }),
            },
        });
    }

    tools
}

const SYSTEM_PROMPT: &str = r#"你是「智联鉴控」合作机构风险管理 AI 智能体助手。
你可以帮助用户：查询机构名单、风险线索、规则库；启用/停用/改等级规则；新增自定义规则；触发吹哨跑批；经营守护（财报评估）；准入瞭望（拟合作前置合规研判）；解读风险含义。
准入场景强调：业务开发初期即可研判，从源头阻断「先合作后尽调」；无证据时结论应为待核验而非绿灯通过。
若已配置百度千帆搜索，可用 baidu_web_search 做真实全网搜索（新闻/处罚/投诉等公开网页）。
若已配置企查查/天眼查 MCP，可用 qcc_* / tyc_* 工具查询企业工商与风险等公开数据：先 list_tools 再 call_tool，禁止编造工商/司法字段。
硬约束：
1. 需要查数或改配置时必须调用工具，不要编造线索 ID / 规则 ID / 网页链接。
2. 定级结论要可解释，引用 rule_id、法规依据或线索字段；全网搜结果需说明来源 URL。
3. 内置基线规则只能 override（停用/改级），不能声称已删除。
4. 用简洁中文回答，先结论后依据。
5. 用户未配置业务数据时，引导其到「机构名单」「规则库」下载固定 Excel 模板导入（无需 AI），再跑批；未配置百度搜索时不要假装已联网搜索。
6. 百度搜索同一轮 baidu_web_search 最多 2 次；query 写成「机构 + 一类事件」问句（爆雷停摆跑路 / 处罚立案投诉），禁止只问「有哪些负面信息」，禁止 OR 布尔式。"#;

pub fn run_agent(
    db: &Mutex<rusqlite::Connection>,
    cfg: &LlmConfig,
    req: &AgentChatRequest,
) -> Result<AgentChatResponse, String> {
    let (mcp_cfg, search_cfg) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        (
            enterprise_mcp::load_config(&conn)?,
            crate::baidu_search::load_config(&conn)?,
        )
    };
    let tools = tool_defs(&mcp_cfg, search_cfg.ready());
    let mut messages: Vec<ChatMessage> = vec![ChatMessage {
        role: "system".into(),
        content: Some(SYSTEM_PROMPT.into()),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }];

    if let Some(hist) = &req.history {
        for h in hist.iter().take(12) {
            messages.push(ChatMessage {
                role: h.role.clone(),
                content: Some(h.content.clone()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        }
    }

    let scope = req.scope.clone().unwrap_or_else(|| "general".into());
    let user_content = if scope == "rules" {
        format!("【场景：规则库维护】\n{}", req.message)
    } else {
        req.message.clone()
    };
    messages.push(ChatMessage {
        role: "user".into(),
        content: Some(user_content),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    });

    let mut tool_traces = Vec::new();
    let mut mutated = false;
    let mut search_calls: u32 = 0;

    for _round in 0..4 {
        // LLM HTTP outside DB lock
        let assistant = llm::chat_completion(cfg, &messages, Some(&tools))?;
        let tool_calls = assistant.tool_calls.clone().unwrap_or_default();
        messages.push(assistant.clone());

        if tool_calls.is_empty() {
            let reply = assistant
                .content
                .unwrap_or_else(|| "（模型未返回文本）".into());
            return Ok(AgentChatResponse {
                ok: true,
                reply,
                used_llm: true,
                mutated,
                tool_traces,
            });
        }

        for call in tool_calls {
            let (result, did_mutate) = if call.function.name == "run_whistle_batch" {
                run_whistle_batch_tool(db, cfg, &call)?
            } else if call.function.name == "baidu_web_search" {
                if search_calls >= crate::baidu_search::MAX_CALLS_PER_OPERATION {
                    (
                        json!({
                            "error": format!(
                                "本轮对话百度搜索已达上限 {} 次，请基于已有结果作答或合并 query 后再试",
                                crate::baidu_search::MAX_CALLS_PER_OPERATION
                            )
                        })
                        .to_string(),
                        false,
                    )
                } else {
                    search_calls += 1;
                    dispatch_baidu_search_tool(&search_cfg, &call)?
                }
            } else if matches!(
                call.function.name.as_str(),
                "qcc_list_tools" | "qcc_call_tool" | "tyc_list_tools" | "tyc_call_tool"
            ) {
                // MCP HTTP 在 DB 锁外执行，避免卡死 UI
                dispatch_mcp_tool(&mcp_cfg, &call)?
            } else {
                let conn = db.lock().map_err(|e| e.to_string())?;
                dispatch_tool(&conn, &call)?
            };
            if did_mutate {
                mutated = true;
            }
            tool_traces.push(format!("{} → {}", call.function.name, truncate(&result, 180)));
            messages.push(ChatMessage {
                role: "tool".into(),
                content: Some(result),
                tool_calls: None,
                tool_call_id: Some(call.id),
                name: Some(call.function.name.clone()),
            });
        }
    }

    Ok(AgentChatResponse {
        ok: true,
        reply: "工具调用轮次已达上限，请把问题拆得更具体一些再问一次。".into(),
        used_llm: true,
        mutated,
        tool_traces,
    })
}

/// Convenience for callers that already hold AppState.
pub fn run_agent_with_state(
    state: &AppState,
    cfg: &LlmConfig,
    req: &AgentChatRequest,
) -> Result<AgentChatResponse, String> {
    run_agent(&state.conn, cfg, req)
}

fn truncate(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        format!("{t}…")
    } else {
        t
    }
}

fn run_whistle_batch_tool(
    db: &Mutex<rusqlite::Connection>,
    _cfg: &LlmConfig,
    call: &ToolCall,
) -> Result<(String, bool), String> {
    let args: serde_json::Value =
        serde_json::from_str(&call.function.arguments).unwrap_or_else(|_| json!({}));
    let ids: Option<Vec<String>> = args.get("partner_ids").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
    });
    let conn = db.lock().map_err(|e| e.to_string())?;
    let id_ref = ids.as_deref().filter(|v| !v.is_empty());
    let res = pipeline::run_whistle_batch_scoped(
        &conn,
        false,
        "agent",
        id_ref,
        true,
        crate::baidu_search::SearchWindow::LastDay,
    )?;
    Ok((serde_json::to_string(&res).unwrap_or_default(), true))
}

fn dispatch_baidu_search_tool(
    cfg: &crate::baidu_search::BaiduSearchConfig,
    call: &ToolCall,
) -> Result<(String, bool), String> {
    if !cfg.ready() {
        return Ok((
            json!({"error":"百度搜索未启用或未配置 API Key，请到设置页配置"}).to_string(),
            false,
        ));
    }
    let args: serde_json::Value =
        serde_json::from_str(&call.function.arguments).unwrap_or_else(|_| json!({}));
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or("缺少 query")?
        .to_string();
    let max_results = args
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(5)
        .clamp(1, 5) as u32;
    let hits = crate::baidu_search::search_hits(&cfg.api_key, &query, max_results)?;
    let brief: Vec<_> = hits
        .iter()
        .map(|h| {
            json!({
                "title": h.title,
                "url": h.url,
                "summary": h.summary,
                "credibility": h.credibility,
                "eventTime": h.event_time,
            })
        })
        .collect();
    Ok((
        json!({"ok":true,"query":query,"count":brief.len(),"results":brief}).to_string(),
        false,
    ))
}

fn dispatch_mcp_tool(
    mcp: &EnterpriseMcpConfig,
    call: &ToolCall,
) -> Result<(String, bool), String> {
    let args: serde_json::Value =
        serde_json::from_str(&call.function.arguments).unwrap_or_else(|_| json!({}));

    let payload = match call.function.name.as_str() {
        "qcc_list_tools" => {
            let server = args
                .get("server")
                .and_then(|v| v.as_str())
                .unwrap_or("company");
            let tools = enterprise_mcp::list_qcc_tools(mcp, server)?;
            enterprise_mcp::tools_brief(&tools, 40)
        }
        "qcc_call_tool" => {
            let server = args
                .get("server")
                .and_then(|v| v.as_str())
                .ok_or("缺少 server")?;
            let tool = args
                .get("tool")
                .and_then(|v| v.as_str())
                .ok_or("缺少 tool")?;
            let arguments = args.get("arguments").cloned().unwrap_or_else(|| json!({}));
            enterprise_mcp::call_qcc(mcp, server, tool, arguments)?
        }
        "tyc_list_tools" => {
            let tools = enterprise_mcp::list_tyc_tools(mcp)?;
            enterprise_mcp::tools_brief(&tools, 40)
        }
        "tyc_call_tool" => {
            let tool = args
                .get("tool")
                .and_then(|v| v.as_str())
                .ok_or("缺少 tool")?;
            let arguments = args.get("arguments").cloned().unwrap_or_else(|| json!({}));
            enterprise_mcp::call_tyc(mcp, tool, arguments)?
        }
        other => json!({"error": format!("unknown mcp tool {other}")}),
    };

    Ok((payload.to_string(), false))
}

fn dispatch_tool(
    conn: &rusqlite::Connection,
    call: &ToolCall,
) -> Result<(String, bool), String> {
    let args: serde_json::Value =
        serde_json::from_str(&call.function.arguments).unwrap_or_else(|_| json!({}));

    match call.function.name.as_str() {
        "list_partners" => {
            let rows = db::list_partners(conn)?;
            Ok((
                serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into()),
                false,
            ))
        }
        "list_risk_clues" => {
            let mut rows = db::list_clues(conn)?;
            if let Some(max) = args.get("max_level").and_then(|v| v.as_u64()) {
                rows.retain(|c| (c.level as u64) <= max);
            }
            rows.truncate(40);
            Ok((
                serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into()),
                false,
            ))
        }
        "list_rules" => {
            let mut views = rules::list_rule_views_from_db(conn)?;
            if let Some(pt) = args.get("partner_type").and_then(|v| v.as_str()) {
                let pt = pt.trim();
                if !pt.is_empty() && pt != "all" {
                    views.retain(|r| r.partner_type == pt);
                }
            }
            if let Some(q) = args.get("query").and_then(|v| v.as_str()) {
                let q = q.trim();
                if !q.is_empty() {
                    views.retain(|r| r.id.contains(q) || r.risk_point.contains(q));
                }
            }
            views.truncate(50);
            Ok((
                serde_json::to_string(&views).unwrap_or_else(|_| "[]".into()),
                false,
            ))
        }
        "override_rule" => {
            let rule_id = args
                .get("rule_id")
                .and_then(|v| v.as_str())
                .ok_or("缺少 rule_id")?
                .to_string();
            let enabled = args.get("enabled").and_then(|v| v.as_bool());
            let level = args
                .get("level")
                .and_then(|v| v.as_u64())
                .map(|v| v as u8);
            db::set_override(conn, &rule_id, enabled, level)?;
            audit(conn, "agent_override_rule", &rule_id, &args.to_string())?;
            Ok((
                json!({"ok":true,"rule_id":rule_id,"enabled":enabled,"level":level}).to_string(),
                true,
            ))
        }
        "create_custom_rule" => {
            let partner_type_raw = args
                .get("partner_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let partner_type = if partner_type_raw == "common" || partner_type_raw == "通用" {
                "common".to_string()
            } else {
                PartnerType::parse(partner_type_raw)
                    .map(|p| p.as_str().to_string())
                    .ok_or_else(|| format!("无法识别机构类型: {partner_type_raw}"))?
            };
            let risk_point = args
                .get("risk_point")
                .and_then(|v| v.as_str())
                .ok_or("缺少 risk_point")?
                .to_string();
            let level = args
                .get("level")
                .and_then(|v| v.as_u64())
                .ok_or("缺少 level")? as u8;
            if !(1..=4).contains(&level) {
                return Err("level 须为 1～4".into());
            }
            let keywords: Vec<String> = args
                .get("keywords")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            if keywords.is_empty() {
                return Err("keywords 不能为空".into());
            }
            let legal_basis = args
                .get("legal_basis")
                .and_then(|v| v.as_str())
                .unwrap_or("Agent 对话新增")
                .to_string();
            let id = format!(
                "CUSTOM-{}-{}",
                partner_type.to_uppercase(),
                &Uuid::new_v4().to_string()[..8]
            );
            let input = CustomRuleInput {
                id: id.clone(),
                partner_type: partner_type.clone(),
                risk_point,
                level,
                match_any_keywords: keywords,
                legal_basis,
                enabled: true,
                trigger: args
                    .get("trigger")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                data_source: args
                    .get("data_source")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            };
            let def = rules::custom_input_to_def(&input);
            db::upsert_custom_rule(conn, &partner_type, &def)?;
            audit(conn, "agent_create_rule", &id, &args.to_string())?;
            Ok((json!({"ok":true,"rule":input}).to_string(), true))
        }
        "delete_custom_rule" => {
            let rule_id = args
                .get("rule_id")
                .and_then(|v| v.as_str())
                .ok_or("缺少 rule_id")?
                .to_string();
            if !rule_id.starts_with("CUSTOM-") {
                return Err("只能删除 CUSTOM- 开头的自定义规则".into());
            }
            db::delete_custom_rule(conn, &rule_id)?;
            audit(conn, "agent_delete_rule", &rule_id, "")?;
            Ok((json!({"ok":true,"deleted":rule_id}).to_string(), true))
        }
        "run_whistle_batch" => {
            Ok((
                "{\"error\":\"run_whistle_batch should be handled outside DB lock\"}".into(),
                false,
            ))
        }
        "get_dashboard_summary" => {
            let clues = db::list_clues(conn)?;
            let partners = db::list_partners(conn)?;
            let summary = json!({
                "partners": partners.len(),
                "clues": clues.len(),
                "level1": clues.iter().filter(|c| c.level==1).count(),
                "level2": clues.iter().filter(|c| c.level==2).count(),
                "pending": clues.iter().filter(|c| c.status.contains("待") || c.status.contains("复核")).count(),
            });
            Ok((summary.to_string(), false))
        }
        "analyze_finance_demo" => {
            Ok((
                json!({"error":"已停用演示样例。请先在「经营守护」上传真实财报后再分析。"})
                    .to_string(),
                false,
            ))
        }
        "run_admission_review" => {
            let req = crate::models::AdmissionReviewRequest {
                partner_id: args
                    .get("partner_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                partner_name: args
                    .get("partner_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                partner_type: args
                    .get("partner_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                uscc: args.get("uscc").and_then(|v| v.as_str()).map(|s| s.to_string()),
                scenario: args
                    .get("scenario")
                    .and_then(|v| v.as_str())
                    .unwrap_or("准入")
                    .to_string(),
                manual_flags: None,
                use_enterprise_mcp: Some(true),
                use_web_search: Some(true),
                use_llm_assist: Some(true),
                materials_text: None,
            };
            let review = crate::admission::run_review(conn, &req)?;
            Ok((
                json!({
                    "id": review.id,
                    "conclusion": review.conclusion,
                    "summary": review.summary,
                    "remediation": review.remediation,
                    "evidenceIncomplete": review.evidence_incomplete,
                    "evidenceSources": review.evidence_sources,
                })
                .to_string(),
                true,
            ))
        }
        other => Ok((format!("{{\"error\":\"unknown tool {other}\"}}"), false)),
    }
}

fn audit(conn: &rusqlite::Connection, action: &str, target: &str, detail: &str) -> Result<(), String> {
    db::insert_audit(
        conn,
        &crate::models::AuditLog {
            id: Uuid::new_v4().to_string(),
            actor: "agent".into(),
            action: action.into(),
            target: target.into(),
            detail: detail.into(),
            ts: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        },
    )
}

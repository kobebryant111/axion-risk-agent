//! 准入瞭望：一票否决 / 重大违规 / 关注事项清单与结论引擎。
//! 支持材料知识库、工商 MCP 补证、LLM 辅助摘要、监管案例关键词迭代。

use crate::db;
use crate::enterprise_mcp::{self, EnterpriseMcpConfig};
use crate::llm::{self, ChatMessage};
use crate::models::{
    AdmissionCaseIngestRequest, AdmissionCaseIngestResult, AdmissionCheckItem,
    AdmissionKnowledge, AdmissionKnowledgeImportRequest, AdmissionReview,
    AdmissionReviewRequest,
};
use rusqlite::{params, Connection};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

pub fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS admission_reviews (
            id TEXT PRIMARY KEY,
            partner_id TEXT NOT NULL DEFAULT '',
            partner_name TEXT NOT NULL,
            partner_type TEXT NOT NULL,
            scenario TEXT NOT NULL,
            conclusion TEXT NOT NULL,
            items_json TEXT NOT NULL,
            remediation_json TEXT NOT NULL DEFAULT '[]',
            summary TEXT NOT NULL DEFAULT '',
            markdown TEXT NOT NULL DEFAULT '',
            evidence_incomplete INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            uscc TEXT NOT NULL DEFAULT '',
            evidence_sources_json TEXT NOT NULL DEFAULT '[]',
            ai_notes TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_admission_partner ON admission_reviews(partner_id);
        CREATE TABLE IF NOT EXISTS admission_knowledge (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS admission_keyword_boosts (
            id TEXT PRIMARY KEY,
            check_id TEXT NOT NULL DEFAULT '',
            keyword TEXT NOT NULL,
            source_title TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );
        ",
    )
    .map_err(|e| e.to_string())?;
    let _ = conn.execute(
        "ALTER TABLE admission_reviews ADD COLUMN uscc TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE admission_reviews ADD COLUMN evidence_sources_json TEXT NOT NULL DEFAULT '[]'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE admission_reviews ADD COLUMN ai_notes TEXT NOT NULL DEFAULT ''",
        [],
    );
    Ok(())
}

#[derive(Clone)]
struct CheckDef {
    id: &'static str,
    color: &'static str,
    title: &'static str,
    legal_basis: &'static str,
    types: &'static [&'static str],
    keywords: &'static [&'static str],
}

#[derive(Clone)]
struct EvidenceDoc {
    source: String,
    text: String,
    ref_url: String,
    level: u8,
}

const CHECKS: &[CheckDef] = &[
    // 红色一票否决
    CheckDef {
        id: "R01",
        color: "red",
        title: "被列入严重违法失信名单",
        legal_basis: "《市场监督管理严重违法失信名单管理办法》",
        types: &[],
        keywords: &["严重违法失信", "违法失信名单"],
    },
    CheckDef {
        id: "R02",
        color: "red",
        title: "被列为失信被执行人",
        legal_basis: "最高法关于失信被执行人名单规定",
        types: &[],
        // 不用单独「失信」：避免「未见失信…」类否定表述误触发
        keywords: &["失信被执行人", "限制高消费", "老赖"],
    },
    CheckDef {
        id: "R03",
        color: "red",
        title: "吊销许可证 / 责令停业整顿",
        legal_basis: "各行业监管法规",
        types: &[],
        keywords: &["吊销", "责令停业", "停业整顿", "撤销许可"],
    },
    CheckDef {
        id: "R04",
        color: "red",
        title: "融担放大倍数超红线（>10/15倍）",
        legal_basis: "融担监管条例第15条",
        types: &["guarantee"],
        keywords: &["放大倍数", "超红线", "杠杆超限"],
    },
    CheckDef {
        id: "R05",
        color: "red",
        title: "为控股股东/实控人提供担保",
        legal_basis: "融担监管条例第17条",
        types: &["guarantee"],
        keywords: &["为实控人担保", "控股股东担保", "关联担保违规"],
    },
    CheckDef {
        id: "R06",
        color: "red",
        title: "从事吸收存款/自营贷款等禁止业务",
        legal_basis: "融担监管条例第23条",
        types: &["guarantee"],
        keywords: &["吸收存款", "自营贷款", "非法集资"],
    },
    CheckDef {
        id: "R07",
        color: "red",
        title: "实控人失联 / 主要股东股权冻结>30%",
        legal_basis: "通用风险点",
        types: &[],
        keywords: &["实控人失联", "股权冻结", "刑事拘留"],
    },
    CheckDef {
        id: "R08",
        color: "red",
        title: "暴力催收或侵犯公民个人信息刑事立案",
        legal_basis: "《个人信息保护法》、助贷新规第8条",
        types: &["collection", "loan"],
        keywords: &["暴力催收", "侵犯公民个人信息", "爆通讯录", "刑事立案"],
    },
    CheckDef {
        id: "R09",
        color: "red",
        title: "综合融资成本超过24%",
        legal_basis: "金规〔2025〕9号第6条",
        types: &["loan"],
        keywords: &["综合融资成本", "超过24%", "利率超24"],
    },
    CheckDef {
        id: "R10",
        color: "red",
        title: "未纳入银行名单制管理",
        legal_basis: "金规〔2025〕9号第4条",
        types: &["loan"],
        keywords: &["名单制", "名单外", "未进入名单"],
    },
    CheckDef {
        id: "R11",
        color: "red",
        title: "支付牌照被注销/暂停",
        legal_basis: "央行支付业务许可相关规定",
        types: &["payment"],
        keywords: &["支付牌照", "注销支付", "暂停支付业务"],
    },
    CheckDef {
        id: "R12",
        color: "red",
        title: "数据违规被刑事立案",
        legal_basis: "《个人信息保护法》等",
        types: &["data"],
        keywords: &["数据违规", "侵犯公民个人信息", "非法提供数据"],
    },
    // 橙色重大违规
    CheckDef {
        id: "O01",
        color: "orange",
        title: "未自主风控 / 核心风控外包",
        legal_basis: "助贷新规第5条",
        types: &["loan"],
        keywords: &["风控外包", "未自主风控"],
    },
    CheckDef {
        id: "O02",
        color: "orange",
        title: "大额行政处罚（>100万）/ 监管公开通报",
        legal_basis: "负面惩罚清单",
        types: &[],
        keywords: &["行政处罚", "罚款", "公开通报", "处罚决定"],
    },
    CheckDef {
        id: "O03",
        color: "orange",
        title: "融担集中度超标 / 准备金不足 / 资产比例不合规 / 财务造假信号",
        legal_basis: "融担条例第16/18条、资产比例办法",
        types: &["guarantee"],
        keywords: &["集中度", "准备金", "资产比例", "财务造假"],
    },
    CheckDef {
        id: "O04",
        color: "orange",
        title: "出现违规催收行为",
        legal_basis: "助贷新规第8条",
        types: &["loan", "collection"],
        keywords: &["违规催收", "骚扰催收", "暴力催收投诉"],
    },
    CheckDef {
        id: "O05",
        color: "orange",
        title: "同业多家解约",
        legal_basis: "行业惯例",
        types: &[],
        keywords: &["取消合作", "终止合作", "解约"],
    },
    CheckDef {
        id: "O06",
        color: "orange",
        title: "数据来源合规性质疑 / 数据安全事件",
        legal_basis: "网信办相关规定",
        types: &["data", "traffic"],
        keywords: &["数据泄露", "数据安全", "约谈", "下架"],
    },
    CheckDef {
        id: "O07",
        color: "orange",
        title: "经营资质存续异常",
        legal_basis: "各监管官网",
        types: &[],
        keywords: &["续期受阻", "资质异常", "许可证", "备案撤销"],
    },
    CheckDef {
        id: "O08",
        color: "orange",
        title: "外部信用评级下调",
        legal_basis: "评级机构公告",
        types: &[],
        keywords: &["评级下调", "信用评级"],
    },
    CheckDef {
        id: "O09",
        color: "orange",
        title: "违规催收举报集中爆发（>50件/月）",
        legal_basis: "消保法、助贷新规第8条",
        types: &["collection"],
        keywords: &["批量举报", "投诉爆发", "催收举报"],
    },
    CheckDef {
        id: "O10",
        color: "orange",
        title: "备付金违规 / 反洗钱处罚",
        legal_basis: "央行相关规定",
        types: &["payment"],
        keywords: &["备付金", "反洗钱", "涉赌涉诈"],
    },
    // 黄色关注
    CheckDef {
        id: "Y01",
        color: "yellow",
        title: "不良率恶化 / 代偿率上升",
        legal_basis: "助贷/融担经营信号",
        types: &["loan", "guarantee"],
        keywords: &["不良率", "代偿率", "资产质量"],
    },
    CheckDef {
        id: "Y02",
        color: "yellow",
        title: "营收连续下滑",
        legal_basis: "财务异常信号",
        types: &[],
        keywords: &["营收下滑", "同比下降", "亏损"],
    },
    CheckDef {
        id: "Y03",
        color: "yellow",
        title: "非标审计意见 / 管理层变更",
        legal_basis: "融担监管条例等",
        types: &[],
        keywords: &["非标意见", "保留意见", "管理层变更", "实控人变更"],
    },
    CheckDef {
        id: "Y04",
        color: "yellow",
        title: "大额诉讼（>净资产5%）",
        legal_basis: "通用风险点",
        types: &[],
        keywords: &["重大诉讼", "涉案", "被告"],
    },
    CheckDef {
        id: "Y05",
        color: "yellow",
        title: "监管评级 C/D 级",
        legal_basis: "分类监管办法",
        types: &["guarantee"],
        keywords: &["监管评级", "C级", "D级"],
    },
    CheckDef {
        id: "Y06",
        color: "yellow",
        title: "催收投诉异常增长 / 消保约谈",
        legal_basis: "消保法",
        types: &["collection", "loan"],
        keywords: &["黑猫投诉", "投诉量", "约谈"],
    },
    CheckDef {
        id: "Y07",
        color: "yellow",
        title: "经营异常名录",
        legal_basis: "《企业经营异常名录管理暂行办法》",
        types: &[],
        keywords: &["经营异常", "异常名录"],
    },
];

pub fn run_review(
    conn: &Connection,
    req: &AdmissionReviewRequest,
) -> Result<AdmissionReview, String> {
    ensure_schema(conn)?;

    let (partner_id, partner_name, partner_type, partner_type_label, uscc) =
        resolve_partner(conn, req)?;

    let mut evidence_sources = Vec::new();
    let mut docs: Vec<EvidenceDoc> = Vec::new();

    // 1) 本地风险线索
    let clues = db::list_clues(conn)?
        .into_iter()
        .filter(|c| {
            (!partner_id.is_empty() && c.partner_id == partner_id)
                || c.partner.contains(&partner_name)
                || partner_name.contains(&c.partner)
        })
        .collect::<Vec<_>>();
    if !clues.is_empty() {
        evidence_sources.push("本地线索".into());
        for c in &clues {
            docs.push(EvidenceDoc {
                source: "本地线索".into(),
                text: format!("{} {} {}", c.title, c.summary, c.legal_basis),
                ref_url: c.source_url.clone(),
                level: c.level,
            });
        }
    }

    // 2) 本次上传材料 + 知识库
    if let Some(mat) = req.materials_text.as_ref().filter(|s| !s.trim().is_empty()) {
        evidence_sources.push("上传材料".into());
        docs.push(EvidenceDoc {
            source: "上传材料".into(),
            text: mat.chars().take(20000).collect(),
            ref_url: String::new(),
            level: 2,
        });
    }
    let knowledge = list_knowledge(conn, 30)?;
    if !knowledge.is_empty() {
        evidence_sources.push("知识库".into());
        for k in &knowledge {
            docs.push(EvidenceDoc {
                source: format!("知识库·{}", kind_label(&k.kind)),
                text: format!("{} {}", k.title, k.content),
                ref_url: String::new(),
                level: 2,
            });
        }
    }

    // 3) Tavily 全网搜（先跑，通常比工商 MCP 更快出结果；最多 2 次 HTTP）
    let use_web = req.use_web_search.unwrap_or(true);
    if use_web {
        if let Ok(tav) = crate::tavily::load_config(conn) {
            if tav.ready() {
                let (web_docs, web_labels, web_err) =
                    fetch_web_docs(&tav.api_key, &partner_name);
                for lab in web_labels {
                    if !evidence_sources.contains(&lab) {
                        evidence_sources.push(lab);
                    }
                }
                docs.extend(web_docs);
                if let Some(e) = web_err {
                    docs.push(EvidenceDoc {
                        source: "全网搜".into(),
                        text: format!("全网搜提示：{e}"),
                        ref_url: String::new(),
                        level: 3,
                    });
                }
            }
        }
    }

    // 4) 企查查 / 天眼查（短超时 + 单会话，失败不阻塞整份报告）
    let use_mcp = req.use_enterprise_mcp.unwrap_or(true);
    if use_mcp {
        let mcp = enterprise_mcp::load_config(conn)?;
        let (mcp_docs, mcp_labels) = fetch_enterprise_docs_fast(&mcp, &partner_name, &uscc);
        for lab in mcp_labels {
            if !evidence_sources.contains(&lab) {
                evidence_sources.push(lab);
            }
        }
        docs.extend(mcp_docs);
    }

    let boosts = load_keyword_boosts(conn)?;

    let mut items = Vec::new();
    for def in CHECKS {
        if !type_match(def.types, &partner_type) {
            continue;
        }
        let (triggered, evidence, source) = match_evidence(def, &docs, &boosts);
        let override_hit = req
            .manual_flags
            .as_ref()
            .and_then(|m| m.get(def.id).copied());
        let triggered = override_hit.unwrap_or(triggered);
        let evidence = if override_hit == Some(true) && evidence.is_empty() {
            "人工勾选触发".into()
        } else {
            evidence
        };

        items.push(AdmissionCheckItem {
            id: def.id.into(),
            color: def.color.into(),
            title: def.title.into(),
            legal_basis: def.legal_basis.into(),
            triggered,
            evidence,
            source_ref: source,
        });
    }

    let red_hit = items.iter().any(|i| i.color == "red" && i.triggered);
    let orange_hits: Vec<&AdmissionCheckItem> = items
        .iter()
        .filter(|i| i.color == "orange" && i.triggered)
        .collect();
    let yellow_hits: Vec<&AdmissionCheckItem> = items
        .iter()
        .filter(|i| i.color == "yellow" && i.triggered)
        .collect();

    let evidence_incomplete = docs.is_empty();
    let has_external = evidence_sources.iter().any(|s| {
        s.contains("企查查") || s.contains("天眼查") || s.contains("全网搜")
    });
    // 融担核心经营指标若证据中完全未出现，不得绿灯
    let guarantee_thin = partner_type == "guarantee"
        && !docs.iter().any(|d| {
            d.text.contains("放大倍数")
                || d.text.contains("代偿率")
                || d.text.contains("担保赔偿准备金")
                || d.text.contains("在保余额")
        });

    let conclusion = if red_hit {
        "否决".to_string()
    } else if !orange_hits.is_empty() {
        "有条件通过".to_string()
    } else if evidence_incomplete || !has_external || guarantee_thin {
        // 无外部取证 / 融担关键指标空白 → 待核验，禁止假通过
        "待核验".to_string()
    } else {
        "通过".to_string()
    };

    let mut remediation = Vec::new();
    for i in &orange_hits {
        remediation.push(format!("限期整改：{}（依据：{}）", i.title, i.legal_basis));
    }
    for i in &yellow_hits {
        remediation.push(format!("持续监控：{}", i.title));
    }
    if evidence_incomplete || !has_external {
        remediation.push(
            "证据不足：请配置 Tavily 全网搜或企查查/天眼查，或粘贴尽调材料后再签批".into(),
        );
    }
    if guarantee_thin {
        remediation.push(
            "融担核心指标待核验：请补齐放大倍数、代偿率、准备金/在保余额等经营数据后再研判".into(),
        );
    }
    if conclusion == "待核验" {
        remediation.push("结论为「待核验」：禁止按「通过」推进业务合作，须补齐证据后重跑".into());
    }

    let mut summary = format!(
        "{}（{}）场景「{}」：一票否决 {} 项触发，重大违规 {} 项，关注 {} 项 → 结论【{}】{}",
        partner_name,
        partner_type_label,
        req.scenario,
        items.iter().filter(|i| i.color == "red" && i.triggered).count(),
        orange_hits.len(),
        yellow_hits.len(),
        conclusion,
        if evidence_incomplete {
            "（证据不完整）"
        } else {
            ""
        }
    );

    let mut ai_notes = String::new();
    if req.use_llm_assist.unwrap_or(true) {
        if let Ok(cfg) = llm::load_config(conn) {
            if cfg.is_ready() {
                match llm_admission_notes(&cfg, &partner_name, &partner_type_label, &req.scenario, &items, &docs) {
                    Ok(notes) => {
                        if !notes.is_empty() {
                            ai_notes = notes;
                            summary = format!("{summary}\nAI 辅助：{}", ai_notes.chars().take(120).collect::<String>());
                        }
                    }
                    Err(e) => {
                        ai_notes = format!("AI 辅助未完成：{e}");
                    }
                }
            }
        }
    }

    let markdown = render_markdown(
        &partner_name,
        &partner_type_label,
        &uscc,
        &req.scenario,
        &conclusion,
        evidence_incomplete,
        &items,
        &remediation,
        &summary,
        &evidence_sources,
        &ai_notes,
    );

    let review = AdmissionReview {
        id: format!("AD-{}", &Uuid::new_v4().to_string()[..8]),
        partner_id,
        partner_name,
        partner_type,
        uscc,
        scenario: req.scenario.clone(),
        conclusion,
        items,
        remediation,
        summary,
        markdown,
        evidence_incomplete,
        evidence_sources,
        ai_notes,
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    save_review(conn, &review)?;
    db::insert_audit(
        conn,
        &crate::models::AuditLog {
            id: Uuid::new_v4().to_string(),
            actor: "user".into(),
            action: "admission_review".into(),
            target: review.id.clone(),
            detail: format!(
                "conclusion={} sources={:?} red={} orange={}",
                review.conclusion,
                review.evidence_sources,
                review.items.iter().filter(|i| i.color == "red" && i.triggered).count(),
                review.items.iter().filter(|i| i.color == "orange" && i.triggered).count()
            ),
            ts: review.created_at.clone(),
        },
    )?;

    Ok(review)
}

fn resolve_partner(
    conn: &Connection,
    req: &AdmissionReviewRequest,
) -> Result<(String, String, String, String, String), String> {
    if let Some(pid) = &req.partner_id {
        if !pid.is_empty() {
            let p = db::list_partners(conn)?
                .into_iter()
                .find(|x| x.id == *pid)
                .ok_or_else(|| format!("机构不存在: {pid}"))?;
            let uscc = req
                .uscc
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(p.uscc.clone());
            return Ok((
                p.id,
                p.name,
                p.partner_type,
                p.partner_type_label,
                uscc,
            ));
        }
    }
    let name = req
        .partner_name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .ok_or("请选择机构或填写拟合作机构名称")?;
    let ptype = req
        .partner_type
        .clone()
        .unwrap_or_else(|| "loan".into());
    let label = crate::finance::partner_type_label(&ptype);
    let uscc = req.uscc.clone().unwrap_or_default();
    Ok((String::new(), name, ptype, label, uscc))
}

fn type_match(allowed: &[&str], partner_type: &str) -> bool {
    allowed.is_empty() || allowed.iter().any(|t| *t == partner_type)
}

fn match_evidence(
    def: &CheckDef,
    docs: &[EvidenceDoc],
    boosts: &HashMap<String, Vec<String>>,
) -> (bool, String, String) {
    let mut kws: Vec<String> = def.keywords.iter().map(|s| (*s).to_string()).collect();
    if let Some(extra) = boosts.get(def.id) {
        kws.extend(extra.clone());
    }
    if let Some(extra) = boosts.get("*") {
        kws.extend(extra.clone());
    }

    for d in docs {
        for kw in &kws {
            if kw.is_empty() {
                continue;
            }
            if d.text.contains(kw.as_str()) {
                if def.color == "red" && d.level > 2 && d.source == "本地线索" {
                    continue;
                }
                return (
                    true,
                    format!("[{}] 命中「{kw}」：{}", d.source, truncate(&d.text, 80)),
                    d.ref_url.clone(),
                );
            }
        }
    }
    (false, String::new(), String::new())
}

fn load_keyword_boosts(conn: &Connection) -> Result<HashMap<String, Vec<String>>, String> {
    ensure_schema(conn)?;
    let mut stmt = conn
        .prepare("SELECT check_id, keyword FROM admission_keyword_boosts")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        let (cid, kw) = row.map_err(|e| e.to_string())?;
        let key = if cid.is_empty() { "*".into() } else { cid };
        map.entry(key).or_default().push(kw);
    }
    Ok(map)
}

/// 准入专用：短超时、单会话、最多 2 次工具参数尝试，避免旧逻辑「每次 call 重开 MCP + 6 组参数 × 90s」卡死。
fn fetch_enterprise_docs_fast(
    cfg: &EnterpriseMcpConfig,
    name: &str,
    uscc: &str,
) -> (Vec<EvidenceDoc>, Vec<String>) {
    use crate::mcp_http::normalize_auth;

    let mut docs = Vec::new();
    let mut labels = Vec::new();
    let query = if !uscc.trim().is_empty() {
        uscc.trim()
    } else {
        name.trim()
    };

    if cfg.qcc_ready() {
        match probe_mcp_session(
            "https://agent.qcc.com/mcp/company/stream",
            &normalize_auth("qcc", &cfg.qcc_api_key),
            query,
            name,
        ) {
            Ok(text) => {
                labels.push("企查查".into());
                docs.push(EvidenceDoc {
                    source: "企查查·company".into(),
                    text,
                    ref_url: String::new(),
                    level: 1,
                });
            }
            Err(e) => {
                docs.push(EvidenceDoc {
                    source: "企查查".into(),
                    text: format!("企查查补证未完成（已跳过，不阻塞报告）：{e}"),
                    ref_url: String::new(),
                    level: 3,
                });
            }
        }
    }

    if cfg.tyc_ready() {
        let auth = normalize_auth("tyc", &cfg.tyc_api_key);
        let tyc = match probe_mcp_session(enterprise_mcp::TYC_MCP_URL, &auth, query, name) {
            Ok(text) => Ok(text),
            Err(e1) => {
                let auth2 = format!("Bearer {}", cfg.tyc_api_key.trim());
                probe_mcp_session(enterprise_mcp::TYC_MCP_URL, &auth2, query, name).map_err(|e2| {
                    format!("{e1}；Bearer 重试: {e2}")
                })
            }
        };
        match tyc {
            Ok(text) => {
                labels.push("天眼查".into());
                docs.push(EvidenceDoc {
                    source: "天眼查".into(),
                    text,
                    ref_url: String::new(),
                    level: 1,
                });
            }
            Err(e) => {
                docs.push(EvidenceDoc {
                    source: "天眼查".into(),
                    text: format!("天眼查补证未完成（已跳过，不阻塞报告）：{e}"),
                    ref_url: String::new(),
                    level: 3,
                });
            }
        }
    }

    (docs, labels)
}

fn probe_mcp_session(
    url: &str,
    authorization: &str,
    query: &str,
    name: &str,
) -> Result<String, String> {
    use crate::mcp_http::McpHttpClient;

    let mut client = McpHttpClient::with_timeouts(url, authorization, 8, 25)?;
    client.initialize()?;
    let tools = client.list_tools()?;
    let preferred = tools.iter().find(|t| {
        let n = t.name.to_lowercase();
        n.contains("basic")
            || n.contains("search")
            || n.contains("info")
            || n.contains("company")
            || n.contains("risk")
            || n.contains("get")
    });
    let tool = preferred.or_else(|| tools.first()).ok_or("无可用工具")?;

    // 最多 2 组参数；同一会话内调用，避免反复 initialize
    let arg_variants = [
        json!({"keyword": query}),
        json!({"searchKey": query}),
        json!({"name": name}),
    ];
    let mut last_err = String::new();
    for args in arg_variants.into_iter().take(2) {
        match client.call_tool(&tool.name, args) {
            Ok(v) => {
                let s = summarize_mcp_value(&v);
                if s.len() > 20 {
                    return Ok(truncate(&s, 6000));
                }
                last_err = "返回内容过短".into();
            }
            Err(e) => last_err = e,
        }
    }
    Err(if last_err.is_empty() {
        "工商 MCP 无有效返回".into()
    } else {
        last_err
    })
}

fn summarize_mcp_value(v: &serde_json::Value) -> String {
    if let Some(content) = v.get("content").and_then(|c| c.as_array()) {
        let mut parts = Vec::new();
        for item in content {
            if let Some(t) = item.get("text").and_then(|x| x.as_str()) {
                parts.push(t.to_string());
            } else {
                parts.push(item.to_string());
            }
        }
        if !parts.is_empty() {
            return parts.join("\n");
        }
    }
    v.to_string()
}

fn fetch_web_docs(
    api_key: &str,
    name: &str,
) -> (Vec<EvidenceDoc>, Vec<String>, Option<String>) {
    use crate::tavily::{search_hits_with_opts, SearchOpts};

    let mut docs = Vec::new();
    let mut labels = Vec::new();
    let mut err_msg = None;
    let name = name.trim();
    if name.is_empty() || api_key.trim().is_empty() {
        return (docs, labels, Some("公司名为空或未配置 Tavily".into()));
    }

    let queries = [
        format!("\"{name}\" (处罚 OR 失信被执行人 OR 被执行 OR 投诉 OR 违规 OR 诉讼 OR 开庭)"),
        format!("\"{name}\" (融资担保 OR 工商 OR 变更 OR 注册资本 OR 许可证)"),
    ];
    let opts = SearchOpts {
        max_results: 5,
        search_depth: "basic",
        time_range: None,
        start_date: None,
        drop_before_date: None,
    };

    for (i, q) in queries.iter().enumerate() {
        if i as u32 >= crate::tavily::MAX_CALLS_PER_OPERATION {
            break;
        }
        match search_hits_with_opts(api_key, q, &opts) {
            Ok(hits) => {
                if !hits.is_empty() {
                    labels.push("全网搜".into());
                }
                for h in hits {
                    docs.push(EvidenceDoc {
                        source: "全网搜".into(),
                        text: format!("{} {} {}", h.title, h.summary, h.body),
                        ref_url: h.url,
                        level: match h.credibility.as_str() {
                            "A" => 1,
                            "B" => 2,
                            _ => 3,
                        },
                    });
                }
            }
            Err(e) => {
                err_msg = Some(e);
            }
        }
    }
    labels.sort();
    labels.dedup();
    (docs, labels, err_msg)
}

fn llm_admission_notes(
    cfg: &llm::LlmConfig,
    name: &str,
    ptype: &str,
    scenario: &str,
    items: &[AdmissionCheckItem],
    docs: &[EvidenceDoc],
) -> Result<String, String> {
    let red_n = items.iter().filter(|i| i.color == "red" && i.triggered).count();
    let orange_n = items.iter().filter(|i| i.color == "orange" && i.triggered).count();
    let yellow_n = items.iter().filter(|i| i.color == "yellow" && i.triggered).count();
    let triggered: Vec<_> = items
        .iter()
        .filter(|i| i.triggered)
        .map(|i| format!("{}[{}] {} | {}", i.id, i.color, i.title, truncate(&i.evidence, 100)))
        .collect();
    let untriggered_critical: Vec<_> = items
        .iter()
        .filter(|i| {
            !i.triggered
                && (i.color == "red"
                    || matches!(
                        i.id.as_str(),
                        "O02" | "O03" | "O07" | "Y01" | "Y03" | "Y04" | "Y05"
                    ))
        })
        .take(8)
        .map(|i| format!("{} {}", i.id, i.title))
        .collect();
    let evidence_brief: String = docs
        .iter()
        .take(10)
        .map(|d| format!("- [{}] {}", d.source, truncate(&d.text, 280)))
        .collect::<Vec<_>>()
        .join("\n");
    let system = r#"你是消费金融「合作机构准入」合规分析师。根据清单触发结果与证据，输出一份简明中文分析报告（可用小标题，不要用 markdown 代码块）。

必须包含：
1）主体速览（名称、类型、能从证据读到的工商/许可要点）
2）风险点统计：红/橙/黄各多少；并列出已触发项的具体含义
3）待核验项：证据不足、无法确认的关键检查（尤其融担杠杆/代偿/准备金、完整司法处罚清单）
4）结论建议：否决 / 有条件通过 / 待核验 / 通过 之一，并说明理由
硬约束：只依据给定材料，禁止编造案号、处罚决定书、失信名单；不确定就写「待核验」。"#;
    let user = format!(
        "机构：{name}\n类型：{ptype}\n场景：{scenario}\n统计：红{red_n}/橙{orange_n}/黄{yellow_n}\n已触发：\n{}\n关键未触发（待核实）：\n{}\n证据摘要：\n{evidence_brief}",
        if triggered.is_empty() {
            "（无）".into()
        } else {
            triggered.join("\n")
        },
        if untriggered_critical.is_empty() {
            "（无）".into()
        } else {
            untriggered_critical.join("\n")
        }
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
    Ok(reply.content.unwrap_or_default().trim().to_string())
}

pub fn import_knowledge(
    conn: &Connection,
    req: &AdmissionKnowledgeImportRequest,
) -> Result<AdmissionKnowledge, String> {
    ensure_schema(conn)?;
    let kind = normalize_kind(&req.kind)?;
    let title = req
        .title
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| kind_label(&kind).into());
    let content = req.content.trim();
    if content.is_empty() {
        return Err("材料内容不能为空".into());
    }
    let item = AdmissionKnowledge {
        id: format!("AK-{}", &Uuid::new_v4().to_string()[..8]),
        kind,
        title,
        content: content.chars().take(50000).collect(),
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };
    conn.execute(
        "INSERT INTO admission_knowledge (id, kind, title, content, created_at) VALUES (?1,?2,?3,?4,?5)",
        params![item.id, item.kind, item.title, item.content, item.created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(item)
}

pub fn list_knowledge(conn: &Connection, limit: u32) -> Result<Vec<AdmissionKnowledge>, String> {
    ensure_schema(conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, title, content, created_at FROM admission_knowledge
             ORDER BY created_at DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit as i64], |r| {
            Ok(AdmissionKnowledge {
                id: r.get(0)?,
                kind: r.get(1)?,
                title: r.get(2)?,
                content: r.get(3)?,
                created_at: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// 录入监管案例/行业风险事件：入库知识库，并用 LLM 抽取关键词增强检查项匹配。
pub fn ingest_case(
    conn: &Connection,
    req: &AdmissionCaseIngestRequest,
) -> Result<AdmissionCaseIngestResult, String> {
    ensure_schema(conn)?;
    let content = req.content.trim();
    if content.is_empty() {
        return Err("案例内容不能为空".into());
    }
    let title = req
        .title
        .clone()
        .unwrap_or_else(|| "监管/风险案例".into());
    let kn = import_knowledge(
        conn,
        &AdmissionKnowledgeImportRequest {
            kind: "risk_case".into(),
            title: Some(title.clone()),
            content: content.into(),
        },
    )?;

    let mut keywords = Vec::new();
    if let Ok(cfg) = llm::load_config(conn) {
        if cfg.is_ready() {
            if let Ok(extracted) = llm_extract_keywords(&cfg, content) {
                keywords = extracted;
            }
        }
    }
    if keywords.is_empty() {
        // 兜底：取较长中文片段
        for part in content.split(|c: char| c.is_whitespace() || "，。；、".contains(c)) {
            let p = part.trim();
            if p.chars().count() >= 4 && p.chars().count() <= 16 {
                keywords.push(p.to_string());
            }
            if keywords.len() >= 6 {
                break;
            }
        }
    }

    let check_id = req.check_id.clone().unwrap_or_default();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    for kw in keywords.iter().take(12) {
        let _ = conn.execute(
            "INSERT INTO admission_keyword_boosts (id, check_id, keyword, source_title, created_at)
             VALUES (?1,?2,?3,?4,?5)",
            params![
                Uuid::new_v4().to_string(),
                check_id,
                kw,
                title,
                now
            ],
        );
    }

    Ok(AdmissionCaseIngestResult {
        ok: true,
        message: format!(
            "已入库案例「{title}」，新增匹配关键词 {} 个，后续研判将自动生效",
            keywords.len().min(12)
        ),
        keywords_added: keywords.into_iter().take(12).collect(),
        knowledge_id: Some(kn.id),
    })
}

fn llm_extract_keywords(cfg: &llm::LlmConfig, content: &str) -> Result<Vec<String>, String> {
    let system = r#"从监管案例/风险事件中抽取用于准入规则匹配的关键词，输出 JSON 数组，如 ["失信被执行人","暴力催收"]。只输出 JSON。"#;
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
            content: Some(content.chars().take(8000).collect()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
    ];
    let reply = llm::chat_completion_timeout_retry(cfg, &messages, None, 60, 1)?;
    let text = reply.content.unwrap_or_default();
    let start = text.find('[').ok_or("无数组")?;
    let end = text.rfind(']').ok_or("无数组结尾")?;
    let arr: Vec<String> = serde_json::from_str(&text[start..=end]).map_err(|e| e.to_string())?;
    Ok(arr.into_iter().filter(|s| !s.trim().is_empty()).collect())
}

fn normalize_kind(kind: &str) -> Result<String, String> {
    let k = kind.trim().to_lowercase();
    match k.as_str() {
        "partner_policy" | "管理办法" | "policy" => Ok("partner_policy".into()),
        "lending_reg" | "监管新规" | "reg" | "regulation" => Ok("lending_reg".into()),
        "audit_report" | "审计报告" | "audit" => Ok("audit_report".into()),
        "risk_case" | "风险案例" | "case" => Ok("risk_case".into()),
        _ => Err("kind 需为 partner_policy / lending_reg / audit_report / risk_case".into()),
    }
}

fn kind_label(kind: &str) -> &str {
    match kind {
        "partner_policy" => "管理办法",
        "lending_reg" => "监管新规",
        "audit_report" => "审计报告",
        "risk_case" => "风险案例",
        _ => kind,
    }
}

fn truncate(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        format!("{t}…")
    } else {
        t
    }
}

fn save_review(conn: &Connection, review: &AdmissionReview) -> Result<(), String> {
    conn.execute(
        "INSERT INTO admission_reviews
         (id, partner_id, partner_name, partner_type, scenario, conclusion, items_json,
          remediation_json, summary, markdown, evidence_incomplete, created_at,
          uscc, evidence_sources_json, ai_notes)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        params![
            review.id,
            review.partner_id,
            review.partner_name,
            review.partner_type,
            review.scenario,
            review.conclusion,
            serde_json::to_string(&review.items).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(&review.remediation).unwrap_or_else(|_| "[]".into()),
            review.summary,
            review.markdown,
            if review.evidence_incomplete { 1 } else { 0 },
            review.created_at,
            review.uscc,
            serde_json::to_string(&review.evidence_sources).unwrap_or_else(|_| "[]".into()),
            review.ai_notes,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_reviews(conn: &Connection, limit: u32) -> Result<Vec<AdmissionReview>, String> {
    ensure_schema(conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, partner_id, partner_name, partner_type, scenario, conclusion, items_json,
                    remediation_json, summary, markdown, evidence_incomplete, created_at,
                    uscc, evidence_sources_json, ai_notes
             FROM admission_reviews ORDER BY created_at DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, String>(9)?,
                r.get::<_, i64>(10)?,
                r.get::<_, String>(11)?,
                r.get::<_, String>(12).unwrap_or_default(),
                r.get::<_, String>(13).unwrap_or_else(|_| "[]".into()),
                r.get::<_, String>(14).unwrap_or_default(),
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for row in rows {
        let (id, pid, name, ptype, scenario, conclusion, ij, rj, summary, md, incomplete, ts, uscc, sources, ai) =
            row.map_err(|e| e.to_string())?;
        out.push(AdmissionReview {
            id,
            partner_id: pid,
            partner_name: name,
            partner_type: ptype,
            uscc,
            scenario,
            conclusion,
            items: serde_json::from_str(&ij).unwrap_or_default(),
            remediation: serde_json::from_str(&rj).unwrap_or_default(),
            summary,
            markdown: md,
            evidence_incomplete: incomplete != 0,
            evidence_sources: serde_json::from_str(&sources).unwrap_or_default(),
            ai_notes: ai,
            created_at: ts,
        });
    }
    Ok(out)
}

fn render_markdown(
    name: &str,
    ptype: &str,
    uscc: &str,
    scenario: &str,
    conclusion: &str,
    incomplete: bool,
    items: &[AdmissionCheckItem],
    remediation: &[String],
    summary: &str,
    sources: &[String],
    ai_notes: &str,
) -> String {
    let mut md = String::new();
    md.push_str(&format!("# 准入意见书 · {name}\n\n"));
    md.push_str(&format!(
        "- 机构类型：{ptype}\n- 统一社会信用代码：{}\n- 触发场景：{scenario}\n- 综合结论：**{conclusion}**{}\n- 证据来源：{}\n- 研判时间：{}\n\n",
        if uscc.is_empty() { "—" } else { uscc },
        if incomplete { "（证据不完整）" } else { "" },
        if sources.is_empty() {
            "无".into()
        } else {
            sources.join(" / ")
        },
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    md.push_str(&format!("## 摘要\n\n{summary}\n\n"));
    if !ai_notes.is_empty() {
        md.push_str(&format!("## AI 研判要点\n\n{ai_notes}\n\n"));
    }

    for (color, title) in [
        ("red", "一票否决项（红色）"),
        ("orange", "重大违规项（橙色）"),
        ("yellow", "关注事项（黄色）"),
    ] {
        md.push_str(&format!("## {title}\n\n"));
        for i in items.iter().filter(|x| x.color == color) {
            let mark = if i.triggered { "☑ 触发" } else { "☐ 未触发" };
            md.push_str(&format!(
                "- [{mark}] {} — {}\n",
                i.title, i.legal_basis
            ));
            if i.triggered && !i.evidence.is_empty() {
                md.push_str(&format!("  - 证据：{}\n", i.evidence));
            }
        }
        md.push('\n');
    }

    md.push_str("## 整改 / 监控清单\n\n");
    if remediation.is_empty() {
        md.push_str("- 无\n");
    } else {
        for r in remediation {
            md.push_str(&format!("- {r}\n"));
        }
    }
    md.push_str("\n## 签批区\n\n- 复核人：________\n- 意见：________\n- 日期：________\n");
    if conclusion == "待核验" || incomplete {
        md.push_str("\n> **警示**：证据不完整或结论为待核验时，不得按「通过」推进业务合作。\n");
    }
    md
}

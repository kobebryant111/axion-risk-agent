use crate::db;
use crate::llm;
use crate::models::{AuditLog, BatchResult, Partner, RiskClue};
use crate::rules::{self, EffectiveRule, GradeResult};
use crate::sources::{self, dual_match, evidence_hash, RawHit};
use crate::baidu_search;
use crate::baidu_search::SearchWindow;
use crate::whistle_ai::{self, AiGrade};
use rusqlite::Connection;
use std::collections::HashMap;
use uuid::Uuid;

pub fn run_whistle_batch(
    conn: &Connection,
    live_heimao: bool,
    actor: &str,
) -> Result<BatchResult, String> {
    run_whistle_batch_scoped(
        conn,
        live_heimao,
        actor,
        None,
        true,
        SearchWindow::LastDay,
    )
}

/// `partner_ids` 为空则扫描全部；`persist_cursor` 为 false 时不推进搜索轮转游标。
pub fn run_whistle_batch_scoped(
    conn: &Connection,
    live_heimao: bool,
    actor: &str,
    partner_ids: Option<&[String]>,
    persist_cursor: bool,
    window: SearchWindow,
) -> Result<BatchResult, String> {
    let _ = live_heimao;
    let all = db::list_partners(conn)?;
    if all.is_empty() {
        return Err("请先导入合作机构名单后再跑批".into());
    }
    let partners = filter_partners(&all, partner_ids)?;
    if partners.is_empty() {
        return Err("所选机构不在当前名单中，请重新选择".into());
    }
    let scoped = partner_ids.map(|ids| !ids.is_empty()).unwrap_or(false);
    let scope_note = if scoped {
        format!("指定 {} 家 / 名单共 {} 家", partners.len(), all.len())
    } else {
        format!("全部 {} 家", partners.len())
    };

    let rules = rules::load_effective_rules(conn)?;
    let risk_keywords = sources::all_risk_keywords(rules::keywords_yaml());
    let llm_cfg = llm::load_config(conn).unwrap_or(llm::LlmConfig {
        base_url: String::new(),
        api_key: String::new(),
        model: String::new(),
        enabled: false,
    });
    let search_cfg = baidu_search::load_config(conn).unwrap_or(baidu_search::BaiduSearchConfig {
        enabled: false,
        api_key: String::new(),
        use_in_batch: true,
    });

    // 1) 百度搜索：1 家两句问句；多家每家一句，不用 OR
    let bundled = if search_cfg.batch_ready() {
        Some(baidu_search::search_partners_bundled(
            conn,
            &search_cfg.api_key,
            &partners,
            10,
            persist_cursor,
            &window,
        )?)
    } else {
        None
    };
    let search_calls = bundled.as_ref().map(|b| b.calls).unwrap_or(0);
    let search_covered = bundled.as_ref().map(|b| b.partners_covered).unwrap_or(0);

    // 2) AI：对全网搜命中做研判分级（未配置/失败则空，后面回退规则）
    let partners_by_id: HashMap<String, &Partner> =
        partners.iter().map(|p| (p.id.clone(), p)).collect();
    let empty_hits = HashMap::new();
    let web_hits = bundled
        .as_ref()
        .map(|b| &b.hits_by_partner)
        .unwrap_or(&empty_hits);
    let (ai_grades, ai_note) = if bundled.is_some() && !web_hits.is_empty() {
        whistle_ai::grade_tavily_hits(&llm_cfg, &partners_by_id, web_hits)
    } else {
        (HashMap::new(), None)
    };
    let ai_graded = !ai_grades.is_empty();

    let mut raw_hits = 0u32;
    let mut matched = 0u32;
    let mut clues_upserted = 0u32;
    let mut level1 = 0u32;
    let mut level2 = 0u32;
    let mut source_errors = Vec::new();
    let mut new_clues = Vec::new();

    if let Some(ref b) = bundled {
        source_errors.extend(b.errors.iter().cloned());
    }
    if let Some(note) = ai_note {
        source_errors.push(note);
    }

    for partner in &partners {
        let mut hits = Vec::new();

        if let Some(hits_web) = web_hits.get(&partner.id) {
            hits.extend(hits_web.iter().cloned());
        }

        raw_hits += hits.len() as u32;

        for hit in hits {
            let clue = if hit.source_id == "baidu" || hit.source_id == "tavily" {
                let key = whistle_ai::item_key(&partner.id, &hit);
                let text = format!("{} {} {}", hit.title, hit.summary, hit.body);
                if !baidu_search::keep_search_hit(&hit.title, &text) {
                    continue;
                }
                if let Some(ai) = ai_grades.get(&key) {
                    if !ai.relevant {
                        continue;
                    }
                    clue_from_ai(partner, &hit, ai)
                        .or_else(|| process_hit(partner, &hit, &risk_keywords, &rules))
                        .or_else(|| Some(clue_from_unconfirmed_hit(partner, &hit)))
                } else {
                    process_hit(partner, &hit, &risk_keywords, &rules)
                        .or_else(|| Some(clue_from_unconfirmed_hit(partner, &hit)))
                }
            } else {
                process_hit(partner, &hit, &risk_keywords, &rules)
            };
            let Some(clue) = clue else {
                continue;
            };

            matched += 1;
            match db::upsert_clue(conn, &clue)? {
                "new" => {
                    clues_upserted += 1;
                    if clue.level == 1 {
                        level1 += 1;
                    }
                    if clue.level == 2 {
                        level2 += 1;
                    }
                    new_clues.push(clue);
                }
                "repeat" => {
                    let mut shown = clue;
                    shown.denoise_status = "repeat".into();
                    if let Ok(Some(id)) = db::clue_id_by_evidence_hash(conn, &shown.evidence_hash) {
                        shown.id = id;
                    }
                    new_clues.push(shown);
                }
                _ => {
                    new_clues.push(clue);
                }
            }
        }
    }

    let search_cap = if partners.len() <= 1 {
        baidu_search::MAX_CALLS_PER_OPERATION
    } else {
        baidu_search::MAX_CALLS_PER_BATCH
    };
    let mut scope_note = scope_note;
    if search_calls > 0 {
        scope_note = format!(
            "{scope_note} · 全网检索{search_calls}次、覆盖{search_covered}家"
        );
    }

    let detail = format!(
        "scope={} scanned={} raw={} matched={} upserted={} L1={} L2={} errors={} search={} window={} search_calls={}/{} search_partners={} ai_grade={} cursor={}",
        scope_note,
        partners.len(),
        raw_hits,
        matched,
        clues_upserted,
        level1,
        level2,
        source_errors.len(),
        if bundled.is_some() { "bundled" } else { "off" },
        window.hint(),
        search_calls,
        search_cap,
        search_covered,
        if ai_graded { "on" } else { "off" },
        if persist_cursor { "on" } else { "trial" }
    );
    db::insert_audit(
        conn,
        &AuditLog {
            id: Uuid::new_v4().to_string(),
            actor: actor.to_string(),
            action: "whistle_batch".into(),
            target: "risk_clues".into(),
            detail,
            ts: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        },
    )?;

    Ok(BatchResult {
        partners_scanned: partners.len() as u32,
        raw_hits,
        matched,
        clues_upserted,
        level1,
        level2,
        source_errors,
        clues: new_clues,
        scope_note,
        search_queries: bundled
            .as_ref()
            .map(|b| b.queries.clone())
            .unwrap_or_default(),
    })
}

fn filter_partners(
    all: &[Partner],
    partner_ids: Option<&[String]>,
) -> Result<Vec<Partner>, String> {
    let Some(ids) = partner_ids else {
        return Ok(all.to_vec());
    };
    if ids.is_empty() {
        return Ok(all.to_vec());
    }
    let set: std::collections::HashSet<&str> = ids
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if set.is_empty() {
        return Ok(all.to_vec());
    }
    let filtered: Vec<Partner> = all
        .iter()
        .filter(|p| set.contains(p.id.as_str()))
        .cloned()
        .collect();
    Ok(filtered)
}

fn clue_from_unconfirmed_hit(partner: &Partner, hit: &RawHit) -> RiskClue {
    let hash = evidence_hash(&hit.source_id, &hit.url, &hit.title, &hit.body);
    RiskClue {
        id: format!(
            "W{}-{}",
            chrono::Local::now().format("%m%d"),
            &Uuid::new_v4().to_string()[..8]
        ),
        partner_id: partner.id.clone(),
        partner: partner.name.clone(),
        partner_type: partner.partner_type_label.clone(),
        level: 4,
        title: hit.title.clone(),
        summary: format!("【检索命中】{}", hit.summary),
        event_date: hit.event_time.clone(),
        owner: "合作风控".into(),
        progress: 0,
        status: "跟踪中".into(),
        source_system: hit.source_id.clone(),
        source_url: hit.url.clone(),
        credibility: hit.credibility.clone(),
        rule_id: "HIT-KEEP".into(),
        rule_set_version: "runtime".into(),
        legal_basis: "待人工复核".into(),
        denoise_status: "web_hit".into(),
        related_party_flag: false,
        evidence_hash: hash,
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    }
}

fn clue_from_ai(partner: &Partner, hit: &RawHit, ai: &AiGrade) -> Option<RiskClue> {
    if !ai.relevant || !(1..=4).contains(&ai.level) {
        return None;
    }

    let level = ai.level;

    let summary = if ai.analysis.trim().is_empty() {
        format!("【AI·{}】{}", ai.risk_point, hit.summary)
    } else {
        format!("【AI·{}】{}", ai.risk_point, ai.analysis.trim())
    };

    let hash = evidence_hash(&hit.source_id, &hit.url, &hit.title, &hit.body);
    Some(RiskClue {
        id: format!(
            "W{}-{}",
            chrono::Local::now().format("%m%d"),
            &Uuid::new_v4().to_string()[..8]
        ),
        partner_id: partner.id.clone(),
        partner: partner.name.clone(),
        partner_type: partner.partner_type_label.clone(),
        level,
        title: hit.title.clone(),
        summary,
        event_date: hit.event_time.clone(),
        owner: "合作风控".into(),
        progress: 0,
        status: if level <= 2 {
            "待复核".into()
        } else {
            "跟踪中".into()
        },
        source_system: hit.source_id.clone(),
        source_url: hit.url.clone(),
        credibility: hit.credibility.clone(),
        rule_id: whistle_ai::AI_RULE_ID.into(),
        rule_set_version: whistle_ai::AI_RULE_SET_VERSION.into(),
        legal_basis: ai.legal_basis.clone(),
        denoise_status: "ai_grade".into(),
        related_party_flag: false,
        evidence_hash: hash,
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

fn process_hit(
    partner: &Partner,
    hit: &RawHit,
    risk_keywords: &[String],
    rules: &[EffectiveRule],
) -> Option<RiskClue> {
    let text = format!("{} {} {}", hit.title, hit.summary, hit.body);

    let (subject_terms, related_flag) = if let Some(rel) = &hit.related_party_term {
        (vec![rel.clone()], true)
    } else {
        (crate::partners::expand_watch_terms(&partner.name, &partner.alias, &partner.lexicon), false)
    };

    let has_subject = subject_terms.iter().any(|t| text.contains(t.as_str()));
    if !dual_match(&text, &subject_terms, risk_keywords) {
        // 黑猫指标规则：主体命中即可进入定级
        let metric_ok = has_subject && hit.heimao.is_some();
        if !metric_ok {
            return None;
        }
    }

    let grade = rules::grade_hit(&partner.partner_type, hit, &text, rules)?;
    Some(clue_from_grade(partner, hit, grade, related_flag))
}

fn clue_from_grade(
    partner: &Partner,
    hit: &RawHit,
    grade: GradeResult,
    related_flag: bool,
) -> RiskClue {
    let hash = evidence_hash(&hit.source_id, &hit.url, &hit.title, &hit.body);
    let denoise = if related_flag {
        "related_party"
    } else {
        "new"
    };

    RiskClue {
        id: format!(
            "W{}-{}",
            chrono::Local::now().format("%m%d"),
            &Uuid::new_v4().to_string()[..8]
        ),
        partner_id: partner.id.clone(),
        partner: partner.name.clone(),
        partner_type: partner.partner_type_label.clone(),
        level: grade.level,
        title: hit.title.clone(),
        summary: hit.summary.clone(),
        event_date: hit.event_time.clone(),
        owner: "合作风控".into(),
        progress: 0,
        status: if grade.level <= 2 {
            "待复核".into()
        } else {
            "跟踪中".into()
        },
        source_system: hit.source_id.clone(),
        source_url: hit.url.clone(),
        credibility: hit.credibility.clone(),
        rule_id: grade.rule_id,
        rule_set_version: grade.rule_set_version,
        legal_basis: grade.legal_basis,
        denoise_status: denoise.into(),
        related_party_flag: related_flag,
        evidence_hash: hash,
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::partners::build_lexicon;

    #[test]
    fn batch_produces_clues_from_imported_partners() {
        let db_path = std::env::temp_dir().join(format!("axiom-test-{}.db", uuid::Uuid::new_v4()));
        let conn = crate::db::open(&db_path).unwrap();

        let related = vec!["宁银催收服务有限公司".into()];
        let name = "宁银消费金融股份有限公司".to_string();
        let alias = "宁银消金".to_string();
        let p = Partner {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.clone(),
            alias: alias.clone(),
            group_name: "宁波银行".into(),
            uscc: "91330200MA2AGXXXX1".into(),
            partner_type: "loan".into(),
            partner_type_label: "助贷机构".into(),
            related_parties: related.clone(),
            status: "active".into(),
            lexicon: build_lexicon(&name, &alias, "宁波银行", "91330200MA2AGXXXX1", &related),
        };
        crate::db::upsert_partner(&conn, &p).unwrap();

        let res = run_whistle_batch(&conn, false, "test").unwrap();
        assert!(res.partners_scanned >= 1);
        assert!(res.scope_note.contains("全部"));

        let miss = run_whistle_batch_scoped(
            &conn,
            false,
            "test",
            Some(&["not-exist".to_string()]),
            false,
            crate::baidu_search::SearchWindow::LastDay,
        );
        assert!(miss.is_err());

        let only = run_whistle_batch_scoped(
            &conn,
            false,
            "test",
            Some(&[p.id.clone()]),
            false,
            crate::baidu_search::SearchWindow::LastDay,
        )
        .unwrap();
        assert_eq!(only.partners_scanned, 1);
        assert!(only.scope_note.contains("指定"));
        let _ = std::fs::remove_file(db_path);
    }
}

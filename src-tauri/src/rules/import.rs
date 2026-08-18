use crate::db;
use crate::models::{PartnerType, RuleEditInput, RuleView, RulesDocumentImportResult};
use crate::rules::{self, RuleDef};
use csv::ReaderBuilder;
use rusqlite::Connection;
use uuid::Uuid;

pub fn upsert_rule_edit(conn: &Connection, input: &RuleEditInput) -> Result<Vec<RuleView>, String> {
    let partner_type = normalize_partner_type(&input.partner_type)?;
    let def = RuleDef {
        id: input.id.clone(),
        risk_point: input.risk_point.clone(),
        level: input.level,
        trigger: input.trigger.clone(),
        data_source: input.data_source.clone(),
        match_any_keywords: input.match_any_keywords.clone(),
        metric: None,
        threshold: None,
        legal_basis: input.legal_basis.clone(),
        enabled: input.enabled,
    };
    if !(1..=4).contains(&def.level) {
        return Err("等级须为 1～4".into());
    }
    if def.risk_point.trim().is_empty() {
        return Err("风险点不能为空".into());
    }
    db::upsert_custom_rule(conn, &partner_type, &def)?;
    db::insert_audit(
        conn,
        &crate::models::AuditLog {
            id: Uuid::new_v4().to_string(),
            actor: "user".into(),
            action: "upsert_rule".into(),
            target: def.id.clone(),
            detail: format!("{} L{}", def.risk_point, def.level),
            ts: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        },
    )?;
    list_all(conn)
}

pub fn list_all(conn: &Connection) -> Result<Vec<RuleView>, String> {
    rules::list_rule_views_from_db(conn)
}

struct ExtractedRule {
    id: String,
    partner_type: String,
    risk_point: String,
    level: u8,
    trigger: Option<String>,
    data_source: Option<String>,
    legal_basis: Option<String>,
    keywords: Option<Vec<String>>,
    enabled: Option<bool>,
}

fn norm_header(h: &str) -> String {
    h.trim()
        .trim_start_matches('\u{feff}')
        .to_lowercase()
        .replace([' ', '_', '-', '\t'], "")
}

fn find_col(headers: &csv::StringRecord, names: &[&str]) -> Option<usize> {
    headers.iter().position(|h| {
        let l = norm_header(h);
        names.iter().any(|n| {
            let nn = norm_header(n);
            !nn.is_empty() && (l == nn || l.contains(&nn))
        })
    })
}

fn parse_level(raw: &str) -> Option<u8> {
    let s = raw.trim().to_uppercase().replace('L', "");
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    let n = digits.parse::<u8>().ok()?;
    if (1..=4).contains(&n) {
        Some(n)
    } else {
        None
    }
}

fn parse_enabled(raw: &str) -> bool {
    let s = raw.trim();
    if s.is_empty() {
        return true;
    }
    let lower = s.to_lowercase();
    !(matches!(
        lower.as_str(),
        "否" | "停用" | "禁用" | "false" | "0" | "n" | "no" | "off"
    ) || s.contains("否")
        || s.contains("停"))
}

fn parse_rules_csv(csv_text: &str) -> Result<(Vec<ExtractedRule>, Vec<String>), String> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(csv_text.as_bytes());

    let headers = rdr
        .headers()
        .map_err(|e| format!("无法解析表头: {e}"))?
        .clone();

    let i_id = find_col(&headers, &["规则编号", "id", "ruleid", "rule_id"]);
    let i_type = find_col(&headers, &["机构类型", "partnertype", "partner_type", "类型"]);
    let i_risk = find_col(&headers, &["风险点", "riskpoint", "risk_point"]);
    let i_level = find_col(&headers, &["等级", "level", "预警等级"]);
    let i_trigger = find_col(&headers, &["触发条件", "trigger"]);
    let i_kw = find_col(
        &headers,
        &["匹配关键词", "keywords", "matchanykeywords", "关键词"],
    );
    let i_src = find_col(&headers, &["数据源", "datasource", "data_source"]);
    let i_legal = find_col(&headers, &["法规依据", "legalbasis", "legal_basis", "依据"]);
    let i_enabled = find_col(&headers, &["是否启用", "enabled", "启用"]);

    if i_type.is_none() || i_risk.is_none() || i_level.is_none() {
        return Err(
            "请使用固定模板：须含「机构类型」「风险点」「等级」列（等级 1～4）".into(),
        );
    }

    let mut rules = Vec::new();
    let mut warnings = Vec::new();

    for (row_no, rec) in rdr.records().enumerate() {
        let line = row_no + 2;
        let rec = match rec {
            Ok(r) => r,
            Err(e) => {
                warnings.push(format!("第 {line} 行读取失败: {e}"));
                continue;
            }
        };
        let get = |i: Option<usize>| -> String {
            i.and_then(|ix| rec.get(ix).map(|s| s.trim().to_string()))
                .unwrap_or_default()
        };

        let risk_point = get(i_risk);
        if risk_point.is_empty() {
            continue;
        }

        let type_raw = get(i_type);
        let partner_type = match normalize_partner_type(&type_raw) {
            Ok(t) => t,
            Err(_) => {
                warnings.push(format!(
                    "第 {line} 行机构类型无法识别: {type_raw}（期望：通用/助贷/融资担保/引流/支付/数据/催收）"
                ));
                continue;
            }
        };

        let Some(level) = parse_level(&get(i_level)) else {
            warnings.push(format!(
                "第 {line} 行等级无法识别: {}（期望 1～4 或 L1～L4）",
                get(i_level)
            ));
            continue;
        };

        let kw_raw = get(i_kw);
        let keywords: Vec<String> = kw_raw
            .split(|c| c == ';' || c == '|' || c == ',' || c == '、' || c == '；')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        rules.push(ExtractedRule {
            id: get(i_id),
            partner_type,
            risk_point,
            level,
            trigger: {
                let t = get(i_trigger);
                if t.is_empty() {
                    None
                } else {
                    Some(t)
                }
            },
            data_source: {
                let t = get(i_src);
                if t.is_empty() {
                    None
                } else {
                    Some(t)
                }
            },
            legal_basis: {
                let t = get(i_legal);
                if t.is_empty() {
                    None
                } else {
                    Some(t)
                }
            },
            keywords: if keywords.is_empty() {
                None
            } else {
                Some(keywords)
            },
            enabled: Some(parse_enabled(&get(i_enabled))),
        });
    }

    if rules.is_empty() {
        if warnings.is_empty() {
            return Err("模板中没有有效规则行，请检查「规则库」工作表".into());
        }
        return Err(warnings.join("\n"));
    }

    Ok((rules, warnings))
}

pub fn import_from_csv(
    conn: &Connection,
    csv_text: &str,
    replace_all: bool,
) -> Result<RulesDocumentImportResult, String> {
    let (extracted, warnings) = parse_rules_csv(csv_text)?;
    let mut result = commit_extracted(conn, extracted, replace_all)?;
    if !warnings.is_empty() {
        let preview: Vec<String> = warnings.iter().take(5).cloned().collect();
        result
            .message
            .push_str(&format!("；跳过 {} 行（{}）", warnings.len(), preview.join("；")));
    }
    Ok(result)
}

fn commit_extracted(
    conn: &Connection,
    extracted: Vec<ExtractedRule>,
    replace_all: bool,
) -> Result<RulesDocumentImportResult, String> {
    if replace_all {
        db::clear_custom_rules(conn)?;
        db::clear_rule_overrides(conn)?;
        db::set_setting(conn, "rules_library_mode", "imported")?;
    }

    let mut upserted = 0u32;
    for item in extracted {
        let partner_type = normalize_partner_type(&item.partner_type).unwrap_or_else(|_| {
            if item.partner_type == "common" || item.partner_type.contains("通用") {
                "common".into()
            } else {
                "common".into()
            }
        });
        let id = if item.id.trim().is_empty() {
            format!(
                "XLS-{}-{}",
                partner_type.to_uppercase(),
                &Uuid::new_v4().to_string()[..8]
            )
        } else {
            item.id.trim().to_string()
        };
        let def = RuleDef {
            id,
            risk_point: item.risk_point,
            level: item.level.clamp(1, 4),
            trigger: item.trigger.unwrap_or_default(),
            data_source: item.data_source.unwrap_or_default(),
            match_any_keywords: item.keywords.unwrap_or_default(),
            metric: None,
            threshold: None,
            legal_basis: item
                .legal_basis
                .unwrap_or_else(|| "Excel导入".into()),
            enabled: item.enabled.unwrap_or(true),
        };
        db::upsert_custom_rule(conn, &partner_type, &def)?;
        upserted += 1;
    }

    db::insert_audit(
        conn,
        &crate::models::AuditLog {
            id: Uuid::new_v4().to_string(),
            actor: "user".into(),
            action: "import_rules_csv".into(),
            target: "rules".into(),
            detail: format!("upserted={upserted} replace={replace_all}"),
            ts: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        },
    )?;

    let rules = list_all(conn)?;
    Ok(RulesDocumentImportResult {
        ok: true,
        message: if replace_all {
            format!("已按 Excel 模板全量替换规则库，共 {upserted} 条")
        } else {
            format!("已导入 {upserted} 条规则")
        },
        upserted,
        rules,
    })
}

pub fn normalize_partner_type(raw: &str) -> Result<String, String> {
    let s = raw.trim();
    if s == "common" || s == "通用" || s == "跨机构" {
        return Ok("common".into());
    }
    PartnerType::parse(s)
        .map(|p| p.as_str().to_string())
        .ok_or_else(|| format!("无法识别类型: {raw}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fixed_rule_template() {
        let csv = "规则编号,机构类型,风险点,等级,触发条件,匹配关键词,数据源,法规依据,是否启用\n\
LOAN-1,助贷,综合融资成本超过24%,1,宣传口径超24%,综合融资成本|超24%,舆情,金规9号,是\n";
        let (rules, warn) = parse_rules_csv(csv).unwrap();
        assert!(warn.is_empty());
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].partner_type, "loan");
        assert_eq!(rules[0].level, 1);
        assert_eq!(rules[0].enabled, Some(true));
        assert_eq!(
            rules[0].keywords.as_ref().unwrap(),
            &vec!["综合融资成本".to_string(), "超24%".to_string()]
        );
    }
}

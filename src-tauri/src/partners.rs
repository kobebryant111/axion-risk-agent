use crate::db;
use crate::models::{
    ImportResult, Partner, PartnerEditInput, PartnerType, PartnersDocumentImportResult,
};
use csv::ReaderBuilder;
use rusqlite::Connection;
use uuid::Uuid;

/// Build monitoring lexicon from name / alias / group / uscc / related parties.
pub fn build_lexicon(
    name: &str,
    alias: &str,
    group_name: &str,
    uscc: &str,
    related: &[String],
) -> Vec<String> {
    let mut words = Vec::new();
    for w in [name, alias, group_name, uscc] {
        let t = w.trim();
        if !t.is_empty() && !words.iter().any(|x: &String| x == t) {
            words.push(t.to_string());
        }
    }
    for r in related {
        let t = r.trim();
        if !t.is_empty() && !words.iter().any(|x: &String| x == t) {
            words.push(t.to_string());
        }
    }
    words
}

pub fn parse_coop_status(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() {
        return "active".into();
    }
    let lower = s.to_lowercase();
    if lower == "paused" || s.contains("暂停") {
        return "paused".into();
    }
    if lower == "exit"
        || s.contains("退出")
        || s.contains("终止")
        || s.contains("无效")
    {
        return "exit".into();
    }
    if lower == "active" || s.contains("合作") || s.contains("有效") {
        return "active".into();
    }
    if lower == "paused" || lower == "exit" || lower == "active" {
        return lower;
    }
    "active".into()
}

pub fn partner_from_edit(input: &PartnerEditInput) -> Result<Partner, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("机构全称不能为空".into());
    }
    let ptype = PartnerType::parse(&input.partner_type)
        .ok_or_else(|| format!("无法识别机构类型: {}", input.partner_type))?;
    let related_parties: Vec<String> = input
        .related_parties
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let alias = input.alias.trim().to_string();
    let group_name = input.group_name.trim().to_string();
    let uscc = input.uscc.trim().to_string();
    let status = parse_coop_status(&input.status);
    let lexicon = build_lexicon(&name, &alias, &group_name, &uscc, &related_parties);
    let id = if input.id.trim().is_empty() {
        Uuid::new_v4().to_string()
    } else {
        input.id.trim().to_string()
    };
    Ok(Partner {
        id,
        name,
        alias,
        group_name,
        uscc,
        partner_type: ptype.as_str().to_string(),
        partner_type_label: ptype.label().to_string(),
        related_parties,
        status,
        lexicon,
    })
}

pub fn upsert_partner_edit(
    conn: &Connection,
    input: &PartnerEditInput,
) -> Result<Vec<Partner>, String> {
    let p = partner_from_edit(input)?;
    db::upsert_partner(conn, &p)?;
    db::insert_audit(
        conn,
        &crate::models::AuditLog {
            id: Uuid::new_v4().to_string(),
            actor: "user".into(),
            action: "upsert_partner".into(),
            target: p.id.clone(),
            detail: format!("{} ({})", p.name, p.partner_type_label),
            ts: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        },
    )?;
    db::list_partners(conn)
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

/// 固定模板列：机构全称、简称、集团、统一社会信用代码、机构类型、关联方、合作状态
pub fn parse_partners_csv(csv_text: &str) -> ImportResult {
    let mut imported = 0u32;
    let mut skipped = 0u32;
    let mut errors = Vec::new();
    let mut partners = Vec::new();

    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(csv_text.as_bytes());

    let headers = match rdr.headers() {
        Ok(h) => h.clone(),
        Err(e) => {
            return ImportResult {
                imported: 0,
                updated: 0,
                skipped: 0,
                errors: vec![format!("无法解析表头: {e}")],
                partners: vec![],
            };
        }
    };

    let i_name = find_col(&headers, &["机构全称", "name", "全称", "机构名称", "partnername"]);
    let i_alias = find_col(&headers, &["alias", "简称"]);
    let i_group = find_col(&headers, &["groupname", "group_name", "集团", "集团名"]);
    let i_uscc = find_col(&headers, &["uscc", "统一社会信用代码", "信用代码"]);
    let i_type = find_col(
        &headers,
        &["机构类型", "partnertype", "partner_type", "类型"],
    );
    let i_related = find_col(&headers, &["relatedparties", "related_parties", "关联方"]);
    let i_status = find_col(&headers, &["status", "合作状态", "状态"]);

    if i_name.is_none() || i_type.is_none() {
        return ImportResult {
            imported: 0,
            updated: 0,
            skipped: 0,
            errors: vec![
                "请使用固定模板：须含「机构全称」与「机构类型」列（助贷/融资担保/引流/支付/数据/催收）"
                    .into(),
            ],
            partners: vec![],
        };
    }

    for (row_no, rec) in rdr.records().enumerate() {
        let line = row_no + 2;
        let rec = match rec {
            Ok(r) => r,
            Err(e) => {
                skipped += 1;
                errors.push(format!("第 {line} 行读取失败: {e}"));
                continue;
            }
        };

        let get = |i: Option<usize>| -> String {
            i.and_then(|ix| rec.get(ix).map(|s| s.trim().to_string()))
                .unwrap_or_default()
        };

        let name = get(i_name);
        if name.is_empty() {
            skipped += 1;
            continue;
        }

        let type_raw = get(i_type);
        let Some(ptype) = PartnerType::parse(&type_raw) else {
            skipped += 1;
            errors.push(format!(
                "第 {line} 行机构类型无法识别: {type_raw}（期望：助贷/融资担保/引流/支付/数据/催收）"
            ));
            continue;
        };

        let alias = get(i_alias);
        let group_name = get(i_group);
        let uscc = get(i_uscc);
        let related_raw = get(i_related);
        let related_parties: Vec<String> = related_raw
            .split(|c| c == ';' || c == '|' || c == ',' || c == '、' || c == '；')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let status = parse_coop_status(&get(i_status));

        let lexicon = build_lexicon(&name, &alias, &group_name, &uscc, &related_parties);
        partners.push(Partner {
            id: Uuid::new_v4().to_string(),
            name,
            alias,
            group_name,
            uscc,
            partner_type: ptype.as_str().to_string(),
            partner_type_label: ptype.label().to_string(),
            related_parties,
            status,
            lexicon,
        });
        imported += 1;
    }

    ImportResult {
        imported,
        updated: 0,
        skipped,
        errors,
        partners,
    }
}

pub fn import_from_csv(
    conn: &Connection,
    csv_text: &str,
    replace_all: bool,
) -> Result<PartnersDocumentImportResult, String> {
    let parsed = parse_partners_csv(csv_text);
    if parsed.partners.is_empty() {
        let hint = if parsed.errors.is_empty() {
            "模板中没有有效机构行，请检查「机构名单」工作表".to_string()
        } else {
            parsed.errors.join("\n")
        };
        return Err(hint);
    }

    if replace_all {
        db::clear_partners(conn)?;
    }

    let mut upserted = 0u32;
    for p in &parsed.partners {
        db::upsert_partner(conn, p)?;
        upserted += 1;
    }

    db::insert_audit(
        conn,
        &crate::models::AuditLog {
            id: Uuid::new_v4().to_string(),
            actor: "user".into(),
            action: "import_partners_csv".into(),
            target: "partners".into(),
            detail: format!(
                "upserted={upserted} replace={replace_all} skipped={}",
                parsed.skipped
            ),
            ts: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        },
    )?;

    let partners = db::list_partners(conn)?;
    let mut message = if replace_all {
        format!("已按 Excel 模板全量替换机构名单，共 {upserted} 家")
    } else {
        format!("已导入 {upserted} 家机构")
    };
    if parsed.skipped > 0 {
        message.push_str(&format!("；跳过 {} 行", parsed.skipped));
    }
    if !parsed.errors.is_empty() {
        let preview: Vec<String> = parsed.errors.iter().take(5).cloned().collect();
        message.push_str(&format!("（{}）", preview.join("；")));
    }

    Ok(PartnersDocumentImportResult {
        ok: true,
        message,
        upserted,
        partners,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexicon_dedup() {
        let lx = build_lexicon("甲公司", "甲", "甲集团", "9111", &["甲".into(), "乙子公司".into()]);
        assert_eq!(lx, vec!["甲公司", "甲", "甲集团", "9111", "乙子公司"]);
    }

    #[test]
    fn parse_fixed_template_headers() {
        let csv = "机构全称,简称,集团,统一社会信用代码,机构类型,关联方,合作状态\n\
甲助贷科技有限公司,甲助贷,甲集团,9111,助贷,甲担保|乙咨询,合作中\n";
        let r = parse_partners_csv(csv);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert_eq!(r.imported, 1);
        assert_eq!(r.partners[0].name, "甲助贷科技有限公司");
        assert_eq!(r.partners[0].partner_type, "loan");
        assert_eq!(r.partners[0].status, "active");
        assert!(r.partners[0].related_parties.contains(&"甲担保".to_string()));
    }

    #[test]
    fn parse_status_paused() {
        let csv = "机构全称,机构类型,合作状态\n乙支付,支付,暂停\n";
        let r = parse_partners_csv(csv);
        assert_eq!(r.partners[0].status, "paused");
    }
}

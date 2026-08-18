use crate::models::{AuditLog, Partner, RiskClue};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

pub fn default_db_path() -> PathBuf {
    let base = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("axiom-risk-agent");
    let _ = std::fs::create_dir_all(&base);
    base.join("axiom.db")
}

pub fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    let _ = conn.busy_timeout(std::time::Duration::from_secs(8));
    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS partners (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            alias TEXT NOT NULL DEFAULT '',
            group_name TEXT NOT NULL DEFAULT '',
            uscc TEXT NOT NULL DEFAULT '',
            partner_type TEXT NOT NULL,
            related_parties_json TEXT NOT NULL DEFAULT '[]',
            status TEXT NOT NULL DEFAULT 'active',
            lexicon_json TEXT NOT NULL DEFAULT '[]',
            updated_at TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_partners_uscc
            ON partners(uscc) WHERE uscc != '';
        CREATE TABLE IF NOT EXISTS risk_clues (
            id TEXT PRIMARY KEY,
            partner_id TEXT NOT NULL,
            partner TEXT NOT NULL,
            partner_type TEXT NOT NULL,
            level INTEGER NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL DEFAULT '',
            event_date TEXT NOT NULL,
            owner TEXT NOT NULL DEFAULT '合作风控',
            progress INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT '待复核',
            source_system TEXT NOT NULL,
            source_url TEXT NOT NULL DEFAULT '',
            credibility TEXT NOT NULL DEFAULT 'B',
            rule_id TEXT NOT NULL DEFAULT '',
            rule_set_version TEXT NOT NULL DEFAULT '',
            legal_basis TEXT NOT NULL DEFAULT '',
            denoise_status TEXT NOT NULL DEFAULT 'new',
            related_party_flag INTEGER NOT NULL DEFAULT 0,
            evidence_hash TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(evidence_hash)
        );
        CREATE TABLE IF NOT EXISTS audit_logs (
            id TEXT PRIMARY KEY,
            actor TEXT NOT NULL,
            action TEXT NOT NULL,
            target TEXT NOT NULL,
            detail TEXT NOT NULL,
            ts TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS rule_overrides (
            rule_id TEXT PRIMARY KEY,
            enabled INTEGER,
            level INTEGER,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS custom_rules (
            id TEXT PRIMARY KEY,
            partner_type TEXT NOT NULL,
            risk_point TEXT NOT NULL,
            level INTEGER NOT NULL,
            match_keywords_json TEXT NOT NULL DEFAULT '[]',
            legal_basis TEXT NOT NULL DEFAULT '',
            trigger_text TEXT NOT NULL DEFAULT '',
            data_source TEXT NOT NULL DEFAULT '',
            enabled INTEGER NOT NULL DEFAULT 1,
            source TEXT NOT NULL DEFAULT 'chat',
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS seen_hashes (
            evidence_hash TEXT PRIMARY KEY,
            first_seen_at TEXT NOT NULL,
            last_seen_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        ",
    )
    .map_err(|e| e.to_string())?;
    let _ = crate::finance::ensure_schema(&conn);
    let _ = crate::admission::ensure_schema(&conn);
    let _ = crate::whistle_reports::ensure_schema(&conn);
    // 清掉历史黑猫 fixture / 演示线索，避免当成真实监测结果
    let _ = conn.execute(
        "DELETE FROM risk_clues
         WHERE source_system = 'heimao'
            OR source_url LIKE '%/demo/heimao%'
            OR source_url LIKE '%tousu.sina.com.cn/demo%'",
        [],
    );
    // migrate older DBs that lack new columns
    let _ = conn.execute(
        "ALTER TABLE custom_rules ADD COLUMN trigger_text TEXT NOT NULL DEFAULT ''",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE custom_rules ADD COLUMN data_source TEXT NOT NULL DEFAULT ''",
        [],
    );
    Ok(conn)
}

pub fn list_partners(conn: &Connection) -> Result<Vec<Partner>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, alias, group_name, uscc, partner_type,
                    related_parties_json, status, lexicon_json
             FROM partners ORDER BY name",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], map_partner)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn map_partner(row: &rusqlite::Row<'_>) -> rusqlite::Result<Partner> {
    let partner_type: String = row.get(5)?;
    let related: String = row.get(6)?;
    let lexicon: String = row.get(8)?;
    let related_parties: Vec<String> =
        serde_json::from_str(&related).unwrap_or_default();
    let lexicon: Vec<String> = serde_json::from_str(&lexicon).unwrap_or_default();
    let label = crate::models::PartnerType::parse(&partner_type)
        .map(|t| t.label().to_string())
        .unwrap_or_else(|| partner_type.clone());
    Ok(Partner {
        id: row.get(0)?,
        name: row.get(1)?,
        alias: row.get(2)?,
        group_name: row.get(3)?,
        uscc: row.get(4)?,
        partner_type,
        partner_type_label: label,
        related_parties,
        status: row.get(7)?,
        lexicon,
    })
}

pub fn upsert_partner(conn: &Connection, p: &Partner) -> Result<bool, String> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let related = serde_json::to_string(&p.related_parties).unwrap_or_else(|_| "[]".into());
    let lexicon = serde_json::to_string(&p.lexicon).unwrap_or_else(|_| "[]".into());

    let by_id: Option<String> = if !p.id.is_empty() {
        conn.query_row(
            "SELECT id FROM partners WHERE id = ?1",
            params![p.id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
    } else {
        None
    };

    let existing: Option<String> = if by_id.is_some() {
        by_id
    } else if !p.uscc.is_empty() {
        conn.query_row(
            "SELECT id FROM partners WHERE uscc = ?1",
            params![p.uscc],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
    } else {
        conn.query_row(
            "SELECT id FROM partners WHERE name = ?1",
            params![p.name],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
    };

    if let Some(id) = existing {
        conn.execute(
            "UPDATE partners SET name=?1, alias=?2, group_name=?3, uscc=?4,
             partner_type=?5, related_parties_json=?6, status=?7, lexicon_json=?8, updated_at=?9
             WHERE id=?10",
            params![
                p.name,
                p.alias,
                p.group_name,
                p.uscc,
                p.partner_type,
                related,
                p.status,
                lexicon,
                now,
                id
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        conn.execute(
            "INSERT INTO partners
             (id, name, alias, group_name, uscc, partner_type, related_parties_json, status, lexicon_json, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                p.id,
                p.name,
                p.alias,
                p.group_name,
                p.uscc,
                p.partner_type,
                related,
                p.status,
                lexicon,
                now
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(false)
    }
}

pub fn delete_partner(conn: &Connection, partner_id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM partners WHERE id = ?1", params![partner_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn clear_partners(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM partners", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_clues(conn: &Connection) -> Result<Vec<RiskClue>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, partner_id, partner, partner_type, level, title, summary,
                    event_date, owner, progress, status, source_system, source_url,
                    credibility, rule_id, rule_set_version, legal_basis, denoise_status,
                    related_party_flag, evidence_hash, created_at
             FROM risk_clues
             WHERE source_system != 'heimao'
               AND IFNULL(source_url, '') NOT LIKE '%/demo/heimao%'
             ORDER BY level ASC, event_date DESC, id DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(RiskClue {
                id: row.get(0)?,
                partner_id: row.get(1)?,
                partner: row.get(2)?,
                partner_type: row.get(3)?,
                level: row.get(4)?,
                title: row.get(5)?,
                summary: row.get(6)?,
                event_date: row.get(7)?,
                owner: row.get(8)?,
                progress: row.get(9)?,
                status: row.get(10)?,
                source_system: row.get(11)?,
                source_url: row.get(12)?,
                credibility: row.get(13)?,
                rule_id: row.get(14)?,
                rule_set_version: row.get(15)?,
                legal_basis: row.get(16)?,
                denoise_status: row.get(17)?,
                related_party_flag: row.get::<_, i64>(18)? != 0,
                evidence_hash: row.get(19)?,
                created_at: row.get(20)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// Insert clue if evidence_hash unseen; if seen, mark denoise as repeat and refresh last_seen.
pub fn upsert_clue(conn: &Connection, clue: &RiskClue) -> Result<&'static str, String> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let seen: Option<String> = conn
        .query_row(
            "SELECT first_seen_at FROM seen_hashes WHERE evidence_hash = ?1",
            params![clue.evidence_hash],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if seen.is_some() {
        conn.execute(
            "UPDATE seen_hashes SET last_seen_at = ?1 WHERE evidence_hash = ?2",
            params![now, clue.evidence_hash],
        )
        .map_err(|e| e.to_string())?;
        let _ = conn.execute(
            "UPDATE risk_clues SET denoise_status = 'repeat' WHERE evidence_hash = ?1",
            params![clue.evidence_hash],
        );
        return Ok("repeat");
    }

    conn.execute(
        "INSERT INTO seen_hashes (evidence_hash, first_seen_at, last_seen_at) VALUES (?1,?2,?3)",
        params![clue.evidence_hash, now, now],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO risk_clues
         (id, partner_id, partner, partner_type, level, title, summary, event_date, owner,
          progress, status, source_system, source_url, credibility, rule_id, rule_set_version,
          legal_basis, denoise_status, related_party_flag, evidence_hash, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
        params![
            clue.id,
            clue.partner_id,
            clue.partner,
            clue.partner_type,
            clue.level,
            clue.title,
            clue.summary,
            clue.event_date,
            clue.owner,
            clue.progress,
            clue.status,
            clue.source_system,
            clue.source_url,
            clue.credibility,
            clue.rule_id,
            clue.rule_set_version,
            clue.legal_basis,
            clue.denoise_status,
            if clue.related_party_flag { 1 } else { 0 },
            clue.evidence_hash,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok("new")
}

pub fn update_clue(
    conn: &Connection,
    clue_id: &str,
    level: Option<u8>,
    status: Option<String>,
) -> Result<RiskClue, String> {
    if let Some(lv) = level {
        conn.execute(
            "UPDATE risk_clues SET level = ?1 WHERE id = ?2",
            params![lv, clue_id],
        )
        .map_err(|e| e.to_string())?;
    }
    if let Some(st) = status {
        conn.execute(
            "UPDATE risk_clues SET status = ?1 WHERE id = ?2",
            params![st, clue_id],
        )
        .map_err(|e| e.to_string())?;
    }
    list_clues(conn)?
        .into_iter()
        .find(|c| c.id == clue_id)
        .ok_or_else(|| format!("线索不存在: {clue_id}"))
}

pub fn insert_audit(conn: &Connection, log: &AuditLog) -> Result<(), String> {
    conn.execute(
        "INSERT INTO audit_logs (id, actor, action, target, detail, ts) VALUES (?1,?2,?3,?4,?5,?6)",
        params![log.id, log.actor, log.action, log.target, log.detail, log.ts],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_audit(conn: &Connection, limit: u32) -> Result<Vec<AuditLog>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, actor, action, target, detail, ts FROM audit_logs
             ORDER BY ts DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(AuditLog {
                id: row.get(0)?,
                actor: row.get(1)?,
                action: row.get(2)?,
                target: row.get(3)?,
                detail: row.get(4)?,
                ts: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn get_overrides(conn: &Connection) -> Result<Vec<(String, Option<bool>, Option<u8>)>, String> {
    let mut stmt = conn
        .prepare("SELECT rule_id, enabled, level FROM rule_overrides")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let enabled: Option<i64> = row.get(1)?;
            let level: Option<i64> = row.get(2)?;
            Ok((
                row.get::<_, String>(0)?,
                enabled.map(|v| v != 0),
                level.map(|v| v as u8),
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn set_override(
    conn: &Connection,
    rule_id: &str,
    enabled: Option<bool>,
    level: Option<u8>,
) -> Result<(), String> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "INSERT INTO rule_overrides (rule_id, enabled, level, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(rule_id) DO UPDATE SET
           enabled=excluded.enabled,
           level=excluded.level,
           updated_at=excluded.updated_at",
        params![
            rule_id,
            enabled.map(|v| if v { 1 } else { 0 }),
            level.map(|v| v as i64),
            now
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_custom_rules(
    conn: &Connection,
) -> Result<Vec<(String, crate::rules::RuleDef)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, partner_type, risk_point, level, match_keywords_json, legal_basis,
                    enabled, trigger_text, data_source
             FROM custom_rules ORDER BY id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let keywords_json: String = row.get(4)?;
            let keywords: Vec<String> =
                serde_json::from_str(&keywords_json).unwrap_or_default();
            let enabled: i64 = row.get(6)?;
            Ok((
                row.get::<_, String>(1)?,
                crate::rules::RuleDef {
                    id: row.get(0)?,
                    risk_point: row.get(2)?,
                    level: row.get(3)?,
                    match_any_keywords: keywords,
                    metric: None,
                    threshold: None,
                    legal_basis: row.get(5)?,
                    enabled: enabled != 0,
                    trigger: row.get(7).unwrap_or_default(),
                    data_source: row.get(8).unwrap_or_default(),
                },
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn upsert_custom_rule(
    conn: &Connection,
    partner_type: &str,
    def: &crate::rules::RuleDef,
) -> Result<(), String> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let kw = serde_json::to_string(&def.match_any_keywords).unwrap_or_else(|_| "[]".into());
    conn.execute(
        "INSERT INTO custom_rules
         (id, partner_type, risk_point, level, match_keywords_json, legal_basis, enabled, source,
          trigger_text, data_source, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,'edit',?8,?9,?10)
         ON CONFLICT(id) DO UPDATE SET
           partner_type=excluded.partner_type,
           risk_point=excluded.risk_point,
           level=excluded.level,
           match_keywords_json=excluded.match_keywords_json,
           legal_basis=excluded.legal_basis,
           enabled=excluded.enabled,
           trigger_text=excluded.trigger_text,
           data_source=excluded.data_source,
           updated_at=excluded.updated_at",
        params![
            def.id,
            partner_type,
            def.risk_point,
            def.level,
            kw,
            def.legal_basis,
            if def.enabled { 1 } else { 0 },
            def.trigger,
            def.data_source,
            now
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_custom_rule(conn: &Connection, rule_id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM custom_rules WHERE id = ?1", params![rule_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn clear_custom_rules(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM custom_rules", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn clear_rule_overrides(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM rule_overrides", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |r| r.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
        params![key, value, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

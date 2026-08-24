use crate::models::{CustomRuleInput, RuleOverride, RuleView, RULE_SET_VERSION};
use crate::sources::{HeimaoMetrics, RawHit};
use serde::Deserialize;

const KEYWORDS_YAML: &str =
    include_str!("../../resources/rules/baseline/v1.1.0/keywords.yaml");
const COMMON_YAML: &str = include_str!("../../resources/rules/baseline/v1.1.0/common.yaml");
const LOAN_YAML: &str = include_str!("../../resources/rules/baseline/v1.1.0/loan_assist.yaml");
const COLL_YAML: &str = include_str!("../../resources/rules/baseline/v1.1.0/collection.yaml");
const GUAR_YAML: &str = include_str!("../../resources/rules/baseline/v1.1.0/guarantee.yaml");
const TRAF_YAML: &str = include_str!("../../resources/rules/baseline/v1.1.0/traffic.yaml");
const PAY_YAML: &str = include_str!("../../resources/rules/baseline/v1.1.0/payment.yaml");
const DATA_YAML: &str = include_str!("../../resources/rules/baseline/v1.1.0/data.yaml");

#[derive(Debug, Clone, Deserialize)]
pub struct RuleThreshold {
    pub growth_ratio: Option<f64>,
    pub growth_ratio_max: Option<f64>,
    pub resolve_rate_max: Option<f64>,
    pub min_count: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuleDef {
    pub id: String,
    pub risk_point: String,
    pub level: u8,
    #[serde(default)]
    pub trigger: String,
    #[serde(default)]
    pub data_source: String,
    #[serde(default)]
    pub match_any_keywords: Vec<String>,
    pub metric: Option<String>,
    pub threshold: Option<RuleThreshold>,
    pub legal_basis: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
struct RuleFile {
    partner_type: String,
    rules: Vec<RuleDef>,
}

#[derive(Debug, Clone)]
pub struct EffectiveRule {
    pub def: RuleDef,
    pub partner_type: String,
    pub overridden: bool,
    pub source: String, // baseline | custom
}

#[derive(Debug, Clone)]
pub struct GradeResult {
    pub level: u8,
    pub rule_id: String,
    #[allow(dead_code)]
    pub risk_point: String,
    pub legal_basis: String,
    pub rule_set_version: String,
}

pub fn keywords_yaml() -> &'static str {
    KEYWORDS_YAML
}

pub fn load_baseline_rules() -> Vec<(String, RuleDef)> {
    let files = [
        COMMON_YAML, LOAN_YAML, COLL_YAML, GUAR_YAML, TRAF_YAML, PAY_YAML, DATA_YAML,
    ];
    let mut out = Vec::new();
    for raw in files {
        if let Ok(file) = serde_yaml::from_str::<RuleFile>(raw) {
            for r in file.rules {
                out.push((file.partner_type.clone(), r));
            }
        }
    }
    out
}

pub fn apply_overrides_and_customs(
    baseline: Vec<(String, RuleDef)>,
    overrides: &[RuleOverride],
    customs: &[RuleDef],
    custom_types: &[String],
) -> Vec<EffectiveRule> {
    use std::collections::HashMap;
    let ov: HashMap<&str, &RuleOverride> =
        overrides.iter().map(|o| (o.rule_id.as_str(), o)).collect();

    let mut by_id: HashMap<String, EffectiveRule> = HashMap::new();
    for (partner_type, mut def) in baseline {
        let mut overridden = false;
        if let Some(o) = ov.get(def.id.as_str()) {
            overridden = true;
            if let Some(en) = o.enabled {
                def.enabled = en;
            }
            if let Some(lv) = o.level {
                def.level = lv;
            }
        }
        by_id.insert(
            def.id.clone(),
            EffectiveRule {
                def,
                partner_type,
                overridden,
                source: "baseline".into(),
            },
        );
    }

    for (def, ptype) in customs.iter().zip(custom_types.iter()) {
        let mut def = def.clone();
        let mut overridden = true;
        let source = if by_id.contains_key(&def.id) {
            "edited".into()
        } else {
            "custom".into()
        };
        if let Some(o) = ov.get(def.id.as_str()) {
            overridden = true;
            if let Some(en) = o.enabled {
                def.enabled = en;
            }
            if let Some(lv) = o.level {
                def.level = lv;
            }
        }
        by_id.insert(
            def.id.clone(),
            EffectiveRule {
                def,
                partner_type: ptype.clone(),
                overridden,
                source,
            },
        );
    }

    let mut out: Vec<_> = by_id.into_values().collect();
    out.sort_by(|a, b| {
        a.partner_type
            .cmp(&b.partner_type)
            .then(a.def.level.cmp(&b.def.level))
            .then(a.def.id.cmp(&b.def.id))
    });
    out
}

pub fn partner_type_label(raw: &str) -> String {
    match raw {
        "common" => "通用".into(),
        "loan" => "助贷".into(),
        "guarantee" => "融资担保".into(),
        "traffic" => "引流".into(),
        "payment" => "支付".into(),
        "data" => "数据".into(),
        "collection" => "催收".into(),
        _ => raw.to_string(),
    }
}

pub fn list_rule_views_with_baseline(
    use_baseline: bool,
    overrides: &[RuleOverride],
    customs: &[(String, RuleDef)],
) -> Vec<RuleView> {
    let custom_defs: Vec<RuleDef> = customs.iter().map(|(_, d)| d.clone()).collect();
    let custom_types: Vec<String> = customs.iter().map(|(t, _)| t.clone()).collect();
    let baseline = if use_baseline {
        load_baseline_rules()
    } else {
        Vec::new()
    };
    let eff = apply_overrides_and_customs(baseline, overrides, &custom_defs, &custom_types);
    eff.into_iter()
        .map(|e| RuleView {
            id: e.def.id,
            partner_type: e.partner_type.clone(),
            partner_type_label: partner_type_label(&e.partner_type),
            risk_point: e.def.risk_point,
            level: e.def.level,
            trigger: e.def.trigger,
            data_source: e.def.data_source,
            legal_basis: e.def.legal_basis,
            enabled: e.def.enabled,
            overridden: e.overridden,
            match_any_keywords: e.def.match_any_keywords,
            metric: e.def.metric,
            source: e.source,
        })
        .collect()
}

/// 是否仍合并内置基线。Excel 全量导入后切到 imported，只展示导入规则。
pub fn use_baseline_library(conn: &rusqlite::Connection) -> Result<bool, String> {
    Ok(crate::db::get_setting(conn, "rules_library_mode")?.as_deref() != Some("imported"))
}

pub fn load_effective_rules(
    conn: &rusqlite::Connection,
) -> Result<Vec<EffectiveRule>, String> {
    let use_baseline = use_baseline_library(conn)?;
    let overrides = crate::db::get_overrides(conn)?
        .into_iter()
        .map(|(rule_id, enabled, level)| RuleOverride {
            rule_id,
            enabled,
            level,
        })
        .collect::<Vec<_>>();
    let customs = crate::db::list_custom_rules(conn)?;
    let custom_defs: Vec<RuleDef> = customs.iter().map(|(_, d)| d.clone()).collect();
    let custom_types: Vec<String> = customs.iter().map(|(t, _)| t.clone()).collect();
    let baseline = if use_baseline {
        load_baseline_rules()
    } else {
        Vec::new()
    };
    Ok(apply_overrides_and_customs(
        baseline,
        &overrides,
        &custom_defs,
        &custom_types,
    ))
}

pub fn list_rule_views_from_db(conn: &rusqlite::Connection) -> Result<Vec<RuleView>, String> {
    let use_baseline = use_baseline_library(conn)?;
    let overrides = crate::db::get_overrides(conn)?
        .into_iter()
        .map(|(rule_id, enabled, level)| RuleOverride {
            rule_id,
            enabled,
            level,
        })
        .collect::<Vec<_>>();
    let customs = crate::db::list_custom_rules(conn)?;
    Ok(list_rule_views_with_baseline(
        use_baseline,
        &overrides,
        &customs,
    ))
}

pub fn grade_hit(
    partner_type: &str,
    hit: &RawHit,
    text: &str,
    rules: &[EffectiveRule],
) -> Option<GradeResult> {
    let applicable: Vec<&EffectiveRule> = rules
        .iter()
        .filter(|r| r.def.enabled && (r.partner_type == "common" || r.partner_type == partner_type))
        .collect();

    let mut best: Option<GradeResult> = None;

    for r in applicable {
        let matched = if let Some(metric) = &r.def.metric {
            match_metric(metric, hit.heimao.as_ref(), r.def.threshold.as_ref())
        } else {
            r.def
                .match_any_keywords
                .iter()
                .any(|k| !k.is_empty() && text.contains(k.as_str()))
        };

        if !matched {
            continue;
        }

        let level = r.def.level;

        let candidate = GradeResult {
            level,
            rule_id: r.def.id.clone(),
            risk_point: r.def.risk_point.clone(),
            legal_basis: r.def.legal_basis.clone(),
            rule_set_version: RULE_SET_VERSION.to_string(),
        };

        best = Some(match best {
            None => candidate,
            Some(prev) if candidate.level < prev.level => candidate,
            Some(prev) => prev,
        });
    }

    best
}

fn match_metric(
    metric: &str,
    heimao: Option<&HeimaoMetrics>,
    threshold: Option<&RuleThreshold>,
) -> bool {
    let Some(m) = heimao else {
        return false;
    };
    let thr = threshold;
    match metric {
        "heimao_complaint_spike" => {
            let growth = thr.and_then(|t| t.growth_ratio).unwrap_or(0.5);
            let resolve_max = thr.and_then(|t| t.resolve_rate_max).unwrap_or(0.3);
            m.growth_ratio() > growth && m.resolve_rate < resolve_max
        }
        "heimao_complaint_high_stable" => {
            let min_count = thr.and_then(|t| t.min_count).unwrap_or(20);
            let growth_max = thr.and_then(|t| t.growth_ratio_max).unwrap_or(0.5);
            m.complaint_count_30d >= min_count && m.growth_ratio() <= growth_max
        }
        _ => false,
    }
}

pub fn custom_input_to_def(input: &CustomRuleInput) -> RuleDef {
    RuleDef {
        id: input.id.clone(),
        risk_point: input.risk_point.clone(),
        level: input.level,
        trigger: input.trigger.clone().unwrap_or_default(),
        data_source: input.data_source.clone().unwrap_or_default(),
        match_any_keywords: input.match_any_keywords.clone(),
        metric: None,
        threshold: None,
        legal_basis: input.legal_basis.clone(),
        enabled: input.enabled,
    }
}

mod chat;
mod import;
pub use chat::apply_rule_chat;
pub use import::{import_from_csv, upsert_rule_edit};

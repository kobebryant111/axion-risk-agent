use crate::models::{
    CustomRuleInput, PartnerType, RuleChatAction, RuleChatResult, RuleOverride,
};
use crate::rules::RuleDef;
use uuid::Uuid;

/// 对话式规则维护：确定性意图解析（无需 LLM 也可落地）。
/// 支持：
/// - 停用/禁用 <规则ID或风险点关键词>
/// - 启用 <规则ID或风险点关键词>
/// - 把 <规则> 改为 L2 / 等级改为2
/// - 新增规则：类型=催收，等级=2，风险点=xxx，关键词=a;b，依据=xxx
/// - 删除自定义规则 <ID>
/// - 有多少条规则 / 统计
pub fn apply_rule_chat(
    message: &str,
    existing: &[(String, RuleDef)], // partner_type, def (baseline+custom merged view defs)
    customs_only: &[String],        // custom rule ids
) -> RuleChatResult {
    let msg = message.trim();
    if msg.is_empty() {
        return RuleChatResult {
            ok: false,
            reply: "请输入指令，例如：停用 LOAN-L1-COST-24；或：新增规则：类型=催收，等级=2，风险点=骚扰第三方，关键词=爆通讯录;骚扰，依据=消保法".into(),
            actions: vec![],
        };
    }

    if msg.contains("多少") || msg.contains("统计") || msg.contains("一共") {
        let total = existing.len();
        let mut by_type: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for (t, _) in existing {
            *by_type.entry(t.as_str()).or_default() += 1;
        }
        let detail = by_type
            .iter()
            .map(|(k, v)| format!("{k}:{v}"))
            .collect::<Vec<_>>()
            .join("，");
        return RuleChatResult {
            ok: true,
            reply: format!(
                "当前生效规则共 {total} 条（基线 v{} + 自定义）。分类：{detail}",
                crate::models::RULE_SET_VERSION
            ),
            actions: vec![],
        };
    }

    if let Some(rest) = strip_prefix_any(msg, &["删除自定义规则", "删除规则", "移除自定义规则", "移除规则"]) {
        let id = rest.trim().to_string();
        if id.is_empty() {
            return fail("请给出要删除的自定义规则 ID");
        }
        if !customs_only.iter().any(|c| c == &id) {
            return fail(&format!(
                "只能删除对话/导入新增的自定义规则；{id} 不在自定义列表中。内置基线请用「停用」。"
            ));
        }
        return RuleChatResult {
            ok: true,
            reply: format!("将删除自定义规则 {id}"),
            actions: vec![RuleChatAction::DeleteCustom { rule_id: id }],
        };
    }

    if let Some(body) = strip_prefix_any(msg, &["新增规则", "添加规则", "新建规则"]) {
        return parse_add_rule(body.trim_start_matches(['：', ':']).trim());
    }

    if let Some(rest) = strip_prefix_any(msg, &["停用", "禁用", "关闭规则"]) {
        return toggle_or_level(rest.trim(), false, None, existing);
    }
    if let Some(rest) = strip_prefix_any(msg, &["启用", "打开规则", "开启"]) {
        return toggle_or_level(rest.trim(), true, None, existing);
    }

    // 把 XXX 改为 L2 / 将 XXX 等级改为 2
    if (msg.contains("改为") || msg.contains("改成") || msg.contains("调整为"))
        && (msg.contains('L') || msg.contains('l') || msg.contains("等级") || msg.contains("级"))
    {
        if let Some((target, level)) = parse_level_change(msg) {
            return toggle_or_level(&target, true, Some(level), existing);
        }
    }

    RuleChatResult {
        ok: false,
        reply: "未识别指令。可试：\n1) 停用 COMMON-L1-DISHONEST\n2) 启用 失信被执行人\n3) 把 暴力催收 改为 L1\n4) 新增规则：类型=催收，等级=2，风险点=批量举报，关键词=批量举报;举报集中，依据=消保法\n5) 删除自定义规则 CUSTOM-xxxx\n6) 有多少条规则".into(),
        actions: vec![],
    }
}

fn fail(msg: &str) -> RuleChatResult {
    RuleChatResult {
        ok: false,
        reply: msg.into(),
        actions: vec![],
    }
}

fn strip_prefix_any<'a>(msg: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    for p in prefixes {
        if let Some(rest) = msg.strip_prefix(p) {
            return Some(rest);
        }
    }
    None
}

fn toggle_or_level(
    query: &str,
    enabled: bool,
    level: Option<u8>,
    existing: &[(String, RuleDef)],
) -> RuleChatResult {
    let q = query
        .trim()
        .trim_matches(['：', ':', ' ', '。', '，'])
        .to_string();
    if q.is_empty() {
        return fail("请指定规则 ID 或风险点关键词");
    }

    let matches: Vec<&RuleDef> = existing
        .iter()
        .map(|(_, d)| d)
        .filter(|d| d.id.eq_ignore_ascii_case(&q) || d.risk_point.contains(&q) || d.id.contains(&q))
        .collect();

    if matches.is_empty() {
        return fail(&format!("未找到匹配「{q}」的规则"));
    }
    if matches.len() > 1 && !matches.iter().any(|d| d.id.eq_ignore_ascii_case(&q)) {
        let ids: Vec<_> = matches.iter().take(5).map(|d| d.id.as_str()).collect();
        return fail(&format!(
            "匹配到多条规则（{}…），请用完整规则 ID 指定",
            ids.join(", ")
        ));
    }

    let rule = matches
        .iter()
        .find(|d| d.id.eq_ignore_ascii_case(&q))
        .unwrap_or(&matches[0]);

    let mut actions = vec![RuleChatAction::Override(RuleOverride {
        rule_id: rule.id.clone(),
        enabled: Some(enabled),
        level,
    })];

    let reply = if let Some(lv) = level {
        format!("将把 {}（{}）等级覆盖为 L{lv}，并保持启用", rule.id, rule.risk_point)
    } else if enabled {
        format!("将启用 {}（{}）", rule.id, rule.risk_point)
    } else {
        format!("将停用 {}（{}）", rule.id, rule.risk_point)
    };

    // if only level change, still set enabled true via override above
    let _ = &mut actions;

    RuleChatResult {
        ok: true,
        reply,
        actions,
    }
}

fn parse_level_change(msg: &str) -> Option<(String, u8)> {
    // patterns: 把 X 改为 L2 / 将X等级改为2 / X 改成3级
    let level = extract_level(msg)?;
    let mut target = msg.to_string();
    for sep in ["改为", "改成", "调整为", "设置为"] {
        if let Some(idx) = target.find(sep) {
            target = target[..idx].to_string();
            break;
        }
    }
    for p in ["把", "将", "请"] {
        if let Some(rest) = target.strip_prefix(p) {
            target = rest.to_string();
        }
    }
    for p in ["的等级", "等级", "规则"] {
        target = target.replace(p, "");
    }
    let target = target.trim().to_string();
    if target.is_empty() {
        None
    } else {
        Some((target, level))
    }
}

fn extract_level(msg: &str) -> Option<u8> {
    let chars: Vec<char> = msg.chars().collect();
    for i in 0..chars.len() {
        if (chars[i] == 'L' || chars[i] == 'l') && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()
        {
            let n = chars[i + 1].to_digit(10)? as u8;
            if (1..=4).contains(&n) {
                return Some(n);
            }
        }
        if chars[i].is_ascii_digit() {
            let n = chars[i].to_digit(10)? as u8;
            if (1..=4).contains(&n)
                && (i + 1 >= chars.len()
                    || chars[i + 1] == '级'
                    || msg.contains("等级"))
            {
                // avoid matching random digits in IDs when possible
                if msg.contains("改为") || msg.contains("改成") || msg.contains("调整为") {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn parse_add_rule(body: &str) -> RuleChatResult {
    // 类型=催收，等级=2，风险点=xxx，关键词=a;b，依据=xxx
    let mut partner_type = String::new();
    let mut level: Option<u8> = None;
    let mut risk_point = String::new();
    let mut keywords = Vec::new();
    let mut legal_basis = String::new();

    for part in body.split(['，', ',', '；', ';', '\n']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (k, v) = if let Some((a, b)) = part.split_once('=') {
            (a.trim(), b.trim())
        } else if let Some((a, b)) = part.split_once('：') {
            (a.trim(), b.trim())
        } else if let Some((a, b)) = part.split_once(':') {
            (a.trim(), b.trim())
        } else {
            continue;
        };
        match k {
            "类型" | "机构类型" | "partner_type" | "type" => partner_type = v.to_string(),
            "等级" | "level" => {
                level = v
                    .trim_start_matches(['L', 'l'])
                    .trim_end_matches('级')
                    .parse()
                    .ok()
            }
            "风险点" | "risk_point" | "名称" => risk_point = v.to_string(),
            "关键词" | "keywords" | "匹配" => {
                keywords = v
                    .split(['/', '|', '、', ';', '；', ' '])
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "依据" | "法规" | "legal_basis" | "法律依据" => legal_basis = v.to_string(),
            _ => {}
        }
    }

    let partner_key = if partner_type == "common"
        || partner_type == "通用"
        || partner_type == "跨机构"
    {
        "common".to_string()
    } else {
        let Some(ptype) = PartnerType::parse(&partner_type) else {
            return fail("新增规则须指定类型（助贷/融担/引流/支付/数据/催收/common）");
        };
        ptype.as_str().to_string()
    };
    let Some(level) = level.filter(|l| (1..=4).contains(l)) else {
        return fail("新增规则须指定等级 L1～L4");
    };
    if risk_point.is_empty() {
        return fail("新增规则须指定风险点");
    }
    if keywords.is_empty() {
        return fail("新增规则须指定至少一个关键词");
    }
    if legal_basis.is_empty() {
        legal_basis = "对话新增".into();
    }

    let id = format!(
        "CUSTOM-{}-{}",
        partner_key.to_uppercase(),
        &Uuid::new_v4().to_string()[..8]
    );

    RuleChatResult {
        ok: true,
        reply: format!(
            "将新增自定义规则 {id}：[{partner_key}] L{level} {risk_point}，关键词 {}",
            keywords.join("/")
        ),
        actions: vec![RuleChatAction::UpsertCustom(CustomRuleInput {
            id,
            partner_type: partner_key,
            risk_point,
            level,
            match_any_keywords: keywords,
            legal_basis,
            enabled: true,
            trigger: None,
            data_source: None,
        })],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_disable() {
        let existing = vec![(
            "loan".into(),
            RuleDef {
                id: "LOAN-L1-COST-24".into(),
                risk_point: "综合融资成本超24%".into(),
                level: 1,
                trigger: String::new(),
                data_source: String::new(),
                match_any_keywords: vec![],
                metric: None,
                threshold: None,
                legal_basis: "x".into(),
                enabled: true,
            },
        )];
        let r = apply_rule_chat("停用 LOAN-L1-COST-24", &existing, &[]);
        assert!(r.ok);
        assert_eq!(r.actions.len(), 1);
    }

    #[test]
    fn parse_add() {
        let r = apply_rule_chat(
            "新增规则：类型=催收，等级=2，风险点=测试风险，关键词=测试词甲;测试词乙，依据=测试法",
            &[],
            &[],
        );
        assert!(r.ok, "{}", r.reply);
        assert!(matches!(r.actions[0], RuleChatAction::UpsertCustom(_)));
    }
}

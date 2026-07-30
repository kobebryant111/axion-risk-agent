use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub product: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TypeCard {
    pub key: String,
    pub title: String,
    pub subtitle: String,
    pub yesterday_new: u32,
    pub high_count: u32,
    pub progress: u32,
    pub tone: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrendPoint {
    pub day: String,
    pub value: u32,
    pub highlight: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RiskClue {
    pub id: String,
    pub partner: String,
    pub partner_type: String,
    pub level: u8,
    pub title: String,
    pub event_date: String,
    pub owner: String,
    pub progress: u32,
    pub status: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EventItem {
    pub title: String,
    pub time: String,
    pub level: u8,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub user_name: String,
    pub monitor_date: String,
    pub kpi_effective: u32,
    pub kpi_level1: u32,
    pub kpi_level2: u32,
    pub kpi_pending: u32,
    pub health_score: f64,
    pub health_rank_label: String,
    pub type_cards: Vec<TypeCard>,
    pub trend: Vec<TrendPoint>,
    pub events: Vec<EventItem>,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "axiom-risk-agent".into(),
        version: "0.1.0".into(),
        product: "智联鉴控".into(),
    }
}

#[tauri::command]
pub fn get_dashboard_snapshot() -> DashboardSnapshot {
    DashboardSnapshot {
        user_name: "风控同学".into(),
        monitor_date: "2026-07-27".into(),
        kpi_effective: 8,
        kpi_level1: 0,
        kpi_level2: 3,
        kpi_pending: 5,
        health_score: 78.6,
        health_rank_label: "组合风险健康分".into(),
        type_cards: vec![
            TypeCard {
                key: "loan".into(),
                title: "助贷机构".into(),
                subtitle: "投诉 · 约谈 · 现金流".into(),
                yesterday_new: 2,
                high_count: 1,
                progress: 62,
                tone: "violet".into(),
            },
            TypeCard {
                key: "guarantee".into(),
                title: "融资担保".into(),
                subtitle: "罚单 · 解约 · 股权".into(),
                yesterday_new: 1,
                high_count: 0,
                progress: 40,
                tone: "blue".into(),
            },
            TypeCard {
                key: "traffic".into(),
                title: "流量引流".into(),
                subtitle: "合作稳定 · 盈利".into(),
                yesterday_new: 2,
                high_count: 1,
                progress: 55,
                tone: "amber".into(),
            },
            TypeCard {
                key: "payment".into(),
                title: "支付机构".into(),
                subtitle: "政策 · 处罚".into(),
                yesterday_new: 1,
                high_count: 1,
                progress: 70,
                tone: "rose".into(),
            },
            TypeCard {
                key: "data".into(),
                title: "数据服务商".into(),
                subtitle: "资质 · 政策前瞻".into(),
                yesterday_new: 1,
                high_count: 0,
                progress: 35,
                tone: "sky".into(),
            },
            TypeCard {
                key: "collection".into(),
                title: "催收机构".into(),
                subtitle: "暴力投诉 · 涉诉".into(),
                yesterday_new: 1,
                high_count: 1,
                progress: 28,
                tone: "orange".into(),
            },
        ],
        trend: vec![
            TrendPoint { day: "周一".into(), value: 6, highlight: false },
            TrendPoint { day: "周二".into(), value: 9, highlight: false },
            TrendPoint { day: "周三".into(), value: 5, highlight: false },
            TrendPoint { day: "周四".into(), value: 11, highlight: false },
            TrendPoint { day: "周五".into(), value: 8, highlight: false },
            TrendPoint { day: "周六".into(), value: 4, highlight: false },
            TrendPoint { day: "周日".into(), value: 8, highlight: true },
        ],
        events: vec![
            EventItem {
                title: "宁银消金催收骚扰投诉".into(),
                time: "07-27 09:20".into(),
                level: 2,
            },
            EventItem {
                title: "预付卡监管征求意见解读".into(),
                time: "07-27 14:10".into(),
                level: 2,
            },
            EventItem {
                title: "个贷明示成本 8/1 倒计时".into(),
                time: "07-27 16:40".into(),
                level: 3,
            },
            EventItem {
                title: "小型个保简化措施 9/1".into(),
                time: "07-27 14:13".into(),
                level: 3,
            },
        ],
    }
}

#[tauri::command]
pub fn list_risk_clues() -> Vec<RiskClue> {
    vec![
        RiskClue {
            id: "Y0727-CS-001".into(),
            partner: "宁银消金催收链条".into(),
            partner_type: "催收机构".into(),
            level: 2,
            title: "黑猫：联系第三人/单位并威胁还款".into(),
            event_date: "2026-07-27".into(),
            owner: "贷后合规".into(),
            progress: 35,
            status: "复核中".into(),
        },
        RiskClue {
            id: "Y0727-AL-001".into(),
            partner: "宁银消金".into(),
            partner_type: "助贷机构".into(),
            level: 2,
            title: "黑猫三日投诉含催收与征信异议".into(),
            event_date: "2026-07-27".into(),
            owner: "合作风控".into(),
            progress: 50,
            status: "处置中".into(),
        },
        RiskClue {
            id: "Y0727-ZF-001".into(),
            partner: "预付卡/储值Ⅱ类机构".into(),
            partner_type: "支付机构".into(),
            level: 2,
            title: "央行征求意见稿：禁核心业务外包".into(),
            event_date: "2026-07-27".into(),
            owner: "支付合作".into(),
            progress: 20,
            status: "待评估".into(),
        },
        RiskClue {
            id: "Y0727-LL-001".into(),
            partner: "行业-导流平台".into(),
            partner_type: "流量引流".into(),
            level: 2,
            title: "息费明示倒计时，导流变现模式承压".into(),
            event_date: "2026-07-27".into(),
            owner: "渠道管理".into(),
            progress: 15,
            status: "待启动".into(),
        },
        RiskClue {
            id: "Y0727-SJ-001".into(),
            partner: "数据服务链条".into(),
            partner_type: "数据服务商".into(),
            level: 3,
            title: "小型个保简化措施 9/1 施行".into(),
            event_date: "2026-07-27".into(),
            owner: "数据合规".into(),
            progress: 10,
            status: "跟踪中".into(),
        },
        RiskClue {
            id: "Y0727-DB-001".into(),
            partner: "行业-增信融担".into(),
            partner_type: "融资担保".into(),
            level: 3,
            title: "担保费须纳入综合融资成本明示表".into(),
            event_date: "2026-07-27".into(),
            owner: "融担管理".into(),
            progress: 25,
            status: "制度跟踪".into(),
        },
    ]
}

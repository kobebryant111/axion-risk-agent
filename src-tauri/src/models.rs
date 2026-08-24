use serde::{Deserialize, Serialize};

pub const RULE_SET_VERSION: &str = "1.1.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PartnerType {
    Loan,
    Guarantee,
    Traffic,
    Payment,
    Data,
    Collection,
}

impl PartnerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Loan => "loan",
            Self::Guarantee => "guarantee",
            Self::Traffic => "traffic",
            Self::Payment => "payment",
            Self::Data => "data",
            Self::Collection => "collection",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Loan => "助贷机构",
            Self::Guarantee => "融资担保",
            Self::Traffic => "流量引流",
            Self::Payment => "支付机构",
            Self::Data => "数据服务商",
            Self::Collection => "催收机构",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim().to_lowercase();
        // 含中文时保留原串做包含判断
        let raw_trim = raw.trim();
        if s == "loan"
            || raw_trim.contains("助贷")
            || raw_trim.contains("共同出资")
            || raw_trim.contains("消费贷")
        {
            return Some(Self::Loan);
        }
        if s == "guarantee" || raw_trim.contains("融担") || raw_trim.contains("担保") {
            return Some(Self::Guarantee);
        }
        if s == "traffic" || raw_trim.contains("引流") || raw_trim.contains("导流") {
            return Some(Self::Traffic);
        }
        if s == "payment"
            || raw_trim.contains("支付")
            || raw_trim.contains("结算")
        {
            return Some(Self::Payment);
        }
        if s == "data" || raw_trim.contains("数据") || raw_trim.contains("征信") {
            return Some(Self::Data);
        }
        if s == "collection"
            || raw_trim.contains("催收")
            || raw_trim.contains("清收")
            || raw_trim.contains("逾期")
        {
            return Some(Self::Collection);
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Partner {
    pub id: String,
    pub name: String,
    pub alias: String,
    pub group_name: String,
    pub uscc: String,
    pub partner_type: String,
    pub partner_type_label: String,
    pub related_parties: Vec<String>,
    pub status: String,
    /// 监测词库：主体词 + 关联方
    pub lexicon: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskClue {
    pub id: String,
    pub partner_id: String,
    pub partner: String,
    pub partner_type: String,
    pub level: u8,
    pub title: String,
    pub summary: String,
    pub event_date: String,
    pub owner: String,
    pub progress: u32,
    pub status: String,
    pub source_system: String,
    pub source_url: String,
    pub credibility: String,
    pub rule_id: String,
    pub rule_set_version: String,
    pub legal_basis: String,
    pub denoise_status: String,
    pub related_party_flag: bool,
    pub evidence_hash: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLog {
    pub id: String,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub detail: String,
    pub ts: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendPoint {
    pub day: String,
    pub value: u32,
    pub highlight: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventItem {
    pub title: String,
    pub time: String,
    pub level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub product: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: u32,
    pub updated: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
    pub partners: Vec<Partner>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartnerEditInput {
    pub id: String,
    pub name: String,
    pub alias: String,
    pub group_name: String,
    pub uscc: String,
    pub partner_type: String,
    pub related_parties: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartnersDocumentImportResult {
    pub ok: bool,
    pub message: String,
    pub upserted: u32,
    pub partners: Vec<Partner>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleView {
    pub id: String,
    pub partner_type: String,
    pub partner_type_label: String,
    pub risk_point: String,
    pub level: u8,
    pub trigger: String,
    pub data_source: String,
    pub legal_basis: String,
    pub enabled: bool,
    pub overridden: bool,
    pub match_any_keywords: Vec<String>,
    pub metric: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleOverride {
    pub rule_id: String,
    pub enabled: Option<bool>,
    pub level: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomRuleInput {
    pub id: String,
    pub partner_type: String,
    pub risk_point: String,
    pub level: u8,
    pub match_any_keywords: Vec<String>,
    pub legal_basis: String,
    pub enabled: bool,
    pub trigger: Option<String>,
    pub data_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleEditInput {
    pub id: String,
    pub partner_type: String,
    pub risk_point: String,
    pub level: u8,
    pub trigger: String,
    pub data_source: String,
    pub legal_basis: String,
    pub match_any_keywords: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RulesDocumentImportResult {
    pub ok: bool,
    pub message: String,
    pub upserted: u32,
    pub rules: Vec<RuleView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleChatAction {
    Override(RuleOverride),
    UpsertCustom(CustomRuleInput),
    DeleteCustom { rule_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleChatResult {
    pub ok: bool,
    pub reply: String,
    pub actions: Vec<RuleChatAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub partners_scanned: u32,
    pub raw_hits: u32,
    pub matched: u32,
    pub clues_upserted: u32,
    pub level1: u32,
    pub level2: u32,
    pub source_errors: Vec<String>,
    pub clues: Vec<RiskClue>,
    #[serde(default)]
    pub scope_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhistleSchedule {
    pub daily_enabled: bool,
    /// HH:mm 本地时间
    pub daily_time: String,
    pub weekly_enabled: bool,
    /// 1=周一 … 7=周日
    pub weekly_dow: u8,
    /// HH:mm 本地时间
    pub weekly_time: String,
    pub last_daily_run: Option<String>,
    pub last_weekly_run: Option<String>,
    pub next_daily_hint: String,
    pub next_weekly_hint: String,
    /// 空 = 监测全部机构
    #[serde(default)]
    pub partner_ids: Vec<String>,
    #[serde(default)]
    pub scope_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhistleScheduleSave {
    pub daily_enabled: Option<bool>,
    pub daily_time: Option<String>,
    pub weekly_enabled: Option<bool>,
    pub weekly_dow: Option<u8>,
    pub weekly_time: Option<String>,
    /// 空数组表示全部机构
    pub partner_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhistleReport {
    pub id: String,
    /// daily | weekly
    pub kind: String,
    pub period: String,
    pub title: String,
    pub summary: String,
    pub markdown: String,
    pub batch_stats_json: String,
    pub clue_count: u32,
    pub level1: u32,
    pub level2: u32,
    pub level3: u32,
    pub level4: u32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhistleJobResult {
    pub kind: String,
    pub batch: BatchResult,
    pub report: WhistleReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateClueRequest {
    pub clue_id: String,
    pub level: Option<u8>,
    pub status: Option<String>,
    pub actor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfigPublic {
    pub base_url: String,
    pub model: String,
    pub enabled: bool,
    pub has_api_key: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfigSave {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmTestResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnterpriseMcpSettings {
    pub qcc_enabled: bool,
    pub qcc_has_api_key: bool,
    pub qcc_ready: bool,
    pub tyc_enabled: bool,
    pub tyc_has_api_key: bool,
    pub tyc_ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnterpriseMcpSettingsSave {
    pub qcc_enabled: Option<bool>,
    pub qcc_api_key: Option<String>,
    pub tyc_enabled: Option<bool>,
    pub tyc_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnterpriseMcpTestResult {
    pub ok: bool,
    pub message: String,
    pub tool_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TavilySettings {
    pub enabled: bool,
    pub has_api_key: bool,
    pub ready: bool,
    /// 是否在风险吹哨跑批中调用全网搜索
    pub use_in_batch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TavilySettingsSave {
    pub enabled: Option<bool>,
    pub api_key: Option<String>,
    pub use_in_batch: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TavilyTestResult {
    pub ok: bool,
    pub message: String,
    pub result_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GuaranteeSpecial {
    pub leverage: Option<f64>,
    pub compensation_rate: Option<f64>,
    pub reserve_adequate: Option<bool>,
    pub single_concentration: Option<f64>,
    pub asset_ratio: Option<f64>,
    pub fraud_signal: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FinanceMetrics {
    pub revenue: Option<f64>,
    pub revenue_yoy: Option<f64>,
    pub net_profit: Option<f64>,
    pub net_profit_yoy: Option<f64>,
    pub net_margin: Option<f64>,
    pub gross_margin: Option<f64>,
    pub op_margin: Option<f64>,
    pub roa: Option<f64>,
    pub roe: Option<f64>,
    pub asset_liability_ratio: Option<f64>,
    pub current_ratio: Option<f64>,
    pub quick_ratio: Option<f64>,
    pub equity_ratio: Option<f64>,
    pub debt_ebitda: Option<f64>,
    pub asset_turnover: Option<f64>,
    pub loan_originated: Option<f64>,
    pub loan_balance: Option<f64>,
    pub npl_90: Option<f64>,
    pub ocfo: Option<f64>,
    pub fcf: Option<f64>,
    pub capex_ratio: Option<f64>,
    pub dividend_payout: Option<f64>,
    pub guarantee: Option<GuaranteeSpecial>,
    #[serde(default)]
    pub currency_unit: String,
    #[serde(default)]
    pub notes: String,
}

/// LLM 从财报附件抽取的结果（含机构元信息 + 指标）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FinanceExtractResult {
    pub partner_name: Option<String>,
    pub period: Option<String>,
    /// loan | guarantee | traffic | payment | data | collection
    pub partner_type: Option<String>,
    #[serde(flatten)]
    pub metrics: FinanceMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DimensionScore {
    pub dimension: String,
    pub grade: String,
    pub score: i32,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceAnalyzeRequest {
    pub partner_id: Option<String>,
    pub partner_name: String,
    pub partner_type: String,
    pub period: String,
    pub metrics: Option<FinanceMetrics>,
    pub use_demo: Option<bool>,
    pub document_text: Option<String>,
    /// text | csv | demo | batch
    pub source_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceReport {
    pub id: String,
    pub partner_id: String,
    pub partner_name: String,
    pub partner_type: String,
    pub period: String,
    pub metrics: FinanceMetrics,
    pub scores: Vec<DimensionScore>,
    pub overall_rating: String,
    /// 0-100 量化经营风险分（越高越好）
    #[serde(default)]
    pub risk_score: i32,
    pub concerns: Vec<String>,
    /// 股东/关联方穿透提示
    #[serde(default)]
    pub related_flags: Vec<String>,
    pub summary: String,
    pub markdown: String,
    #[serde(default)]
    pub source_kind: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinancePeerRow {
    pub partner_id: String,
    pub partner_name: String,
    pub partner_type: String,
    pub period: String,
    pub overall_rating: String,
    pub risk_score: i32,
    pub rank: u32,
    pub report_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceSeriesPoint {
    pub period: String,
    pub risk_score: i32,
    pub overall_rating: String,
    pub asset_liability_ratio: Option<f64>,
    pub net_margin: Option<f64>,
    pub fcf: Option<f64>,
    pub report_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceBatchResult {
    pub period: String,
    pub total_partners: u32,
    pub analyzed: u32,
    pub skipped: u32,
    pub failed: u32,
    pub errors: Vec<String>,
    pub reports: Vec<FinanceReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinanceCsvImportResult {
    pub imported: u32,
    pub failed: u32,
    pub errors: Vec<String>,
    pub reports: Vec<FinanceReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionCheckItem {
    pub id: String,
    pub color: String,
    pub title: String,
    pub legal_basis: String,
    pub triggered: bool,
    pub evidence: String,
    pub source_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionReviewRequest {
    pub partner_id: Option<String>,
    pub partner_name: Option<String>,
    pub partner_type: Option<String>,
    /// 统一社会信用代码（拟合作主体可先填）
    pub uscc: Option<String>,
    /// 准入 | 续约 | 再审
    pub scenario: String,
    /// 检查项 id -> 是否人工强制触发
    pub manual_flags: Option<std::collections::HashMap<String, bool>>,
    /// 是否拉取企查查/天眼查补证
    pub use_enterprise_mcp: Option<bool>,
    /// 是否用 Tavily 全网搜补证（默认 true；按次计费，最多 2 次）
    pub use_web_search: Option<bool>,
    /// 是否用 LLM 生成意见书摘要与建议
    pub use_llm_assist: Option<bool>,
    /// 本次粘贴的管理办法/审计报告/案例等材料正文
    pub materials_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionReview {
    pub id: String,
    pub partner_id: String,
    pub partner_name: String,
    pub partner_type: String,
    #[serde(default)]
    pub uscc: String,
    pub scenario: String,
    pub conclusion: String,
    pub items: Vec<AdmissionCheckItem>,
    pub remediation: Vec<String>,
    pub summary: String,
    pub markdown: String,
    pub evidence_incomplete: bool,
    /// 本地线索 / 企查查 / 天眼查 / 上传材料 / 知识库
    #[serde(default)]
    pub evidence_sources: Vec<String>,
    #[serde(default)]
    pub ai_notes: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionKnowledge {
    pub id: String,
    /// partner_policy | lending_reg | audit_report | risk_case
    pub kind: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionKnowledgeImportRequest {
    pub kind: String,
    pub title: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionCaseIngestRequest {
    pub title: Option<String>,
    pub content: String,
    /// 可选：绑定到检查项 id，如 R08；空则作为通用关键词增强
    pub check_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionCaseIngestResult {
    pub ok: bool,
    pub message: String,
    pub keywords_added: Vec<String>,
    pub knowledge_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatRequest {
    pub message: String,
    pub scope: Option<String>,
    pub history: Option<Vec<AgentChatMessage>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatResponse {
    pub ok: bool,
    pub reply: String,
    pub used_llm: bool,
    pub mutated: bool,
    pub tool_traces: Vec<String>,
}

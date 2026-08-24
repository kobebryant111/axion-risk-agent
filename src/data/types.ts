export type TypeCard = {
  key: string;
  title: string;
  subtitle: string;
  yesterdayNew: number;
  highCount: number;
  progress: number;
  tone: string;
};

export type TrendPoint = {
  day: string;
  value: number;
  highlight: boolean;
};

export type RiskClue = {
  id: string;
  partnerId?: string;
  partner: string;
  partnerType: string;
  level: number;
  title: string;
  summary?: string;
  eventDate: string;
  owner: string;
  progress: number;
  status: string;
  sourceSystem?: string;
  sourceUrl?: string;
  credibility?: string;
  ruleId?: string;
  ruleSetVersion?: string;
  legalBasis?: string;
  denoiseStatus?: string;
  relatedPartyFlag?: boolean;
  evidenceHash?: string;
  createdAt?: string;
};

export type EventItem = {
  title: string;
  time: string;
  level: number;
};

export type DashboardSnapshot = {
  userName: string;
  monitorDate: string;
  kpiEffective: number;
  kpiLevel1: number;
  kpiLevel2: number;
  kpiPending: number;
  healthScore: number;
  healthRankLabel: string;
  typeCards: TypeCard[];
  trend: TrendPoint[];
  events: EventItem[];
};

export type Partner = {
  id: string;
  name: string;
  alias: string;
  groupName: string;
  uscc: string;
  partnerType: string;
  partnerTypeLabel: string;
  relatedParties: string[];
  status: string;
  lexicon: string[];
};

export type PartnerEditInput = {
  id: string;
  name: string;
  alias: string;
  groupName: string;
  uscc: string;
  partnerType: string;
  relatedParties: string[];
  status: string;
};

export type PartnersDocumentImportResult = {
  ok: boolean;
  message: string;
  upserted: number;
  partners: Partner[];
};

export type RuleView = {
  id: string;
  partnerType: string;
  partnerTypeLabel: string;
  riskPoint: string;
  level: number;
  trigger: string;
  dataSource: string;
  legalBasis: string;
  enabled: boolean;
  overridden: boolean;
  matchAnyKeywords: string[];
  metric?: string | null;
  source: string;
};

export type RuleEditInput = {
  id: string;
  partnerType: string;
  riskPoint: string;
  level: number;
  trigger: string;
  dataSource: string;
  legalBasis: string;
  matchAnyKeywords: string[];
  enabled: boolean;
};

export type RulesDocumentImportResult = {
  ok: boolean;
  message: string;
  upserted: number;
  rules: RuleView[];
};

export type RuleOverride = {
  ruleId: string;
  enabled?: boolean | null;
  level?: number | null;
};

export type RuleChatResult = {
  ok: boolean;
  reply: string;
  actions: unknown[];
};

export type LlmConfigPublic = {
  baseUrl: string;
  model: string;
  enabled: boolean;
  hasApiKey: boolean;
  ready: boolean;
};

export type LlmConfigSave = {
  baseUrl?: string;
  apiKey?: string;
  model?: string;
  enabled?: boolean;
};

export type LlmTestResult = {
  ok: boolean;
  message: string;
};

export type EnterpriseMcpSettings = {
  qccEnabled: boolean;
  qccHasApiKey: boolean;
  qccReady: boolean;
  tycEnabled: boolean;
  tycHasApiKey: boolean;
  tycReady: boolean;
};

export type EnterpriseMcpSettingsSave = {
  qccEnabled?: boolean;
  qccApiKey?: string;
  tycEnabled?: boolean;
  tycApiKey?: string;
};

export type EnterpriseMcpTestResult = {
  ok: boolean;
  message: string;
  toolCount: number;
};

export type TavilySettings = {
  enabled: boolean;
  hasApiKey: boolean;
  ready: boolean;
  useInBatch: boolean;
};

export type TavilySettingsSave = {
  enabled?: boolean;
  apiKey?: string;
  useInBatch?: boolean;
};

export type TavilyTestResult = {
  ok: boolean;
  message: string;
  resultCount: number;
};

export type AgentChatMessage = {
  role: string;
  content: string;
};

export type AgentChatResponse = {
  ok: boolean;
  reply: string;
  usedLlm: boolean;
  mutated: boolean;
  toolTraces: string[];
};


export type ImportResult = {
  imported: number;
  updated: number;
  skipped: number;
  errors: string[];
  partners: Partner[];
};

export type BatchResult = {
  partnersScanned: number;
  rawHits: number;
  matched: number;
  cluesUpserted: number;
  level1: number;
  level2: number;
  sourceErrors: string[];
  clues: RiskClue[];
  scopeNote?: string;
};

export type WhistleSchedule = {
  dailyEnabled: boolean;
  /** HH:mm */
  dailyTime: string;
  weeklyEnabled: boolean;
  /** 1=周一 … 7=周日 */
  weeklyDow: number;
  weeklyTime: string;
  lastDailyRun?: string | null;
  lastWeeklyRun?: string | null;
  nextDailyHint: string;
  nextWeeklyHint: string;
  partnerIds?: string[];
  scopeHint?: string;
};

export type WhistleScheduleSave = {
  dailyEnabled?: boolean;
  dailyTime?: string;
  weeklyEnabled?: boolean;
  weeklyDow?: number;
  weeklyTime?: string;
  partnerIds?: string[];
};

export type WhistleReport = {
  id: string;
  kind: "daily" | "weekly" | string;
  period: string;
  title: string;
  summary: string;
  markdown: string;
  batchStatsJson: string;
  clueCount: number;
  level1: number;
  level2: number;
  level3: number;
  level4: number;
  createdAt: string;
};

export type WhistleJobResult = {
  kind: string;
  batch: BatchResult;
  report: WhistleReport;
};

export type AuditLog = {
  id: string;
  actor: string;
  action: string;
  target: string;
  detail: string;
  ts: string;
};

export type GuaranteeSpecial = {
  leverage?: number;
  compensationRate?: number;
  reserveAdequate?: boolean;
  singleConcentration?: number;
  assetRatio?: number;
  fraudSignal?: boolean;
};

export type FinanceMetrics = {
  revenue?: number;
  revenueYoy?: number;
  netProfit?: number;
  netProfitYoy?: number;
  netMargin?: number;
  grossMargin?: number;
  opMargin?: number;
  roa?: number;
  roe?: number;
  assetLiabilityRatio?: number;
  currentRatio?: number;
  quickRatio?: number;
  equityRatio?: number;
  debtEbitda?: number;
  assetTurnover?: number;
  loanOriginated?: number;
  loanBalance?: number;
  npl90?: number;
  ocfo?: number;
  fcf?: number;
  capexRatio?: number;
  dividendPayout?: number;
  guarantee?: GuaranteeSpecial | null;
  currencyUnit?: string;
  notes?: string;
};

export type DimensionScore = {
  dimension: string;
  grade: string;
  score: number;
  comment: string;
};

export type FinanceAnalyzeRequest = {
  partnerId?: string;
  partnerName: string;
  partnerType: string;
  period: string;
  metrics?: FinanceMetrics;
  useDemo?: boolean;
  documentText?: string;
  sourceKind?: string;
};

export type FinanceReport = {
  id: string;
  partnerId: string;
  partnerName: string;
  partnerType: string;
  period: string;
  metrics: FinanceMetrics;
  scores: DimensionScore[];
  overallRating: string;
  riskScore: number;
  concerns: string[];
  relatedFlags: string[];
  summary: string;
  markdown: string;
  sourceKind: string;
  createdAt: string;
};

export type FinancePeerRow = {
  partnerId: string;
  partnerName: string;
  partnerType: string;
  period: string;
  overallRating: string;
  riskScore: number;
  rank: number;
  reportId: string;
};

export type FinanceSeriesPoint = {
  period: string;
  riskScore: number;
  overallRating: string;
  assetLiabilityRatio?: number;
  netMargin?: number;
  fcf?: number;
  reportId: string;
  createdAt: string;
};

export type FinanceBatchResult = {
  period: string;
  totalPartners: number;
  analyzed: number;
  skipped: number;
  failed: number;
  errors: string[];
  reports: FinanceReport[];
};

export type FinanceCsvImportResult = {
  imported: number;
  failed: number;
  errors: string[];
  reports: FinanceReport[];
};

export type AdmissionCheckItem = {
  id: string;
  color: string;
  title: string;
  legalBasis: string;
  triggered: boolean;
  evidence: string;
  sourceRef: string;
};

export type AdmissionReviewRequest = {
  partnerId?: string;
  partnerName?: string;
  partnerType?: string;
  uscc?: string;
  scenario: string;
  manualFlags?: Record<string, boolean>;
  useEnterpriseMcp?: boolean;
  /** 默认 true：按公司名自动全网搜取证 */
  useWebSearch?: boolean;
  useLlmAssist?: boolean;
  materialsText?: string;
};

export type AdmissionReview = {
  id: string;
  partnerId: string;
  partnerName: string;
  partnerType: string;
  uscc?: string;
  scenario: string;
  conclusion: string;
  items: AdmissionCheckItem[];
  remediation: string[];
  summary: string;
  markdown: string;
  evidenceIncomplete: boolean;
  evidenceSources?: string[];
  aiNotes?: string;
  createdAt: string;
};

export type AdmissionKnowledge = {
  id: string;
  kind: string;
  title: string;
  content: string;
  createdAt: string;
};

export type AdmissionKnowledgeImportRequest = {
  kind: string;
  title?: string;
  content: string;
};

export type AdmissionCaseIngestRequest = {
  title?: string;
  content: string;
  checkId?: string;
};

export type AdmissionCaseIngestResult = {
  ok: boolean;
  message: string;
  keywordsAdded: string[];
  knowledgeId?: string | null;
};

import { invoke } from "@tauri-apps/api/core";
import type {
  AgentChatMessage,
  AgentChatResponse,
  AgentConversation,
  AgentConversationSummary,
  AgentStoredMessage,
  AuditLog,
  BatchResult,
  DashboardSnapshot,
  LlmConfigPublic,
  LlmConfigSave,
  LlmTestResult,
  EnterpriseMcpSettings,
  EnterpriseMcpSettingsSave,
  EnterpriseMcpTestResult,
  BaiduSearchSettings,
  BaiduSearchSettingsSave,
  BaiduSearchTestResult,
  FinanceAnalyzeRequest,
  FinanceBatchResult,
  FinanceCsvImportResult,
  FinanceMetrics,
  FinancePeerRow,
  FinanceReport,
  FinanceSeriesPoint,
  AdmissionReview,
  AdmissionReviewRequest,
  AdmissionKnowledge,
  AdmissionKnowledgeImportRequest,
  AdmissionCaseIngestRequest,
  AdmissionCaseIngestResult,
  Partner,
  PartnerEditInput,
  PartnersDocumentImportResult,
  RiskClue,
  RuleChatResult,
  RuleEditInput,
  RuleOverride,
  RulesDocumentImportResult,
  RuleView,
  WhistleJobResult,
  WhistleReport,
  WhistleSchedule,
  WhistleScheduleSave,
} from "./data/types";
import { emptyDashboard } from "./data/empty";

const isTauri = () =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function invokeErrorMessage(e: unknown): string {
  if (typeof e === "string" && e.trim()) return e;
  if (e instanceof Error && e.message.trim()) return e.message;
  if (e && typeof e === "object" && "message" in e) {
    const m = (e as { message: unknown }).message;
    if (typeof m === "string" && m.trim()) return m;
  }
  return e == null ? "未知错误" : String(e);
}

async function tryInvokeSoft<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T | null> {
  if (!isTauri()) return null;
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    console.warn(`invoke ${cmd} failed`, e);
    return null;
  }
}

/** 用户点的按钮：失败时抛出后端原文，不要伪装成「需桌面端」。 */
async function invokeRequired<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauri()) {
    throw new Error("当前窗口不是桌面端，请用 npm run tauri dev 打开应用");
  }
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw new Error(invokeErrorMessage(e));
  }
}

export async function loadDashboard(): Promise<DashboardSnapshot> {
  return (await tryInvokeSoft<DashboardSnapshot>("get_dashboard_snapshot")) ?? emptyDashboard();
}

export async function loadClues(): Promise<RiskClue[]> {
  return (await tryInvokeSoft<RiskClue[]>("list_risk_clues")) ?? [];
}

export async function loadPartners(): Promise<Partner[]> {
  return (await tryInvokeSoft<Partner[]>("list_partners")) ?? [];
}

export async function importPartnersCsv(
  csv: string,
  replaceAll = true,
): Promise<PartnersDocumentImportResult> {
  const res = await tryInvokeSoft<PartnersDocumentImportResult>("import_partners_csv", {
    csv,
    replaceAll,
  });
  if (!res) {
    throw new Error("当前为浏览器预览模式，请使用「npm run tauri dev」导入名单");
  }
  return res;
}

export async function upsertPartner(partner: PartnerEditInput): Promise<Partner[]> {
  const res = await tryInvokeSoft<Partner[]>("upsert_partner", { partner });
  if (!res) throw new Error("保存机构需桌面端");
  return res;
}

export async function importPartnersDocument(
  document: string,
  replaceAll = true,
): Promise<PartnersDocumentImportResult> {
  return importPartnersCsv(document, replaceAll);
}

export async function loadRules(): Promise<RuleView[]> {
  return (await tryInvokeSoft<RuleView[]>("list_rules")) ?? [];
}

export async function applyRuleOverrides(overrides: RuleOverride[]): Promise<RuleView[]> {
  const res = await tryInvokeSoft<RuleView[]>("apply_rule_overrides", { overrides });
  if (!res) throw new Error("无法保存规则覆盖（需桌面端）");
  return res;
}

export async function ruleChat(message: string): Promise<RuleChatResult> {
  const res = await tryInvokeSoft<RuleChatResult>("rule_chat", { message });
  if (!res) throw new Error("对话更新规则需桌面端（npm run tauri dev）");
  return res;
}

export async function upsertRule(rule: RuleEditInput): Promise<RuleView[]> {
  const res = await tryInvokeSoft<RuleView[]>("upsert_rule", { rule });
  if (!res) throw new Error("保存规则需桌面端");
  return res;
}

export async function importRulesDocument(
  document: string,
  replaceAll = true,
): Promise<RulesDocumentImportResult> {
  if (!isTauri()) throw new Error("Excel 导入需桌面端（npm run tauri dev）");
  return await invoke<RulesDocumentImportResult>("import_rules_document", {
    document,
    replaceCustom: replaceAll,
  });
}

export async function agentChat(
  message: string,
  opts?: { scope?: string; history?: AgentChatMessage[] },
): Promise<AgentChatResponse> {
  if (!isTauri()) {
    return {
      ok: false,
      reply: "问鉴控需桌面端（npm run tauri dev）",
      usedLlm: false,
      mutated: false,
      toolTraces: [],
    };
  }
  try {
    return await invoke<AgentChatResponse>("agent_chat", {
      req: {
        message,
        scope: opts?.scope ?? "general",
        history: opts?.history ?? [],
      },
    });
  } catch (e) {
    return {
      ok: false,
      reply: e instanceof Error ? e.message : String(e),
      usedLlm: false,
      mutated: false,
      toolTraces: [],
    };
  }
}

export async function listAgentConversations(
  limit = 50,
): Promise<AgentConversationSummary[]> {
  return (
    (await tryInvokeSoft<AgentConversationSummary[]>("list_agent_conversations", {
      limit,
    })) ?? []
  );
}

export async function getAgentConversation(
  id: string,
): Promise<AgentConversation | null> {
  return (
    (await tryInvokeSoft<AgentConversation | null>("get_agent_conversation", {
      id,
    })) ?? null
  );
}

export async function saveAgentConversation(input: {
  id?: string;
  title?: string;
  messages: AgentStoredMessage[];
}): Promise<AgentConversation | null> {
  return (
    (await tryInvokeSoft<AgentConversation>("save_agent_conversation", {
      req: input,
    })) ?? null
  );
}

export async function deleteAgentConversation(id: string): Promise<void> {
  await tryInvokeSoft("delete_agent_conversation", { id });
}

export async function getLlmConfig(): Promise<LlmConfigPublic> {
  return (
    (await tryInvokeSoft<LlmConfigPublic>("get_llm_config")) ?? {
      baseUrl: "https://api.openai.com/v1",
      model: "gpt-4o-mini",
      enabled: false,
      hasApiKey: false,
      ready: false,
    }
  );
}

export async function saveLlmConfig(config: LlmConfigSave): Promise<LlmConfigPublic> {
  const res = await tryInvokeSoft<LlmConfigPublic>("save_llm_config", { config });
  if (!res) throw new Error("保存 LLM 配置需桌面端");
  return res;
}

export async function testLlmConnection(): Promise<LlmTestResult> {
  const res = await tryInvokeSoft<LlmTestResult>("test_llm_connection");
  if (!res) throw new Error("测试连接需桌面端");
  return res;
}

export async function runWhistleBatch(opts?: {
  liveHeimao?: boolean;
  partnerIds?: string[];
  trial?: boolean;
}): Promise<BatchResult> {
  const res = await tryInvokeSoft<BatchResult>("run_whistle_batch", {
    liveHeimao: opts?.liveHeimao ?? false,
    partnerIds: opts?.partnerIds,
    trial: opts?.trial ?? false,
  });
  if (!res) throw new Error("无法跑批（需桌面端）");
  return res;
}

export async function getWhistleSchedule(): Promise<WhistleSchedule> {
  return (
    (await tryInvokeSoft<WhistleSchedule>("get_whistle_schedule")) ?? {
      dailyEnabled: true,
      dailyTime: "08:00",
      weeklyEnabled: true,
      weeklyDow: 1,
      weeklyTime: "09:00",
      lastDailyRun: null,
      lastWeeklyRun: null,
      nextDailyHint: "需桌面端启用定时",
      nextWeeklyHint: "需桌面端启用定时",
      partnerIds: [],
      scopeHint: "需桌面端",
      lastScopeMode: "",
      lastScopeIds: [],
    }
  );
}

export async function saveWhistleLastScope(
  mode: "all" | "selected",
  ids: string[] = [],
): Promise<void> {
  await tryInvokeSoft("save_whistle_last_scope", { mode, ids });
}

export async function saveWhistleSchedule(
  schedule: WhistleScheduleSave,
): Promise<WhistleSchedule> {
  const res = await tryInvokeSoft<WhistleSchedule>("save_whistle_schedule", {
    schedule,
  });
  if (!res) throw new Error("保存定时需桌面端");
  return res;
}

export async function runWhistleJob(
  kind: "daily" | "weekly",
  partnerIds?: string[],
  onDate?: string,
): Promise<WhistleJobResult> {
  return invokeRequired<WhistleJobResult>("run_whistle_job", {
    kind,
    partnerIds,
    onDate,
  });
}

export async function listWhistleReports(limit = 30): Promise<WhistleReport[]> {
  return (await tryInvokeSoft<WhistleReport[]>("list_whistle_reports", { limit })) ?? [];
}

export async function getEnterpriseMcpSettings(): Promise<EnterpriseMcpSettings> {
  return (
    (await tryInvokeSoft<EnterpriseMcpSettings>("get_enterprise_mcp_settings")) ?? {
      qccEnabled: false,
      qccHasApiKey: false,
      qccReady: false,
      tycEnabled: false,
      tycHasApiKey: false,
      tycReady: false,
    }
  );
}

export async function saveEnterpriseMcpSettings(
  settings: EnterpriseMcpSettingsSave,
): Promise<EnterpriseMcpSettings> {
  const res = await tryInvokeSoft<EnterpriseMcpSettings>(
    "save_enterprise_mcp_settings",
    { settings },
  );
  if (!res) throw new Error("保存企查查/天眼查设置需桌面端");
  return res;
}

export async function testEnterpriseMcp(
  provider: "qcc" | "tyc",
): Promise<EnterpriseMcpTestResult> {
  const res = await tryInvokeSoft<EnterpriseMcpTestResult>("test_enterprise_mcp", {
    provider,
  });
  if (!res) throw new Error("测试连通需桌面端");
  return res;
}

export async function getBaiduSearchSettings(): Promise<BaiduSearchSettings> {
  return (
    (await tryInvokeSoft<BaiduSearchSettings>("get_baidu_search_settings")) ?? {
      enabled: false,
      hasApiKey: false,
      ready: false,
      useInBatch: true,
    }
  );
}

export async function saveBaiduSearchSettings(
  settings: BaiduSearchSettingsSave,
): Promise<BaiduSearchSettings> {
  const res = await tryInvokeSoft<BaiduSearchSettings>("save_baidu_search_settings", {
    settings,
  });
  if (!res) throw new Error("保存百度搜索设置需桌面端");
  return res;
}

export async function testBaiduSearch(): Promise<BaiduSearchTestResult> {
  const res = await tryInvokeSoft<BaiduSearchTestResult>("test_baidu_search");
  if (!res) throw new Error("测试百度搜索需桌面端");
  return res;
}

export async function updateClue(input: {
  clueId: string;
  level?: number;
  status?: string;
  actor?: string;
}): Promise<RiskClue> {
  const res = await tryInvokeSoft<RiskClue>("update_clue", { req: input });
  if (!res) throw new Error("无法更新线索（需桌面端）");
  return res;
}

export async function loadAuditLogs(limit = 50): Promise<AuditLog[]> {
  return (await tryInvokeSoft<AuditLog[]>("list_audit_logs", { limit })) ?? [];
}

export async function analyzeFinanceReport(
  req: FinanceAnalyzeRequest,
): Promise<FinanceReport> {
  if (!isTauri()) throw new Error("经营分析需桌面端");
  return await invoke<FinanceReport>("analyze_finance_report", { req });
}

export async function listFinanceReports(limit = 30): Promise<FinanceReport[]> {
  return (await tryInvokeSoft<FinanceReport[]>("list_finance_reports", { limit })) ?? [];
}

export async function compareFinancePeers(
  partnerType: string,
  period: string,
  limit = 20,
): Promise<FinancePeerRow[]> {
  return (
    (await tryInvokeSoft<FinancePeerRow[]>("compare_finance_peers", {
      partnerType,
      period,
      limit,
    })) ?? []
  );
}

export async function listFinanceSeries(
  partnerId?: string,
  partnerName?: string,
  limit = 12,
): Promise<FinanceSeriesPoint[]> {
  return (
    (await tryInvokeSoft<FinanceSeriesPoint[]>("list_finance_series", {
      partnerId,
      partnerName,
      limit,
    })) ?? []
  );
}

export async function runFinanceQuarterBatch(
  period: string,
  fillDemoIfEmpty = false,
): Promise<FinanceBatchResult> {
  if (!isTauri()) throw new Error("季度批跑需桌面端");
  return await invoke<FinanceBatchResult>("run_finance_quarter_batch", {
    period,
    fillDemoIfEmpty,
  });
}

export async function importFinanceCsv(csv: string): Promise<FinanceCsvImportResult> {
  if (!isTauri()) throw new Error("CSV 导入需桌面端");
  return await invoke<FinanceCsvImportResult>("import_finance_csv", { csv });
}

export async function getFinanceDemoMetrics(): Promise<FinanceMetrics> {
  return (
    (await tryInvokeSoft<FinanceMetrics>("get_finance_demo_metrics")) ?? {
      revenue: 131.52,
      revenueYoy: -7.4,
      netProfit: 16.77,
      netProfitYoy: 52.4,
      netMargin: 12.75,
      assetLiabilityRatio: 48.4,
      currentRatio: 1.86,
      fcf: 33,
      currencyUnit: "亿元人民币",
      notes: "浏览器预览演示数据",
    }
  );
}

export async function runAdmissionReview(
  req: AdmissionReviewRequest,
): Promise<AdmissionReview> {
  if (!isTauri()) throw new Error("准入研判需桌面端");
  return await invoke<AdmissionReview>("run_admission_review", { req });
}

export async function listAdmissionReviews(limit = 30): Promise<AdmissionReview[]> {
  return (
    (await tryInvokeSoft<AdmissionReview[]>("list_admission_reviews", { limit })) ?? []
  );
}

export async function importAdmissionKnowledge(
  req: AdmissionKnowledgeImportRequest,
): Promise<AdmissionKnowledge> {
  if (!isTauri()) throw new Error("导入准入材料需桌面端");
  return await invoke<AdmissionKnowledge>("import_admission_knowledge", { req });
}

export async function listAdmissionKnowledge(limit = 40): Promise<AdmissionKnowledge[]> {
  return (
    (await tryInvokeSoft<AdmissionKnowledge[]>("list_admission_knowledge", { limit })) ?? []
  );
}

export async function ingestAdmissionCase(
  req: AdmissionCaseIngestRequest,
): Promise<AdmissionCaseIngestResult> {
  if (!isTauri()) throw new Error("录入案例需桌面端");
  return await invoke<AdmissionCaseIngestResult>("ingest_admission_case", { req });
}

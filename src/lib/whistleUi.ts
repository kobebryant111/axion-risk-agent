import type { RiskClue } from "../data/types";

export function levelTone(level: number) {
  if (level <= 1) return "bg-red-100 text-red-600";
  if (level === 2) return "bg-orange-100 text-orange-600";
  if (level === 3) return "bg-amber-100 text-amber-700";
  return "bg-slate-100 text-slate-500";
}

export function sourceLabel(source?: string) {
  if (source === "heimao") return "黑猫";
  if (source === "baidu" || source === "tavily") return "全网搜";
  if (source === "ai_news") return "历史研判";
  if (source === "qcc" || source?.startsWith("qcc")) return "企查查";
  if (source === "tyc" || source?.startsWith("tyc")) return "天眼查";
  return source || "—";
}

/** 把数据源技术报错收成业务同学能看的一句话 */
export function friendlySourceErrors(errors: string[]): string[] {
  if (!errors.length) return [];
  const webErr = errors.filter((e) => /tavily|baidu|百度搜索/i.test(e));
  const rest = errors.filter((e) => !/tavily|baidu|百度搜索/i.test(e));
  const out: string[] = [];
  if (webErr.length) {
    const blob = webErr.join("\n");
    if (/401|403|api[_ ]?key|unauthorized|鉴权/i.test(blob)) {
      out.push("全网搜密钥无效，请到「设置」检查百度千帆搜索");
    } else if (/429|quota|limit|额度/i.test(blob)) {
      out.push("全网搜次数已用尽，请稍后再试");
    } else {
      out.push("全网搜暂时不可用，已用其他来源继续监测");
    }
  }
  for (const e of rest.slice(0, 3)) {
    out.push(stripTechnical(e));
  }
  return out;
}

function stripTechnical(raw: string): string {
  const cut = raw.replace(/\s*\{[\s\S]*\}\s*$/, "").trim();
  if (cut.length > 80) return `${cut.slice(0, 78)}…`;
  return cut || "外部数据源暂不可用";
}

export type ReportClue = Pick<
  RiskClue,
  | "id"
  | "partner"
  | "partnerType"
  | "level"
  | "title"
  | "summary"
  | "eventDate"
  | "status"
  | "sourceSystem"
  | "sourceUrl"
  | "credibility"
  | "ruleId"
  | "legalBasis"
>;

export type ReportBatchStats = {
  partnersScanned?: number;
  rawHits?: number;
  matched?: number;
  cluesUpserted?: number;
  level1?: number;
  level2?: number;
  sourceErrors?: string[];
  scopeNote?: string;
  searchQueries?: string[];
  clues?: ReportClue[];
};

export function parseReportStats(json: string): ReportBatchStats {
  try {
    const v = JSON.parse(json) as ReportBatchStats;
    return v && typeof v === "object" ? v : {};
  } catch {
    return {};
  }
}

function isPromotionalClue(c: { title?: string }): boolean {
  const t = c.title || "";
  return (
    t.includes("成长之路") ||
    t.includes("向阳而生") ||
    t.includes("新征程") ||
    t.includes("政企同频") ||
    t.includes("紧跟监管") ||
    t.includes("扎根营口")
  );
}

function isDemoClue(c: { sourceSystem?: string; sourceUrl?: string }): boolean {
  const src = (c.sourceSystem || "").toLowerCase();
  const url = c.sourceUrl || "";
  return src === "heimao" || url.includes("/demo/heimao") || url.includes("tousu.sina.com.cn/demo");
}

export function reportClueToRiskClue(c: ReportClue): RiskClue {
  return {
    id: c.id,
    partner: c.partner,
    partnerType: c.partnerType,
    level: c.level,
    title: c.title,
    summary: c.summary,
    eventDate: c.eventDate,
    owner: "合作风控",
    progress: 0,
    status: c.status,
    sourceSystem: c.sourceSystem,
    sourceUrl: c.sourceUrl,
    credibility: c.credibility,
    ruleId: c.ruleId,
    legalBasis: c.legalBasis,
  };
}

export function resolveLiveClue(live: RiskClue[], snap: ReportClue): RiskClue {
  const byId = live.find((c) => c.id && c.id === snap.id);
  if (byId) return byId;
  const url = (snap.sourceUrl || "").trim();
  if (url) {
    const byUrl = live.find((c) => (c.sourceUrl || "").trim() === url);
    if (byUrl) return byUrl;
  }
  const byTitle = live.find(
    (c) => c.title === snap.title && c.partner === snap.partner,
  );
  if (byTitle) return byTitle;
  return reportClueToRiskClue(snap);
}

export function cluesForReport(
  stats: ReportBatchStats,
  _period: string,
  _liveClues: RiskClue[],
): ReportClue[] {
  return (stats.clues ?? []).filter((c) => !isDemoClue(c) && !isPromotionalClue(c));
}

import type { RiskClue } from "../data/types";

export function levelTone(level: number) {
  if (level <= 1) return "bg-red-100 text-red-600";
  if (level === 2) return "bg-orange-100 text-orange-600";
  if (level === 3) return "bg-amber-100 text-amber-700";
  return "bg-slate-100 text-slate-500";
}

export function sourceLabel(source?: string) {
  if (source === "heimao") return "黑猫";
  if (source === "tavily") return "全网搜";
  if (source === "ai_news") return "历史研判";
  if (source === "qcc" || source?.startsWith("qcc")) return "企查查";
  if (source === "tyc" || source?.startsWith("tyc")) return "天眼查";
  return source || "—";
}

/** 把数据源技术报错收成业务同学能看的一句话 */
export function friendlySourceErrors(errors: string[]): string[] {
  if (!errors.length) return [];
  const tavily = errors.filter((e) => /tavily/i.test(e));
  const rest = errors.filter((e) => !/tavily/i.test(e));
  const out: string[] = [];
  if (tavily.length) {
    const blob = tavily.join("\n");
    if (/401|403|api[_ ]?key|unauthorized/i.test(blob)) {
      out.push("全网搜密钥无效，请到「设置」检查 Tavily");
    } else if (/429|quota|limit/i.test(blob)) {
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

function isDemoClue(c: { sourceSystem?: string; sourceUrl?: string }): boolean {
  const src = (c.sourceSystem || "").toLowerCase();
  const url = c.sourceUrl || "";
  return src === "heimao" || url.includes("/demo/heimao") || url.includes("tousu.sina.com.cn/demo");
}

export function cluesForReport(
  stats: ReportBatchStats,
  period: string,
  liveClues: RiskClue[],
): ReportClue[] {
  const fromStats = (stats.clues ?? []).filter((c) => !isDemoClue(c));
  if (fromStats.length > 0) return fromStats;
  const range = period.includes("~")
    ? period.split("~").map((s) => s.trim())
    : [period.trim(), period.trim()];
  const start = range[0] ?? "";
  const end = range[1] ?? start;
  return liveClues.filter((c) => {
    if (isDemoClue(c)) return false;
    const d = (c.createdAt || c.eventDate || "").slice(0, 10);
    return d >= start && d <= end;
  });
}

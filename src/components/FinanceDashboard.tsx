import type { FinanceReport, FinanceSeriesPoint } from "../data/types";

function gradeTone(g: string) {
  if (g.startsWith("A")) return "bg-emerald-100 text-emerald-700";
  if (g.startsWith("B")) return "bg-amber-100 text-amber-800";
  if (g.includes("预警") || g.includes("关注")) return "bg-red-100 text-red-700";
  return "bg-slate-100 text-slate-600";
}

function fmt(v: number | null | undefined, unit = "") {
  if (v == null || Number.isNaN(v)) return "—";
  const abs = Math.abs(v);
  const text =
    abs >= 100 ? v.toFixed(1) : abs >= 10 ? v.toFixed(2) : v.toFixed(2);
  return `${text}${unit}`;
}

function MetricBar({
  label,
  value,
  max = 100,
  tone = "accent",
}: {
  label: string;
  value: number;
  max?: number;
  tone?: "accent" | "warn" | "ok";
}) {
  const pct = Math.max(4, Math.min(100, Math.round((value / max) * 100)));
  const color =
    tone === "warn"
      ? "bg-amber-500"
      : tone === "ok"
        ? "bg-emerald-500"
        : "bg-axiom-accent";
  return (
    <div>
      <div className="mb-1 flex items-center justify-between text-[11px]">
        <span className="text-axiom-muted">{label}</span>
        <span className="font-semibold text-axiom-text">{value}</span>
      </div>
      <div className="h-2 overflow-hidden rounded-full bg-black/[0.06]">
        <div className={`h-full rounded-full ${color}`} style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

function SeriesBars({ series }: { series: FinanceSeriesPoint[] }) {
  const max = Math.max(...series.map((s) => s.riskScore), 1);
  return (
    <div className="flex h-36 items-end gap-2 px-1">
      {series.map((s) => {
        const h = Math.max(12, Math.round((s.riskScore / max) * 120));
        return (
          <div key={s.reportId + s.period} className="flex flex-1 flex-col items-center gap-1">
            <div className="text-[10px] font-semibold text-axiom-text">{s.riskScore}</div>
            <div
              className="w-full max-w-[36px] rounded-t-xl bg-axiom-accent/80"
              style={{ height: h }}
              title={`${s.period}: ${s.riskScore}`}
            />
            <div className="truncate text-[10px] text-axiom-muted">{s.period}</div>
          </div>
        );
      })}
    </div>
  );
}

type Props = {
  report: FinanceReport;
  series: FinanceSeriesPoint[];
  onExport: () => void;
};

export function FinanceDashboard({ report, series, onExport }: Props) {
  const m = report.metrics;
  const unit = m.currencyUnit || "亿元";

  const kpis = [
    {
      label: "营收 / 担保费",
      value: fmt(m.revenue, ` ${unit}`),
      sub: m.revenueYoy != null ? `同比 ${fmt(m.revenueYoy, "%")}` : "—",
      tone: "text-sky-800 bg-sky-50",
    },
    {
      label: "净利润",
      value: fmt(m.netProfit, ` ${unit}`),
      sub: m.netProfitYoy != null ? `同比 ${fmt(m.netProfitYoy, "%")}` : "—",
      tone:
        (m.netProfit ?? 0) < 0
          ? "text-red-700 bg-red-50"
          : "text-emerald-700 bg-emerald-50",
    },
    {
      label: "净利率",
      value: fmt(m.netMargin, "%"),
      sub: m.grossMargin != null ? `毛利率 ${fmt(m.grossMargin, "%")}` : "—",
      tone: "text-violet-800 bg-violet-50",
    },
    {
      label: "经营现金流",
      value: fmt(m.ocfo, ` ${unit}`),
      sub: m.fcf != null ? `FCF ${fmt(m.fcf, ` ${unit}`)}` : "—",
      tone:
        (m.ocfo ?? 0) < 0
          ? "text-amber-800 bg-amber-50"
          : "text-emerald-800 bg-emerald-50",
    },
  ];

  const snapshot = [
    ["资产负债率", fmt(m.assetLiabilityRatio, "%")],
    ["流动比率", fmt(m.currentRatio)],
    ["速动比率", fmt(m.quickRatio)],
    ["ROE", fmt(m.roe, "%")],
    ["ROA", fmt(m.roa, "%")],
    ["资产周转", fmt(m.assetTurnover)],
    ["贷款余额", fmt(m.loanBalance, ` ${unit}`)],
    ["不良率90+", fmt(m.npl90, "%")],
  ];

  return (
    <section className="card space-y-5 p-6">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <div className="text-xs font-semibold text-axiom-accent">财报智能看板</div>
          <div className="mt-1 text-xl font-bold tracking-tight">
            {report.partnerName} · {report.period}
          </div>
          <p className="mt-1 max-w-3xl text-sm leading-relaxed text-axiom-muted">
            {report.summary}
          </p>
          <div className="mt-1 text-[11px] text-axiom-muted">
            来源 {report.sourceKind || "—"} · {report.createdAt}
            {m.notes ? ` · ${m.notes}` : ""}
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <span
            className={`rounded-full px-3 py-1 text-sm font-bold ${gradeTone(report.overallRating)}`}
          >
            综合 {report.overallRating}
          </span>
          <span className="rounded-full bg-sky-100 px-3 py-1 text-sm font-bold text-sky-800">
            经营分 {report.riskScore ?? "—"}/100
          </span>
          <button
            type="button"
            onClick={onExport}
            className="rounded-full bg-black/[0.04] px-3 py-1.5 text-xs font-semibold"
          >
            导出报告
          </button>
        </div>
      </div>

      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        {kpis.map((k) => (
          <div key={k.label} className={`rounded-2xl px-4 py-3 ${k.tone}`}>
            <div className="text-[11px] font-semibold opacity-80">{k.label}</div>
            <div className="mt-1 text-xl font-bold tracking-tight">{k.value}</div>
            <div className="mt-0.5 text-[11px] opacity-75">{k.sub}</div>
          </div>
        ))}
      </div>

      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        {report.scores.map((s) => (
          <div key={s.dimension} className="rounded-2xl bg-black/[0.02] p-4">
            <div className="mb-2 flex items-center justify-between">
              <div className="text-xs text-axiom-muted">{s.dimension}</div>
              <span className={`rounded-full px-2 py-0.5 text-xs font-bold ${gradeTone(s.grade)}`}>
                {s.grade}
              </span>
            </div>
            <MetricBar
              label="得分"
              value={s.score}
              tone={s.score < 55 ? "warn" : s.score >= 80 ? "ok" : "accent"}
            />
            <div className="mt-2 text-xs leading-relaxed text-axiom-muted">{s.comment}</div>
          </div>
        ))}
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <div>
          <div className="mb-2 text-sm font-semibold">财务快照</div>
          <div className="overflow-hidden rounded-2xl border border-black/[0.04]">
            <table className="min-w-full text-left text-xs">
              <thead className="bg-black/[0.03] text-axiom-muted">
                <tr>
                  <th className="px-3 py-2 font-semibold">指标</th>
                  <th className="px-3 py-2 font-semibold">数值</th>
                </tr>
              </thead>
              <tbody>
                {snapshot.map(([k, v]) => (
                  <tr key={k} className="border-t border-black/[0.04]">
                    <td className="px-3 py-2">{k}</td>
                    <td className="px-3 py-2 font-medium">{v}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>

        <div>
          <div className="mb-2 text-sm font-semibold">经营分时序</div>
          {series.length === 0 ? (
            <div className="flex h-36 items-center justify-center rounded-2xl bg-black/[0.02] text-sm text-axiom-muted">
              多期评估后显示趋势
            </div>
          ) : (
            <div className="rounded-2xl bg-black/[0.02] p-3">
              <SeriesBars series={series} />
            </div>
          )}
        </div>
      </div>

      {m.guarantee && (
        <div className="rounded-2xl border border-violet-100 bg-violet-50/60 p-4">
          <div className="mb-2 text-sm font-semibold text-violet-900">融担专项看板</div>
          <div className="grid gap-2 text-xs sm:grid-cols-3">
            <div>放大倍数：{m.guarantee.leverage ?? "—"}</div>
            <div>代偿率：{fmt(m.guarantee.compensationRate, "%")}</div>
            <div>
              准备金：
              {m.guarantee.reserveAdequate == null
                ? "—"
                : m.guarantee.reserveAdequate
                  ? "足额"
                  : "不足/承压"}
            </div>
            <div>单一集中度：{fmt(m.guarantee.singleConcentration, "%")}</div>
            <div>资产比例：{fmt(m.guarantee.assetRatio, "%")}</div>
            <div>造假信号：{m.guarantee.fraudSignal ? "是" : "否/未识别"}</div>
          </div>
        </div>
      )}

      {(report.relatedFlags?.length ?? 0) > 0 && (
        <div className="rounded-2xl bg-slate-50 px-4 py-3 text-sm text-slate-800">
          <div className="mb-1 font-semibold">关联方穿透</div>
          <ul className="list-disc space-y-1 pl-5 text-xs">
            {report.relatedFlags.map((c) => (
              <li key={c}>{c}</li>
            ))}
          </ul>
        </div>
      )}

      {report.concerns.length > 0 && (
        <div className="rounded-2xl bg-amber-50 px-4 py-3 text-sm text-amber-900">
          <div className="mb-1 font-semibold">智能关注点</div>
          <ul className="list-disc space-y-1 pl-5 text-xs">
            {report.concerns.map((c) => (
              <li key={c}>{c}</li>
            ))}
          </ul>
        </div>
      )}

      <details className="rounded-2xl bg-black/[0.02] p-4">
        <summary className="cursor-pointer text-sm font-semibold">
          标准化报告 Markdown
        </summary>
        <pre className="mt-3 max-h-80 overflow-auto whitespace-pre-wrap text-xs text-axiom-text">
          {report.markdown}
        </pre>
      </details>
    </section>
  );
}

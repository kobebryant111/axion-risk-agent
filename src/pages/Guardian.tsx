import { useEffect, useMemo, useRef, useState } from "react";
import {
  analyzeFinanceReport,
  compareFinancePeers,
  getLlmConfig,
  listFinanceReports,
  listFinanceSeries,
} from "../api";
import { FinanceDashboard } from "../components/FinanceDashboard";
import { FancySelect, fieldClass } from "../components/FancySelect";
import type {
  FinancePeerRow,
  FinanceReport,
  FinanceSeriesPoint,
  Partner,
} from "../data/types";
import { fileToImportText, formatInvokeError } from "../lib/fileImport";

type Props = {
  partners: Partner[];
};

const TYPE_OPTS = [
  { value: "loan", label: "助贷机构" },
  { value: "guarantee", label: "融资担保" },
  { value: "traffic", label: "流量引流" },
  { value: "payment", label: "支付机构" },
  { value: "data", label: "数据服务商" },
  { value: "collection", label: "催收机构" },
  { value: "interbank", label: "金市同业" },
  { value: "ops", label: "运营辅助" },
];

function gradeTone(g: string) {
  if (g.startsWith("A")) return "bg-emerald-100 text-emerald-700";
  if (g.startsWith("B")) return "bg-amber-100 text-amber-800";
  if (g.includes("预警") || g.includes("关注")) return "bg-red-100 text-red-700";
  return "bg-slate-100 text-slate-600";
}

export function GuardianPage({ partners }: Props) {
  const [partnerId, setPartnerId] = useState("");
  const [partnerName, setPartnerName] = useState("");
  const [partnerType, setPartnerType] = useState("loan");
  const [period, setPeriod] = useState("");
  const [fileName, setFileName] = useState<string | null>(null);
  const [docText, setDocText] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [report, setReport] = useState<FinanceReport | null>(null);
  const [history, setHistory] = useState<FinanceReport[]>([]);
  const [peers, setPeers] = useState<FinancePeerRow[]>([]);
  const [series, setSeries] = useState<FinanceSeriesPoint[]>([]);
  const [dragOver, setDragOver] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    void refreshSide();
  }, []);

  useEffect(() => {
    const p = partners.find((x) => x.id === partnerId);
    if (p) {
      setPartnerName(p.name);
      setPartnerType(p.partnerType);
      void listFinanceSeries(p.id, p.name, 8).then(setSeries);
    }
  }, [partnerId, partners]);

  const partnerOpts = useMemo(
    () => [
      { value: "", label: "自动识别 / 手动填写" },
      ...partners.map((p) => ({
        value: p.id,
        label: p.name,
        hint: p.partnerTypeLabel,
      })),
    ],
    [partners],
  );

  async function refreshSide(
    pType = partnerType,
    pPeriod = period || "2025全年",
    pid = partnerId,
    pname = partnerName,
  ) {
    try {
      setHistory(await listFinanceReports(40));
      setPeers(await compareFinancePeers(pType, pPeriod, 15));
      setSeries(await listFinanceSeries(pid || undefined, pname || undefined, 8));
    } catch {
      /* browser */
    }
  }

  async function afterReport(res: FinanceReport) {
    setReport(res);
    setPartnerName(res.partnerName);
    setPartnerType(res.partnerType || partnerType);
    setPeriod(res.period);
    await refreshSide(res.partnerType, res.period, res.partnerId, res.partnerName);
  }

  async function loadFile(file: File) {
    setErr(null);
    setMsg(null);
    try {
      const text = await fileToImportText(file);
      setDocText(text);
      setFileName(file.name);
      setMsg(`已载入「${file.name}」（${Math.round(text.length / 1000)} 千字），可点击开始分析`);
    } catch (e) {
      setFileName(null);
      setDocText("");
      setErr(formatInvokeError(e));
    }
  }

  async function runAnalyze() {
    setBusy(true);
    setErr(null);
    setMsg(null);
    try {
      if (!docText.trim()) throw new Error("请先上传财报附件（Word/Excel/TXT）");
      const llm = await getLlmConfig();
      if (!llm?.ready) {
        throw new Error("请先在「设置」启用并配置大模型，再分析财报附件");
      }
      const res = await analyzeFinanceReport({
        partnerId: partnerId || undefined,
        partnerName: partnerName.trim(),
        partnerType,
        period: period.trim(),
        documentText: docText,
        sourceKind: "attachment",
      });
      setMsg("已完成附件解析，财报看板已生成");
      await afterReport(res);
    } catch (e) {
      setErr(formatInvokeError(e));
    } finally {
      setBusy(false);
    }
  }

  function exportMd() {
    if (!report) return;
    const blob = new Blob([report.markdown], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${report.partnerName}-${report.period}-经营评估.md`;
    a.click();
    URL.revokeObjectURL(url);
  }

  return (
    <main className="flex min-w-0 flex-1 flex-col gap-4 overflow-auto">
      <header className="card p-6">
        <div className="text-xs font-semibold text-axiom-accent">经营守护</div>
        <h1 className="mt-1 text-2xl font-bold tracking-tight">上传财报 · 智能看板</h1>
      </header>

      <section className="card space-y-4 p-6">
        <div
          onDragOver={(e) => {
            e.preventDefault();
            setDragOver(true);
          }}
          onDragLeave={() => setDragOver(false)}
          onDrop={(e) => {
            e.preventDefault();
            setDragOver(false);
            const f = e.dataTransfer.files?.[0];
            if (f) void loadFile(f);
          }}
          className={`rounded-3xl border-2 border-dashed px-6 py-10 text-center transition ${
            dragOver
              ? "border-axiom-accent bg-axiom-soft/40"
              : "border-black/[0.08] bg-black/[0.015]"
          }`}
        >
          <div className="text-sm font-semibold text-axiom-text">
            拖拽财报附件到此处，或点击选择文件
          </div>
          <div className="mt-1 text-xs text-axiom-muted">
            支持 .docx / .xlsx / .xls / .csv / .txt（暂不支持 PDF、旧版 .doc）
          </div>
          {fileName && (
            <div className="mt-3 text-xs font-semibold text-axiom-accent">
              已选：{fileName}
              {docText ? ` · ${docText.length.toLocaleString()} 字` : ""}
            </div>
          )}
          <div className="mt-4 flex flex-wrap items-center justify-center gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={() => fileRef.current?.click()}
              className="rounded-full bg-axiom-accent px-5 py-2.5 text-sm font-semibold text-white disabled:opacity-50"
            >
              选择附件
            </button>
            <button
              type="button"
              disabled={busy || !docText.trim()}
              onClick={() => void runAnalyze()}
              className="rounded-full bg-axiom-soft px-5 py-2.5 text-sm font-semibold text-axiom-accent disabled:opacity-50"
            >
              {busy ? "分析中…" : "开始分析并生成看板"}
            </button>
          </div>
          <input
            ref={fileRef}
            type="file"
            accept=".docx,.xlsx,.xls,.csv,.txt,.md,.json,text/plain,application/vnd.openxmlformats-officedocument.wordprocessingml.document,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            className="hidden"
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) void loadFile(f);
              e.target.value = "";
            }}
          />
        </div>

        <div className="grid gap-4 md:grid-cols-2">
          <FancySelect
            label="合作机构（可选，可让 AI 自动识别）"
            value={partnerId}
            options={partnerOpts}
            onChange={setPartnerId}
          />
          <label className="block">
            <div className="mb-1.5 text-[11px] font-semibold tracking-wide text-axiom-muted">
              机构名称（可空，由附件识别）
            </div>
            <input
              value={partnerName}
              onChange={(e) => setPartnerName(e.target.value)}
              className={fieldClass}
              placeholder="留空则从审计报告自动识别"
            />
          </label>
          <FancySelect
            label="机构类型"
            value={partnerType}
            options={TYPE_OPTS}
            onChange={setPartnerType}
          />
          <label className="block">
            <div className="mb-1.5 text-[11px] font-semibold tracking-wide text-axiom-muted">
              报告期间（可空）
            </div>
            <input
              value={period}
              onChange={(e) => setPeriod(e.target.value)}
              className={fieldClass}
              placeholder="如 2025全年，可留空自动识别"
            />
          </label>
        </div>
      </section>

      {err && (
        <div className="rounded-2xl bg-red-50 px-4 py-3 text-sm text-red-600">{err}</div>
      )}
      {msg && (
        <div className="rounded-2xl bg-emerald-50 px-4 py-3 text-sm text-emerald-700">{msg}</div>
      )}

      {report && (
        <FinanceDashboard report={report} series={series} onExport={exportMd} />
      )}

      <div className="grid gap-4 lg:grid-cols-2">
        <section className="card p-5">
          <div className="mb-3 flex items-center justify-between">
            <div className="text-base font-semibold">同业横向对比</div>
            <button
              type="button"
              className="text-xs font-semibold text-axiom-accent"
              onClick={() =>
                void compareFinancePeers(partnerType, period || "2025全年", 15).then(setPeers)
              }
            >
              刷新
            </button>
          </div>
          {peers.length === 0 ? (
            <div className="py-8 text-center text-sm text-axiom-muted">
              同期间同类型暂无对比样本
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="min-w-full text-left text-xs">
                <thead className="text-axiom-muted">
                  <tr>
                    <th className="px-2 py-1.5">排名</th>
                    <th className="px-2 py-1.5">机构</th>
                    <th className="px-2 py-1.5">评级</th>
                    <th className="px-2 py-1.5">经营分</th>
                  </tr>
                </thead>
                <tbody>
                  {peers.map((p) => (
                    <tr key={p.reportId} className="border-t border-black/[0.04]">
                      <td className="px-2 py-2 font-semibold">{p.rank}</td>
                      <td className="px-2 py-2">{p.partnerName}</td>
                      <td className="px-2 py-2">
                        <span
                          className={`rounded-full px-2 py-0.5 text-[10px] font-bold ${gradeTone(p.overallRating)}`}
                        >
                          {p.overallRating}
                        </span>
                      </td>
                      <td className="px-2 py-2">{p.riskScore}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>

        <section className="card p-5">
          <div className="mb-3 text-base font-semibold">历史经营数据库</div>
          {history.length === 0 ? (
            <div className="py-8 text-center text-sm text-axiom-muted">暂无报告</div>
          ) : (
            <div className="max-h-72 space-y-2 overflow-auto">
              {history.map((h) => (
                <button
                  key={h.id}
                  type="button"
                  onClick={() => {
                    setReport(h);
                    void listFinanceSeries(h.partnerId, h.partnerName, 8).then(setSeries);
                    void compareFinancePeers(h.partnerType, h.period, 15).then(setPeers);
                  }}
                  className="flex w-full items-center justify-between rounded-2xl border border-black/[0.03] px-3 py-3 text-left text-sm hover:bg-black/[0.015]"
                >
                  <span>
                    {h.partnerName} · {h.period}
                    <span className="ml-2 text-xs text-axiom-muted">{h.sourceKind || ""}</span>
                  </span>
                  <span
                    className={`rounded-full px-2 py-0.5 text-xs font-bold ${gradeTone(h.overallRating)}`}
                  >
                    {h.overallRating}
                  </span>
                </button>
              ))}
            </div>
          )}
        </section>
      </div>
    </main>
  );
}

import { useEffect, useRef, useState } from "react";
import {
  getEnterpriseMcpSettings,
  getBaiduSearchSettings,
  listAdmissionReviews,
  runAdmissionReview,
} from "../api";
import { AdmissionDashboard } from "../components/AdmissionDashboard";
import { fieldClass } from "../components/FancySelect";
import type { AdmissionReview, Partner } from "../data/types";
import { formatInvokeError } from "../lib/fileImport";

type Props = {
  partners: Partner[];
};

function conclusionTone(c: string) {
  if (c === "否决") return "bg-red-100 text-red-700";
  if (c.includes("条件")) return "bg-orange-100 text-orange-700";
  if (c.includes("待核验")) return "bg-amber-100 text-amber-800";
  return "bg-emerald-100 text-emerald-700";
}

function guessType(name: string): string {
  if (/担保|融担|再担保/.test(name)) return "guarantee";
  if (/催收|清收|委外/.test(name)) return "collection";
  if (/支付|清算/.test(name)) return "payment";
  if (/数据|征信|评分/.test(name)) return "data";
  if (/科技|流量|营销|导流/.test(name)) return "traffic";
  return "loan";
}

export function ScoutPage({ partners: _partners }: Props) {
  const [partnerName, setPartnerName] = useState("");
  const [partnerType, setPartnerType] = useState("loan");
  const [mcpReady, setMcpReady] = useState(false);
  const [webReady, setWebReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const progressTimer = useRef<number | null>(null);
  const [review, setReview] = useState<AdmissionReview | null>(null);
  const [history, setHistory] = useState<AdmissionReview[]>([]);

  useEffect(() => {
    void listAdmissionReviews(20).then(setHistory);
    void getEnterpriseMcpSettings()
      .then((s) => setMcpReady(!!(s.qccReady || s.tycReady)))
      .catch(() => setMcpReady(false));
    void getBaiduSearchSettings()
      .then((s) => setWebReady(!!s.ready))
      .catch(() => setWebReady(false));
  }, []);

  useEffect(() => {
    return () => {
      if (progressTimer.current != null) window.clearInterval(progressTimer.current);
    };
  }, []);

  async function run() {
    setBusy(true);
    setErr(null);
    setMsg(null);
    const started = Date.now();
    const stages = [
      "正在全网搜取证…",
      "正在拉取工商补证（短超时，失败会跳过）…",
      "正在匹配红橙黄清单…",
      "正在生成 AI 分析报告…",
    ];
    setProgress(stages[0]);
    if (progressTimer.current != null) window.clearInterval(progressTimer.current);
    progressTimer.current = window.setInterval(() => {
      const sec = Math.floor((Date.now() - started) / 1000);
      const stage = stages[Math.min(Math.floor(sec / 12), stages.length - 1)];
      setProgress(`${stage}（已等待 ${sec}s，通常 30–90s）`);
    }, 1000);
    try {
      const name = partnerName.trim();
      if (!name) {
        throw new Error("请输入公司全称");
      }
      const type = partnerType || guessType(name);
      setPartnerType(type);
      const res = await runAdmissionReview({
        partnerName: name,
        partnerType: type,
        scenario: "准入",
        useEnterpriseMcp: true,
        useWebSearch: true,
        useLlmAssist: true,
      });
      setReview(res);
      setHistory(await listAdmissionReviews(20));
      setMsg(
        res.conclusion === "待核验"
          ? `分析完成：【待核验】红 ${statsFor(res).red} / 橙 ${statsFor(res).orange} / 黄 ${statsFor(res).yellow}。禁止按通过推进合作。`
          : `分析完成：【${res.conclusion}】红 ${statsFor(res).red} / 橙 ${statsFor(res).orange} / 黄 ${statsFor(res).yellow}`,
      );
    } catch (e) {
      setErr(formatInvokeError(e));
    } finally {
      if (progressTimer.current != null) {
        window.clearInterval(progressTimer.current);
        progressTimer.current = null;
      }
      setProgress("");
      setBusy(false);
    }
  }

  function statsFor(r: AdmissionReview) {
    return {
      red: r.items.filter((i) => i.color === "red" && i.triggered).length,
      orange: r.items.filter((i) => i.color === "orange" && i.triggered).length,
      yellow: r.items.filter((i) => i.color === "yellow" && i.triggered).length,
    };
  }

  function exportMd() {
    if (!review) return;
    const blob = new Blob([review.markdown], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${review.partnerName}-准入分析报告.md`;
    a.click();
    URL.revokeObjectURL(url);
  }

  return (
    <main className="flex min-w-0 flex-1 flex-col gap-4 overflow-auto">
      <header className="card p-6">
        <div className="text-xs font-semibold text-axiom-accent">准入瞭望</div>
        <h1 className="mt-1 text-2xl font-bold tracking-tight">输入公司名 · 自动准入分析</h1>
      </header>

      <section className="card space-y-4 p-6">
        <label className="block">
          <div className="mb-1.5 text-[11px] font-semibold tracking-wide text-axiom-muted">
            公司全称
          </div>
          <input
            value={partnerName}
            onChange={(e) => {
              const v = e.target.value;
              setPartnerName(v);
              if (v.trim()) {
                setPartnerType(guessType(v));
              }
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !busy) void run();
            }}
            className={`${fieldClass} text-base`}
            placeholder="例如：中黔联融资担保集团有限公司"
          />
        </label>

        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            disabled={busy}
            onClick={() => void run()}
            className="rounded-full bg-axiom-accent px-6 py-2.5 text-sm font-semibold text-white disabled:opacity-50"
          >
            {busy ? "分析中…" : "开始准入分析"}
          </button>
          <span className="text-[11px] text-axiom-muted">
            {busy && progress
              ? progress
              : `全网搜 ${webReady ? "已就绪" : "未配置"} · 企查查/天眼查 ${mcpReady ? "已就绪" : "未配置"}`}
          </span>
        </div>
        {busy && (
          <div className="rounded-2xl bg-sky-50 px-4 py-3 text-xs leading-relaxed text-sky-800">
            未卡死：后台在跑全网搜 / 工商补证 / AI。工商侧已改为短超时，失败会跳过并继续出报告。
          </div>
        )}
      </section>

      {err && (
        <div className="rounded-2xl bg-red-50 px-4 py-3 text-sm text-red-600">{err}</div>
      )}
      {msg && (
        <div className="rounded-2xl bg-emerald-50 px-4 py-3 text-sm text-emerald-700">{msg}</div>
      )}

      {review && <AdmissionDashboard review={review} onExport={exportMd} />}

      <section className="card p-5">
        <div className="mb-3 text-base font-semibold">历史分析</div>
        {history.length === 0 ? (
          <div className="py-6 text-center text-sm text-axiom-muted">暂无记录</div>
        ) : (
          <div className="space-y-2">
            {history.map((h) => (
              <button
                key={h.id}
                type="button"
                onClick={() => setReview(h)}
                className="flex w-full items-center justify-between rounded-2xl border border-black/[0.03] px-3 py-3 text-left text-sm hover:bg-black/[0.015]"
              >
                <span>
                  {h.partnerName}
                  <span className="ml-2 text-xs text-axiom-muted">{h.createdAt}</span>
                </span>
                <span
                  className={`rounded-full px-2 py-0.5 text-xs font-bold ${conclusionTone(h.conclusion)}`}
                >
                  {h.conclusion}
                </span>
              </button>
            ))}
          </div>
        )}
      </section>
    </main>
  );
}

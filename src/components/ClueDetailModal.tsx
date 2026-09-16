import { useState } from "react";
import type { RiskClue } from "../data/types";
import { levelTone, sourceLabel } from "../lib/whistleUi";

type Props = {
  clue: RiskClue;
  onClose: () => void;
  onChangeLevel?: (clue: RiskClue, level: number) => Promise<void>;
  onCloseClue?: (clue: RiskClue) => Promise<void>;
  onStartHandle?: (clue: RiskClue) => Promise<void>;
};

export function ClueDetailModal({
  clue,
  onClose,
  onChangeLevel,
  onCloseClue,
  onStartHandle,
}: Props) {
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const canHandle = Boolean(onChangeLevel || onCloseClue || onStartHandle);
  const closed = clue.status === "已关闭";
  const tracking = clue.status === "跟踪中" || clue.progress > 0;

  async function run(action: () => Promise<void>) {
    setBusy(true);
    setErr(null);
    try {
      await action();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center bg-black/30 p-4"
      onClick={onClose}
    >
      <div
        className="flex max-h-[88vh] w-full max-w-lg flex-col overflow-hidden rounded-[28px] bg-white shadow-soft"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="clue-detail-title"
      >
        <div className="flex items-start justify-between gap-3 border-b border-black/[0.04] p-5">
          <div className="min-w-0">
            <div
              className={`inline-flex rounded-full px-2 py-0.5 text-xs font-bold ${levelTone(clue.level)}`}
            >
              L{clue.level}
            </div>
            <h2
              id="clue-detail-title"
              className="mt-2 text-lg font-bold leading-snug text-axiom-text"
            >
              {clue.title}
            </h2>
            <div className="mt-1.5 text-xs font-semibold text-axiom-muted">
              {clue.status} · {clue.eventDate}
            </div>
          </div>
          <button
            type="button"
            className="shrink-0 rounded-full bg-black/[0.04] px-3 py-1 text-sm font-semibold"
            onClick={onClose}
          >
            关闭
          </button>
        </div>

        <div className="min-h-0 flex-1 space-y-3.5 overflow-y-auto p-5 text-sm">
          <Row k="机构" v={clue.partner} />
          <Row k="机构类型" v={clue.partnerType || "—"} />
          <Row k="摘要" v={clue.summary || "—"} />
          <Row
            k="规则"
            v={`${clue.ruleId || "—"} @ ${clue.ruleSetVersion || "—"}`}
          />
          <Row k="法规依据" v={clue.legalBasis || "—"} />
          <Row
            k="来源"
            v={`${sourceLabel(clue.sourceSystem)} · ${clue.credibility || "—"} 类源`}
          />
          <Row k="证据链接" v={clue.sourceUrl || "—"} url={clue.sourceUrl} />
          <Row k="负责人" v={clue.owner || "—"} />
          <Row k="降噪" v={clue.denoiseStatus || "—"} />
          <Row k="状态" v={clue.status} />
        </div>

        {canHandle && (
          <div className="space-y-2 border-t border-black/[0.04] p-5">
            <div className="text-sm font-semibold text-axiom-muted">
              人工处置（留痕审计）
            </div>
            {err && (
              <div className="rounded-2xl bg-red-50 px-3 py-2 text-xs font-semibold text-red-700">
                {err}
              </div>
            )}
            <div className="flex flex-wrap gap-2">
              {onStartHandle && !closed && !tracking && (
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => void run(() => onStartHandle(clue))}
                  className="rounded-full bg-axiom-accent px-3 py-1.5 text-xs font-semibold text-white disabled:opacity-50"
                >
                  开始处理
                </button>
              )}
              {onChangeLevel &&
                [1, 2, 3, 4].map((lv) => (
                  <button
                    key={lv}
                    type="button"
                    disabled={busy || closed}
                    onClick={() => void run(() => onChangeLevel(clue, lv))}
                    className="rounded-full bg-axiom-soft px-3 py-1.5 text-xs font-semibold text-axiom-accent disabled:opacity-50"
                  >
                    改为 L{lv}
                  </button>
                ))}
              {onCloseClue && !closed && (
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => void run(() => onCloseClue(clue))}
                  className="rounded-full bg-axiom-accent px-3 py-1.5 text-xs font-semibold text-white disabled:opacity-50"
                >
                  人工关闭
                </button>
              )}
              {closed && (
                <span className="text-xs font-semibold text-axiom-muted">
                  该线索已关闭
                </span>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function Row({ k, v, url }: { k: string; v: string; url?: string }) {
  const href = url?.trim();
  const isLink = Boolean(href && /^https?:\/\//i.test(href));
  return (
    <div>
      <div className="text-sm font-semibold text-axiom-muted">{k}</div>
      {isLink ? (
        <a
          href={href}
          target="_blank"
          rel="noreferrer"
          className="mt-0.5 block break-all font-medium text-axiom-accent underline-offset-2 hover:underline"
        >
          {href}
        </a>
      ) : (
        <div className="mt-0.5 break-all font-medium text-axiom-text">{v}</div>
      )}
    </div>
  );
}

export type BgImportKind = "partners" | "rules";

export type BgImportJob = {
  kind: BgImportKind;
  running: boolean;
  text: string;
  result?: string;
  error?: string;
};

type Props = {
  job: BgImportJob | null;
  onDismiss: () => void;
  onOpenTarget: (kind: BgImportKind) => void;
};

/** 右下角非阻塞任务条：导入时不挡操作 */
export function BgImportToast({ job, onDismiss, onOpenTarget }: Props) {
  if (!job) return null;

  const title =
    job.kind === "partners" ? "机构名单 · Excel 导入" : "规则库 · Excel 导入";

  return (
    <div className="pointer-events-none fixed bottom-5 right-5 z-[60] w-[min(360px,calc(100vw-2rem))]">
      <div className="pointer-events-auto rounded-[22px] border border-black/[0.06] bg-white p-4 shadow-soft">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="text-xs font-semibold text-axiom-accent">{title}</div>
            {job.running ? (
              <p className="mt-1.5 text-sm leading-relaxed text-axiom-text">{job.text}</p>
            ) : job.error ? (
              <p className="mt-1.5 text-sm leading-relaxed text-red-600">{job.error}</p>
            ) : (
              <p className="mt-1.5 text-sm leading-relaxed text-axiom-text">
                {job.result || "完成"}
              </p>
            )}
          </div>
          {!job.running && (
            <button
              type="button"
              onClick={onDismiss}
              className="shrink-0 text-xs text-axiom-muted hover:text-axiom-text"
            >
              关闭
            </button>
          )}
        </div>

        {job.running && (
          <div className="mt-3 h-1 overflow-hidden rounded-full bg-black/[0.06]">
            <div className="h-full w-2/3 animate-pulse rounded-full bg-axiom-accent" />
          </div>
        )}

        <div className="mt-3 flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => onOpenTarget(job.kind)}
            className="rounded-full bg-axiom-soft px-3 py-1 text-xs font-semibold text-axiom-accent"
          >
            {job.kind === "partners" ? "查看机构名单" : "查看规则库"}
          </button>
          {job.running && (
            <span className="self-center text-[11px] text-axiom-muted">
              可继续操作其他页面
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

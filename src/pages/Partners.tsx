import { useMemo, useRef, useState } from "react";
import { upsertPartner } from "../api";
import type { Partner, PartnerEditInput } from "../data/types";
import { downloadPartnerTemplate } from "../lib/excelTemplates";
import { formatInvokeError } from "../lib/fileImport";

type Props = {
  partners: Partner[];
  onChanged: (partners: Partner[]) => void;
  onGoSettings?: () => void;
  /** 后台 Excel 导入（不挡 UI） */
  importRunning?: boolean;
  onStartExcelImport?: (file: File) => void;
};

const TYPE_OPTIONS = [
  { value: "loan", label: "助贷" },
  { value: "guarantee", label: "融资担保" },
  { value: "traffic", label: "引流" },
  { value: "payment", label: "支付" },
  { value: "data", label: "数据" },
  { value: "collection", label: "催收" },
];

function emptyEdit(): PartnerEditInput {
  return {
    id: "",
    name: "",
    alias: "",
    groupName: "",
    uscc: "",
    partnerType: "loan",
    relatedParties: [],
    status: "active",
  };
}

export function PartnersPage({
  partners,
  onChanged,
  importRunning = false,
  onStartExcelImport,
}: Props) {
  const fileRef = useRef<HTMLInputElement>(null);
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [editing, setEditing] = useState<PartnerEditInput | null>(null);
  const [typeFilter, setTypeFilter] = useState("all");

  const typeCount = useMemo(() => {
    const m = new Map<string, number>();
    for (const p of partners) {
      m.set(p.partnerTypeLabel, (m.get(p.partnerTypeLabel) ?? 0) + 1);
    }
    return [...m.entries()];
  }, [partners]);

  const typeKeys = useMemo(() => {
    const present = new Set(partners.map((p) => p.partnerType));
    return ["all", ...TYPE_OPTIONS.map((t) => t.value).filter((v) => present.has(v))];
  }, [partners]);

  const filtered =
    typeFilter === "all"
      ? partners
      : partners.filter((p) => p.partnerType === typeFilter);

  function openEdit(p?: Partner) {
    if (!p) {
      setEditing(emptyEdit());
      return;
    }
    setEditing({
      id: p.id,
      name: p.name,
      alias: p.alias,
      groupName: p.groupName,
      uscc: p.uscc,
      partnerType: p.partnerType,
      relatedParties: [...p.relatedParties],
      status: p.status || "active",
    });
  }

  async function saveEdit() {
    if (!editing) return;
    setSaving(true);
    setMsg(null);
    try {
      onChanged(await upsertPartner(editing));
      setEditing(null);
      setMsg("机构已保存");
    } catch (e) {
      setMsg(formatInvokeError(e));
    } finally {
      setSaving(false);
    }
  }

  function startExcelImport() {
    if (importRunning) return;
    setMsg(null);
    fileRef.current?.click();
  }

  function typeFilterLabel(key: string) {
    if (key === "all") return "全部";
    return TYPE_OPTIONS.find((t) => t.value === key)?.label ?? key;
  }

  return (
    <main className="flex h-full min-h-0 min-w-0 flex-1 flex-col gap-3 overflow-hidden">
      <header className="card flex shrink-0 flex-wrap items-center justify-between gap-3 p-4">
        <div>
          <h1 className="text-xl font-bold tracking-tight">机构名单</h1>
          <div className="mt-0.5 text-xs text-axiom-muted">
            合计 {partners.length} 家
            {typeFilter !== "all" ? ` · 当前筛选 ${filtered.length}` : ""}
            {" · "}先下载固定模板，按列填写后导入 Excel / CSV
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <input
            id="partner-excel-import"
            ref={fileRef}
            type="file"
            accept=".xlsx,.xls,.csv,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet,application/vnd.ms-excel,text/csv"
            className="hidden"
            onChange={(e) => {
              const f = e.target.files?.[0];
              e.target.value = "";
              if (f && onStartExcelImport) onStartExcelImport(f);
            }}
          />
          <button
            type="button"
            onClick={() => downloadPartnerTemplate()}
            className="rounded-full bg-white px-4 py-2 text-sm font-semibold text-axiom-text ring-1 ring-black/[0.08] hover:bg-axiom-soft"
          >
            下载模板
          </button>
          <button
            type="button"
            disabled={saving}
            onClick={() => openEdit()}
            className="rounded-full bg-axiom-accent px-4 py-2 text-sm font-semibold text-white disabled:opacity-50"
          >
            新建机构
          </button>
          <button
            type="button"
            disabled={importRunning}
            title="按固定模板全量替换当前名单"
            onClick={startExcelImport}
            className="rounded-full bg-white px-4 py-2 text-sm font-semibold text-axiom-accent ring-1 ring-axiom-accent/30 hover:bg-axiom-soft disabled:opacity-50"
          >
            {importRunning ? "后台导入中…" : "导入 Excel"}
          </button>
        </div>
      </header>

      {msg && (
        <div className="shrink-0 rounded-2xl bg-black/[0.03] px-4 py-2 text-sm">
          {msg}
        </div>
      )}

      <section className="grid shrink-0 grid-cols-2 gap-2 md:grid-cols-4">
        <div className="card px-4 py-3">
          <div className="text-[11px] text-axiom-muted">机构总数</div>
          <div className="mt-0.5 text-xl font-bold">{partners.length}</div>
        </div>
        {typeCount.slice(0, 3).map(([label, n]) => (
          <div key={label} className="card px-4 py-3">
            <div className="text-[11px] text-axiom-muted">{label}</div>
            <div className="mt-0.5 text-xl font-bold">{n}</div>
          </div>
        ))}
      </section>

      <div className="flex shrink-0 flex-wrap gap-1">
        {typeKeys.map((t) => (
          <button
            key={t}
            type="button"
            onClick={() => setTypeFilter(t)}
            className={`rounded-full px-3 py-1 text-xs font-semibold ${
              typeFilter === t
                ? "bg-axiom-accent text-white"
                : "bg-black/[0.04] text-axiom-muted"
            }`}
          >
            {typeFilterLabel(t)}
          </button>
        ))}
      </div>

      <section className="card flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className="min-h-0 flex-1 space-y-2 overflow-y-auto p-3">
          {filtered.length === 0 ? (
            <div className="py-16 text-center text-sm text-axiom-muted">
              暂无名单。请先下载固定模板填写后导入，或逐条新建。
            </div>
          ) : (
            filtered.map((p) => (
              <article
                key={p.id}
                className="flex gap-3 rounded-[20px] border border-black/[0.04] bg-[#fcfbfe] px-3.5 py-3 transition hover:border-axiom-accent/20 hover:bg-white"
              >
                <div className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-2xl bg-axiom-soft text-[11px] font-bold text-axiom-accent">
                  {p.partnerTypeLabel.slice(0, 2)}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <h3 className="text-[15px] font-semibold leading-snug text-axiom-text">
                      {p.name}
                    </h3>
                    <span className="rounded-full bg-black/[0.04] px-2 py-0.5 text-[11px] text-axiom-muted">
                      {p.partnerTypeLabel}
                    </span>
                    {p.status !== "active" && (
                      <span className="rounded-full bg-slate-100 px-2 py-0.5 text-[11px] text-slate-500">
                        {p.status === "paused" ? "暂停" : p.status === "exit" ? "退出" : p.status}
                      </span>
                    )}
                  </div>
                  <p className="mt-1 text-[12px] text-axiom-muted">
                    {[p.alias, p.groupName, p.uscc].filter(Boolean).join(" · ") || "—"}
                  </p>
                  {p.relatedParties.length > 0 && (
                    <p className="mt-1 text-[11px] text-axiom-muted">
                      关联方 {p.relatedParties.join("、")}
                    </p>
                  )}
                  <div className="mt-2 flex flex-wrap gap-1.5">
                    {p.lexicon.slice(0, 8).map((w) => (
                      <span
                        key={w}
                        className="rounded-lg bg-white px-2 py-0.5 text-[11px] text-axiom-muted ring-1 ring-black/[0.04]"
                      >
                        {w}
                      </span>
                    ))}
                    {p.lexicon.length > 8 && (
                      <span className="px-1 text-[11px] text-axiom-muted">
                        +{p.lexicon.length - 8}
                      </span>
                    )}
                  </div>
                </div>
                <div className="flex shrink-0 flex-col justify-center">
                  <button
                    type="button"
                    disabled={saving}
                    onClick={() => openEdit(p)}
                    className="rounded-full bg-white px-3 py-1.5 text-xs font-semibold text-axiom-accent ring-1 ring-black/[0.06] hover:bg-axiom-soft"
                  >
                    编辑
                  </button>
                </div>
              </article>
            ))
          )}
        </div>
      </section>

      {editing && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/25 p-4">
          <div className="max-h-[90vh] w-full max-w-xl overflow-auto rounded-[28px] bg-white p-6 shadow-soft">
            <div className="mb-4 flex items-center justify-between">
              <h2 className="text-lg font-bold">
                {editing.id ? "编辑机构" : "新建机构"}
              </h2>
              <button type="button" onClick={() => setEditing(null)} className="text-sm">
                关闭
              </button>
            </div>
            <div className="space-y-3 text-sm">
              <Field
                label="机构全称"
                value={editing.name}
                onChange={(v) => setEditing({ ...editing, name: v })}
              />
              <Field
                label="简称"
                value={editing.alias}
                onChange={(v) => setEditing({ ...editing, alias: v })}
              />
              <Field
                label="集团"
                value={editing.groupName}
                onChange={(v) => setEditing({ ...editing, groupName: v })}
              />
              <Field
                label="统一社会信用代码"
                value={editing.uscc}
                onChange={(v) => setEditing({ ...editing, uscc: v })}
              />
              <label className="block">
                <div className="mb-1 text-xs text-axiom-muted">类型</div>
                <select
                  value={editing.partnerType}
                  onChange={(e) =>
                    setEditing({ ...editing, partnerType: e.target.value })
                  }
                  className="w-full rounded-2xl border border-black/[0.06] px-3 py-2"
                >
                  {TYPE_OPTIONS.map((t) => (
                    <option key={t.value} value={t.value}>
                      {t.label}
                    </option>
                  ))}
                </select>
              </label>
              <Field
                label="关联方（顿号/分号分隔）"
                value={editing.relatedParties.join("、")}
                onChange={(v) =>
                  setEditing({
                    ...editing,
                    relatedParties: v
                      .split(/[、,;；|]/)
                      .map((s) => s.trim())
                      .filter(Boolean),
                  })
                }
              />
              <label className="block">
                <div className="mb-1 text-xs text-axiom-muted">状态</div>
                <select
                  value={editing.status}
                  onChange={(e) => setEditing({ ...editing, status: e.target.value })}
                  className="w-full rounded-2xl border border-black/[0.06] px-3 py-2"
                >
                  <option value="active">合作中</option>
                  <option value="paused">暂停</option>
                  <option value="exit">退出</option>
                </select>
              </label>
            </div>
            <div className="mt-5 flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setEditing(null)}
                className="rounded-full bg-black/[0.04] px-4 py-2 text-sm"
              >
                取消
              </button>
              <button
                type="button"
                disabled={saving || !editing.name.trim()}
                onClick={() => void saveEdit()}
                className="rounded-full bg-axiom-accent px-4 py-2 text-sm font-semibold text-white disabled:opacity-50"
              >
                保存
              </button>
            </div>
          </div>
        </div>
      )}
    </main>
  );
}

function Field({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <label className="block">
      <div className="mb-1 text-xs text-axiom-muted">{label}</div>
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-2xl border border-black/[0.06] px-3 py-2 outline-none focus:border-axiom-accent"
      />
    </label>
  );
}

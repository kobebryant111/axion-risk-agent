import { useEffect, useMemo, useState } from "react";
import type { Partner } from "../data/types";
import { FancySelect } from "./FancySelect";

export type PartnerScope = {
  mode: "all" | "selected";
  ids: string[];
};

const TYPE_OPTIONS = [
  { value: "all", label: "全部类型" },
  { value: "loan", label: "助贷" },
  { value: "guarantee", label: "融担" },
  { value: "traffic", label: "引流" },
  { value: "payment", label: "支付" },
  { value: "data", label: "数据" },
  { value: "collection", label: "催收" },
  { value: "interbank", label: "金市同业" },
  { value: "ops", label: "运营辅助" },
];

const PREVIEW = 5;

type Props = {
  partners: Partner[];
  value: PartnerScope;
  onChange: (next: PartnerScope) => void;
  disabled?: boolean;
};

export function effectivePartnerIds(scope: PartnerScope): string[] {
  return scope.mode === "all" ? [] : scope.ids;
}

export function scopeSummary(scope: PartnerScope, total: number): string {
  if (scope.mode === "all") return total > 0 ? `全部 ${total} 家` : "全部机构";
  return `指定 ${scope.ids.length} 家`;
}

export function PartnerScopePicker({ partners, value, onChange, disabled }: Props) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const [viewAllOpen, setViewAllOpen] = useState(false);
  const [draftIds, setDraftIds] = useState<string[]>([]);
  const [q, setQ] = useState("");
  const [typeFilter, setTypeFilter] = useState("all");
  const [viewQ, setViewQ] = useState("");

  const selected = useMemo(() => new Set(value.ids), [value.ids]);
  const draftSet = useMemo(() => new Set(draftIds), [draftIds]);

  const picked = useMemo(
    () => partners.filter((p) => selected.has(p.id)),
    [partners, selected],
  );

  const listed = value.mode === "all" ? partners : picked;
  const preview = listed.slice(0, PREVIEW);
  const more = Math.max(0, listed.length - preview.length);

  const filtered = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return partners.filter((p) => {
      if (typeFilter !== "all" && p.partnerType !== typeFilter) return false;
      if (!needle) return true;
      return `${p.name} ${p.alias} ${p.groupName} ${p.uscc}`
        .toLowerCase()
        .includes(needle);
    });
  }, [partners, q, typeFilter]);

  const viewList = useMemo(() => {
    const needle = viewQ.trim().toLowerCase();
    if (!needle) return listed;
    return listed.filter((p) =>
      `${p.name} ${p.alias} ${p.groupName} ${p.uscc}`.toLowerCase().includes(needle),
    );
  }, [listed, viewQ]);

  function openPicker() {
    setDraftIds(value.mode === "selected" ? [...value.ids] : []);
    setQ("");
    setTypeFilter("all");
    setPickerOpen(true);
  }

  function confirmPicker() {
    onChange({ mode: "selected", ids: draftIds });
    setPickerOpen(false);
  }

  function toggleDraft(id: string) {
    setDraftIds((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id],
    );
  }

  function toggleFiltered(on: boolean) {
    const ids = new Set(draftIds);
    for (const p of filtered) {
      if (on) ids.add(p.id);
      else ids.delete(p.id);
    }
    setDraftIds([...ids]);
  }

  return (
    <div className={disabled ? "pointer-events-none opacity-60" : ""}>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="text-sm font-semibold">监测范围</div>
        <div className="flex gap-1">
          <button
            type="button"
            disabled={disabled}
            onClick={() => onChange({ ...value, mode: "all" })}
            className={`rounded-full px-3 py-1 text-xs font-semibold ${
              value.mode === "all"
                ? "bg-axiom-accent text-white"
                : "bg-black/[0.04] text-axiom-muted"
            }`}
          >
            监测全部
          </button>
          <button
            type="button"
            disabled={disabled}
            onClick={openPicker}
            className={`rounded-full px-3 py-1 text-xs font-semibold ${
              value.mode === "selected"
                ? "bg-axiom-accent text-white"
                : "bg-black/[0.04] text-axiom-muted"
            }`}
          >
            选择机构
          </button>
        </div>
      </div>

      {listed.length === 0 ? (
        <div className="mt-3 rounded-2xl bg-black/[0.02] px-3 py-6 text-center text-xs text-axiom-muted">
          {partners.length === 0
            ? "请先导入机构名单"
            : "尚未选择机构，点击右上角「选择机构」"}
        </div>
      ) : (
        <div className="mt-3 rounded-2xl bg-black/[0.02] p-1.5">
          {preview.map((p) => (
            <div
              key={p.id}
              className="flex items-center gap-2 rounded-xl px-2.5 py-2"
            >
              <span className="min-w-0 flex-1 truncate text-sm">{p.name}</span>
              <span className="shrink-0 text-[11px] text-axiom-muted">
                {p.partnerTypeLabel}
              </span>
            </div>
          ))}
          <div className="flex items-center justify-between px-2.5 py-2">
            <span className="text-[11px] text-axiom-muted">
              {value.mode === "all"
                ? `已纳入全部 ${partners.length} 家`
                : `已选 ${picked.length} 家`}
              {more > 0 ? `，此处展示前 ${PREVIEW} 条` : ""}
            </span>
            <button
              type="button"
              disabled={disabled}
              onClick={() => {
                setViewQ("");
                setViewAllOpen(true);
              }}
              className="rounded-full bg-white px-2.5 py-0.5 text-[11px] font-semibold text-axiom-accent shadow-soft"
            >
              全部
            </button>
          </div>
        </div>
      )}

      {pickerOpen && (
        <PickerModal
          title="选择监测机构"
          count={draftIds.length}
          query={q}
          onQuery={setQ}
          typeFilter={typeFilter}
          onTypeFilter={setTypeFilter}
          filtered={filtered}
          draftSet={draftSet}
          onToggle={toggleDraft}
          onSelectFiltered={toggleFiltered}
          onCancel={() => setPickerOpen(false)}
          onConfirm={confirmPicker}
        />
      )}

      {viewAllOpen && (
        <ViewAllModal
          title={value.mode === "all" ? "全部监测机构" : "已选机构"}
          subtitle={
            value.mode === "all"
              ? `共 ${listed.length} 家`
              : `已选 ${listed.length} 家`
          }
          query={viewQ}
          onQuery={setViewQ}
          list={viewList}
          empty={listed.length === 0 ? "暂无机构" : "没有匹配的机构"}
          onClose={() => setViewAllOpen(false)}
        />
      )}
    </div>
  );
}

function PickerModal({
  title,
  count,
  query,
  onQuery,
  typeFilter,
  onTypeFilter,
  filtered,
  draftSet,
  onToggle,
  onSelectFiltered,
  onCancel,
  onConfirm,
}: {
  title: string;
  count: number;
  query: string;
  onQuery: (v: string) => void;
  typeFilter: string;
  onTypeFilter: (v: string) => void;
  filtered: Partner[];
  draftSet: Set<string>;
  onToggle: (id: string) => void;
  onSelectFiltered: (on: boolean) => void;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const allOn = filtered.length > 0 && filtered.every((p) => draftSet.has(p.id));

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onCancel]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/25 p-4"
      onClick={onCancel}
    >
      <div
        className="flex max-h-[84vh] w-full max-w-lg flex-col overflow-hidden rounded-[28px] bg-white shadow-soft"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="flex shrink-0 items-center justify-between gap-3 border-b border-black/[0.04] px-5 py-4">
          <div>
            <h2 className="text-base font-bold">{title}</h2>
            <div className="mt-0.5 text-[11px] text-axiom-muted">已选 {count} 家</div>
          </div>
          <button
            type="button"
            className="rounded-full bg-black/[0.04] px-3 py-1 text-sm"
            onClick={onCancel}
          >
            关闭
          </button>
        </header>

        <div className="flex shrink-0 gap-2 px-5 pt-4">
          <input
            autoFocus
            value={query}
            onChange={(e) => onQuery(e.target.value)}
            placeholder="搜索名称 / 信用代码"
            className="min-w-0 flex-1 rounded-2xl border border-black/[0.05] bg-gradient-to-b from-white to-[#faf9fc] px-3 py-2 text-sm outline-none focus:border-axiom-accent/40 focus:ring-2 focus:ring-axiom-accent/15"
          />
          <FancySelect
            compact
            className="w-[8.5rem] shrink-0"
            value={typeFilter}
            options={TYPE_OPTIONS}
            onChange={onTypeFilter}
          />
        </div>

        <div className="flex shrink-0 items-center justify-between px-5 py-2">
          <span className="text-[11px] text-axiom-muted">
            当前 {filtered.length} 家
          </span>
          <button
            type="button"
            className="text-[11px] font-semibold text-axiom-accent"
            onClick={() => onSelectFiltered(!allOn)}
          >
            {allOn ? "取消当前" : "全选当前"}
          </button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto px-3 pb-2">
          {filtered.length === 0 ? (
            <div className="px-2 py-10 text-center text-sm text-axiom-muted">
              没有匹配的机构
            </div>
          ) : (
            filtered.map((p) => {
              const on = draftSet.has(p.id);
              return (
                <button
                  key={p.id}
                  type="button"
                  onClick={() => onToggle(p.id)}
                  className={`mb-0.5 flex w-full items-center gap-2 rounded-xl px-2.5 py-2 text-left ${
                    on ? "bg-axiom-soft/70" : "hover:bg-black/[0.03]"
                  }`}
                >
                  <span
                    className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-md text-[10px] ${
                      on
                        ? "bg-axiom-accent text-white"
                        : "border border-black/[0.12] bg-white text-transparent"
                    }`}
                  >
                    ✓
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm">{p.name}</span>
                    {p.uscc ? (
                      <span className="block truncate text-[11px] text-axiom-muted">
                        {p.uscc}
                      </span>
                    ) : null}
                  </span>
                  <span className="shrink-0 text-[11px] text-axiom-muted">
                    {p.partnerTypeLabel}
                  </span>
                </button>
              );
            })
          )}
        </div>

        <footer className="flex shrink-0 items-center justify-end gap-2 border-t border-black/[0.04] px-5 py-3">
          <button
            type="button"
            className="rounded-full bg-black/[0.04] px-4 py-1.5 text-sm font-semibold"
            onClick={onCancel}
          >
            取消
          </button>
          <button
            type="button"
            className="rounded-full bg-axiom-accent px-4 py-1.5 text-sm font-semibold text-white"
            onClick={onConfirm}
          >
            确定
          </button>
        </footer>
      </div>
    </div>
  );
}

function ViewAllModal({
  title,
  subtitle,
  query,
  onQuery,
  list,
  empty,
  onClose,
}: {
  title: string;
  subtitle: string;
  query: string;
  onQuery: (v: string) => void;
  list: Partner[];
  empty: string;
  onClose: () => void;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/25 p-4"
      onClick={onClose}
    >
      <div
        className="flex max-h-[84vh] w-full max-w-lg flex-col overflow-hidden rounded-[28px] bg-white shadow-soft"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="flex shrink-0 items-center justify-between gap-3 border-b border-black/[0.04] px-5 py-4">
          <div>
            <h2 className="text-base font-bold">{title}</h2>
            <div className="mt-0.5 text-[11px] text-axiom-muted">{subtitle}</div>
          </div>
          <button
            type="button"
            className="rounded-full bg-black/[0.04] px-3 py-1 text-sm"
            onClick={onClose}
          >
            关闭
          </button>
        </header>

        <div className="shrink-0 px-5 pt-4">
          <input
            autoFocus
            value={query}
            onChange={(e) => onQuery(e.target.value)}
            placeholder="搜索名称 / 信用代码"
            className="w-full rounded-2xl border border-black/[0.05] bg-gradient-to-b from-white to-[#faf9fc] px-3 py-2 text-sm outline-none focus:border-axiom-accent/40 focus:ring-2 focus:ring-axiom-accent/15"
          />
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto px-3 py-3">
          {list.length === 0 ? (
            <div className="px-2 py-10 text-center text-sm text-axiom-muted">{empty}</div>
          ) : (
            list.map((p) => (
              <div
                key={p.id}
                className="flex items-center gap-2 rounded-xl px-2.5 py-2"
              >
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-sm">{p.name}</span>
                  {p.uscc ? (
                    <span className="block truncate text-[11px] text-axiom-muted">
                      {p.uscc}
                    </span>
                  ) : null}
                </span>
                <span className="shrink-0 text-[11px] text-axiom-muted">
                  {p.partnerTypeLabel}
                </span>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

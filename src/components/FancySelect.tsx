import { useEffect, useId, useRef, useState } from "react";

export type FancyOption = {
  value: string;
  label: string;
  hint?: string;
};

type Props = {
  label?: string;
  value: string;
  options: FancyOption[];
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  compact?: boolean;
  className?: string;
};

export function FancySelect({
  label,
  value,
  options,
  onChange,
  placeholder = "请选择",
  disabled = false,
  compact = false,
  className = "",
}: Props) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const listId = useId();
  const selected = options.find((o) => o.value === value);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={rootRef} className={`relative ${className || "block"}`}>
      {label && (
        <div className="mb-1.5 text-[11px] font-semibold tracking-wide text-axiom-muted">
          {label}
        </div>
      )}
      <button
        type="button"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        onClick={() => !disabled && setOpen((v) => !v)}
        className={`flex w-full items-center justify-between gap-2 border bg-gradient-to-b text-left shadow-[inset_0_1px_0_rgba(255,255,255,0.8)] transition disabled:opacity-50 ${
          compact
            ? "rounded-2xl px-3 py-1.5 text-sm"
            : "rounded-[18px] px-4 py-3 text-sm"
        } ${
          open
            ? "border-axiom-accent/40 from-white to-axiom-soft/40 ring-2 ring-axiom-accent/15"
            : "border-black/[0.05] from-white to-[#faf9fc] hover:border-black/[0.08]"
        }`}
      >
        <span className="min-w-0 flex-1">
          {selected ? (
            <>
              <span className="block truncate font-medium text-axiom-text">
                {selected.label}
              </span>
              {!compact && selected.hint && (
                <span className="mt-0.5 block truncate text-[11px] text-axiom-muted">
                  {selected.hint}
                </span>
              )}
            </>
          ) : (
            <span className="text-axiom-muted">{placeholder}</span>
          )}
        </span>
        <span
          className={`flex shrink-0 items-center justify-center rounded-full bg-axiom-soft text-axiom-accent transition ${
            compact ? "h-6 w-6" : "h-7 w-7"
          } ${open ? "rotate-180" : ""}`}
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden>
            <path
              d="M2.5 4.5L6 8l3.5-3.5"
              stroke="currentColor"
              strokeWidth="1.6"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </span>
      </button>

      {open && (
        <ul
          id={listId}
          role="listbox"
          className="absolute z-[80] mt-1.5 max-h-60 w-full min-w-[7.5rem] overflow-auto rounded-[18px] border border-black/[0.05] bg-white p-1.5 shadow-[0_16px_40px_rgba(31,41,55,0.12)]"
        >
          {options.map((o) => {
            const active = o.value === value;
            return (
              <li key={o.value || "__empty__"}>
                <button
                  type="button"
                  role="option"
                  aria-selected={active}
                  onClick={() => {
                    onChange(o.value);
                    setOpen(false);
                  }}
                  className={`flex w-full items-center justify-between gap-2 rounded-[14px] px-3 py-2 text-left text-sm transition ${
                    active
                      ? "bg-axiom-soft font-semibold text-axiom-accent"
                      : "text-axiom-text hover:bg-black/[0.03]"
                  }`}
                >
                  <span className="min-w-0 truncate">{o.label}</span>
                  {active && (
                    <svg
                      width="14"
                      height="14"
                      viewBox="0 0 14 14"
                      fill="none"
                      className="shrink-0"
                    >
                      <path
                        d="M3 7.2L5.8 10l5.2-6"
                        stroke="currentColor"
                        strokeWidth="1.8"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      />
                    </svg>
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}

export const fieldClass =
  "w-full rounded-[18px] border border-black/[0.05] bg-gradient-to-b from-white to-[#faf9fc] px-4 py-3 text-sm shadow-[inset_0_1px_0_rgba(255,255,255,0.8)] outline-none transition placeholder:text-axiom-muted focus:border-axiom-accent/40 focus:ring-2 focus:ring-axiom-accent/15";

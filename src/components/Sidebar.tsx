type NavKey =
  | "overview"
  | "whistle"
  | "guardian"
  | "scout"
  | "partners"
  | "rules"
  | "settings";

const NAV: { key: NavKey; label: string; icon: string }[] = [
  { key: "overview", label: "总览", icon: "◈" },
  { key: "whistle", label: "风险吹哨", icon: "⚑" },
  { key: "guardian", label: "经营守护", icon: "◎" },
  { key: "scout", label: "准入瞭望", icon: "◉" },
  { key: "partners", label: "机构名单", icon: "☰" },
  { key: "rules", label: "规则库", icon: "▤" },
  { key: "settings", label: "设置", icon: "⚙" },
];

type Props = {
  active: NavKey;
  onChange: (key: NavKey) => void;
};

export function Sidebar({ active, onChange }: Props) {
  return (
    <aside className="flex h-full w-[220px] shrink-0 flex-col rounded-[28px] bg-white/90 p-5 shadow-soft">
      <div className="mb-8 flex items-center gap-3 px-1">
        <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-axiom-accent text-lg font-bold text-white">
          A
        </div>
        <div>
          <div className="text-lg font-bold tracking-tight text-axiom-text">
            axiom
          </div>
          <div className="text-[11px] text-axiom-muted">智联鉴控</div>
        </div>
      </div>

      <nav className="flex flex-1 flex-col gap-1">
        {NAV.map((item) => {
          const isActive = item.key === active;
          return (
            <button
              key={item.key}
              type="button"
              onClick={() => onChange(item.key)}
              className={`flex items-center gap-3 rounded-2xl px-3 py-2.5 text-left text-sm transition ${
                isActive
                  ? "bg-axiom-soft font-semibold text-axiom-accent"
                  : "text-axiom-muted hover:bg-black/[0.03] hover:text-axiom-text"
              }`}
            >
              <span
                className={`w-5 text-center ${isActive ? "text-axiom-accent" : ""}`}
              >
                {item.icon}
              </span>
              {item.label}
            </button>
          );
        })}
      </nav>

      <div className="mt-4 rounded-3xl bg-gradient-to-br from-[#efe9ff] to-[#f7f4ff] p-4">
        <div className="mb-1 text-sm font-semibold text-axiom-text">
          帮助中心
        </div>
        <p className="mb-3 text-xs leading-relaxed text-axiom-muted">
          四级预警、六类主体监测与准入清单说明
        </p>
        <button
          type="button"
          className="w-full rounded-full bg-axiom-accent px-3 py-2 text-xs font-semibold text-white"
        >
          发送反馈
        </button>
      </div>
    </aside>
  );
}

export type { NavKey };

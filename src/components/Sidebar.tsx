type NavKey =
  | "overview"
  | "whistle"
  | "guardian"
  | "scout"
  | "partners"
  | "rules"
  | "agent"
  | "settings";

const NAV: { key: NavKey; label: string; icon: string }[] = [
  { key: "overview", label: "总览", icon: "◈" },
  { key: "whistle", label: "风险吹哨", icon: "⚑" },
  { key: "guardian", label: "经营守护", icon: "◎" },
  { key: "scout", label: "准入瞭望", icon: "◉" },
  { key: "partners", label: "机构名单", icon: "☰" },
  { key: "rules", label: "规则库", icon: "▤" },
  { key: "agent", label: "问鉴控", icon: "✦" },
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
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-2xl bg-axiom-accent text-lg font-bold text-white">
          A
        </div>
        <div className="min-w-0">
          <div className="truncate text-base font-bold tracking-tight text-axiom-text">
            智鉴风控官
          </div>
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
              className={`flex items-center gap-3 rounded-2xl px-3 py-3 text-left text-[15px] transition ${
                isActive
                  ? "bg-axiom-soft font-bold text-axiom-accent"
                  : "font-semibold text-axiom-muted hover:bg-black/[0.03] hover:text-axiom-text"
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
    </aside>
  );
}

export type { NavKey };

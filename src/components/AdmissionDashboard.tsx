import type { AdmissionCheckItem, AdmissionReview } from "../data/types";

type Props = {
  review: AdmissionReview;
  onExport: () => void;
};

function typeLabel(t: string) {
  const map: Record<string, string> = {
    loan: "助贷机构",
    guarantee: "融资担保",
    traffic: "流量引流",
    payment: "支付机构",
    data: "数据服务商",
    collection: "催收机构",
  };
  return map[t] || t;
}

function conclusionMeta(c: string) {
  if (c === "否决")
    return {
      tone: "from-rose-600 to-red-500",
      soft: "bg-red-50 text-red-800",
      ring: "#ef4444",
      stamp: "否决",
      score: 18,
      hint: "存在一票否决项，建议终止准入",
    };
  if (c.includes("条件"))
    return {
      tone: "from-orange-500 to-amber-500",
      soft: "bg-orange-50 text-orange-800",
      ring: "#f97316",
      stamp: "有条件",
      score: 52,
      hint: "重大违规待整改后可再议",
    };
  if (c.includes("待核验"))
    return {
      tone: "from-amber-500 to-yellow-500",
      soft: "bg-amber-50 text-amber-900",
      ring: "#f59e0b",
      stamp: "待核验",
      score: 40,
      hint: "证据不足，禁止按通过推进",
    };
  return {
    tone: "from-emerald-500 to-teal-500",
    soft: "bg-emerald-50 text-emerald-800",
    ring: "#10b981",
    stamp: "通过",
    score: 88,
    hint: "清单未触发重大否决项",
  };
}

function colorLabel(c: string) {
  if (c === "red") return "一票否决";
  if (c === "orange") return "重大违规";
  return "关注事项";
}

function sourceIcon(source: string) {
  if (source.includes("全网")) return "◎";
  if (source.includes("企查") || source.includes("天眼")) return "◈";
  if (source.includes("知识")) return "▣";
  if (source.includes("线索")) return "◇";
  if (source.includes("材料")) return "▤";
  return "○";
}

/** 结论场景插画 */
function ConclusionArt({ conclusion }: { conclusion: string }) {
  const meta = conclusionMeta(conclusion);
  return (
    <svg viewBox="0 0 320 180" className="h-full w-full" aria-hidden>
      <defs>
        <linearGradient id="admHero" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#7c5cfc" stopOpacity="0.9" />
          <stop offset="100%" stopColor={meta.ring} stopOpacity="0.85" />
        </linearGradient>
        <radialGradient id="admGlow" cx="50%" cy="40%" r="60%">
          <stop offset="0%" stopColor="#fff" stopOpacity="0.35" />
          <stop offset="100%" stopColor="#fff" stopOpacity="0" />
        </radialGradient>
      </defs>
      <rect width="320" height="180" rx="28" fill="url(#admHero)" />
      <circle cx="260" cy="36" r="48" fill="url(#admGlow)" />
      <circle cx="48" cy="150" r="36" fill="#fff" fillOpacity="0.08" />
      {/* 建筑剪影 */}
      <g fill="#fff" fillOpacity="0.22">
        <rect x="28" y="78" width="28" height="72" rx="4" />
        <rect x="64" y="58" width="34" height="92" rx="4" />
        <rect x="106" y="88" width="24" height="62" rx="4" />
        <rect x="138" y="68" width="40" height="82" rx="4" />
      </g>
      <g fill="#fff" fillOpacity="0.35">
        {[86, 96, 106, 116, 126, 136].map((y) => (
          <rect key={y} x="72" y={y} width="8" height="5" rx="1" />
        ))}
        {[78, 88, 98, 108, 118, 128, 138].map((y) => (
          <rect key={`b${y}`} x="148" y={y} width="8" height="5" rx="1" />
        ))}
      </g>
      {/* 放大镜 / 盾牌 */}
      <g transform="translate(210,48)">
        <circle cx="36" cy="36" r="28" fill="#fff" fillOpacity="0.2" />
        <path
          d="M36 12c-12 0-22 8-22 18v10c0 14 10 26 22 30 12-4 22-16 22-30V30c0-10-10-18-22-18z"
          fill="#fff"
          fillOpacity="0.92"
        />
        <path
          d="M28 36l6 6 12-14"
          fill="none"
          stroke={meta.ring}
          strokeWidth="4"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </g>
      <text x="28" y="44" fill="#fff" fontSize="13" fontWeight="700" opacity="0.9">
        准入瞭望 · 视觉意见书
      </text>
      <text x="28" y="168" fill="#fff" fontSize="11" opacity="0.75">
        {meta.hint}
      </text>
    </svg>
  );
}

/** 准入健康分半环 */
function AdmissionGauge({ score, label }: { score: number; label: string }) {
  const pct = Math.max(0, Math.min(100, score));
  const color =
    pct < 35 ? "#ef4444" : pct < 55 ? "#f59e0b" : pct < 75 ? "#f97316" : "#10b981";
  return (
    <div className="relative mx-auto h-[150px] w-[240px]">
      <svg viewBox="0 0 240 140" className="h-full w-full">
        <path
          d="M20 120 A100 100 0 0 1 220 120"
          fill="none"
          stroke="#ece7ff"
          strokeWidth="18"
          strokeLinecap="round"
        />
        <path
          d="M20 120 A100 100 0 0 1 220 120"
          fill="none"
          stroke={color}
          strokeWidth="18"
          strokeLinecap="round"
          strokeDasharray={`${(pct / 100) * 314} 314`}
        />
      </svg>
      <div className="absolute inset-x-0 bottom-1 text-center">
        <div className="text-3xl font-bold tracking-tight" style={{ color }}>
          {score}
        </div>
        <div className="text-xs text-axiom-muted">{label}</div>
      </div>
    </div>
  );
}

/** 红橙黄占比环形图 */
function RiskDonut({
  red,
  orange,
  yellow,
  clear,
}: {
  red: number;
  orange: number;
  yellow: number;
  clear: number;
}) {
  const total = Math.max(1, red + orange + yellow + clear);
  const segs = [
    { n: red, c: "#ef4444", label: "红" },
    { n: orange, c: "#f97316", label: "橙" },
    { n: yellow, c: "#f59e0b", label: "黄" },
    { n: clear, c: "#10b981", label: "未触发" },
  ];
  const r = 54;
  const c = 2 * Math.PI * r;
  let offset = 0;
  return (
    <div className="flex items-center gap-4">
      <svg viewBox="0 0 140 140" className="h-36 w-36 shrink-0">
        <circle cx="70" cy="70" r={r} fill="none" stroke="#f3f0f8" strokeWidth="16" />
        {segs.map((s) => {
          const len = (s.n / total) * c;
          const el = (
            <circle
              key={s.label}
              cx="70"
              cy="70"
              r={r}
              fill="none"
              stroke={s.c}
              strokeWidth="16"
              strokeDasharray={`${len} ${c - len}`}
              strokeDashoffset={-offset}
              transform="rotate(-90 70 70)"
              strokeLinecap="butt"
            />
          );
          offset += len;
          return el;
        })}
        <text
          x="70"
          y="66"
          textAnchor="middle"
          className="fill-axiom-text"
          fontSize="22"
          fontWeight="700"
        >
          {red + orange + yellow}
        </text>
        <text
          x="70"
          y="86"
          textAnchor="middle"
          className="fill-axiom-muted"
          fontSize="10"
        >
          触发项
        </text>
      </svg>
      <div className="space-y-2 text-xs">
        {segs.map((s) => (
          <div key={s.label} className="flex items-center gap-2">
            <span
              className="inline-block h-2.5 w-2.5 rounded-full"
              style={{ background: s.c }}
            />
            <span className="text-axiom-muted">{s.label}</span>
            <span className="font-bold">{s.n}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

/** 检查覆盖条形图 */
function CoverageBars({
  items,
}: {
  items: { color: string; total: number; hit: number; label: string }[];
}) {
  return (
    <div className="space-y-3">
      {items.map((row) => {
        const pct = row.total ? Math.round((row.hit / row.total) * 100) : 0;
        const bar =
          row.color === "red"
            ? "bg-red-500"
            : row.color === "orange"
              ? "bg-orange-500"
              : "bg-amber-400";
        return (
          <div key={row.color}>
            <div className="mb-1 flex justify-between text-[11px]">
              <span className="font-semibold text-axiom-text">{row.label}</span>
              <span className="text-axiom-muted">
                触发 {row.hit}/{row.total}（{pct}%）
              </span>
            </div>
            <div className="h-3 overflow-hidden rounded-full bg-black/[0.06]">
              <div
                className={`h-full rounded-full ${bar} transition-all`}
                style={{ width: `${Math.max(row.hit ? 8 : 0, pct)}%` }}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}

/** 信号灯 */
function TrafficLights({ red, orange, yellow }: { red: number; orange: number; yellow: number }) {
  const lamps = [
    { on: red > 0, color: "#ef4444", glow: "shadow-red-400/50", label: `红 ${red}` },
    { on: orange > 0, color: "#f97316", glow: "shadow-orange-400/50", label: `橙 ${orange}` },
    { on: yellow > 0, color: "#f59e0b", glow: "shadow-amber-400/40", label: `黄 ${yellow}` },
  ];
  return (
    <div className="flex items-end justify-center gap-4 py-2">
      <svg viewBox="0 0 72 160" className="h-40 w-[72px]" aria-hidden>
        <rect x="18" y="8" width="36" height="144" rx="18" fill="#1f2937" />
        <circle cx="36" cy="40" r="14" fill={red > 0 ? "#ef4444" : "#4b5563"} opacity={red > 0 ? 1 : 0.45} />
        <circle cx="36" cy="80" r="14" fill={orange > 0 ? "#f97316" : "#4b5563"} opacity={orange > 0 ? 1 : 0.45} />
        <circle cx="36" cy="120" r="14" fill={yellow > 0 ? "#f59e0b" : "#4b5563"} opacity={yellow > 0 ? 1 : 0.45} />
        {red > 0 && <circle cx="36" cy="40" r="18" fill="#ef4444" opacity="0.2" />}
      </svg>
      <div className="space-y-3 pb-2 text-xs font-semibold">
        {lamps.map((l) => (
          <div key={l.label} className="flex items-center gap-2">
            <span
              className={`h-3 w-3 rounded-full shadow-md ${l.on ? l.glow : "opacity-40"}`}
              style={{ background: l.on ? l.color : "#9ca3af" }}
            />
            {l.label}
          </div>
        ))}
      </div>
    </div>
  );
}

function ItemCard({ item }: { item: AdmissionCheckItem }) {
  const border =
    item.color === "red"
      ? "border-l-red-500 bg-red-50/50"
      : item.color === "orange"
        ? "border-l-orange-500 bg-orange-50/40"
        : "border-l-amber-400 bg-amber-50/40";
  return (
    <div className={`rounded-2xl border border-black/[0.04] border-l-4 px-3 py-2.5 ${border}`}>
      <div className="flex items-start gap-2">
        <RiskIcon color={item.color} />
        <div className="min-w-0 flex-1">
          <div className="font-semibold text-sm">
            {item.id} · {item.title}
          </div>
          <div className="mt-0.5 text-[11px] text-axiom-muted">{item.legalBasis}</div>
          {item.evidence && (
            <div className="mt-1 text-xs leading-relaxed text-axiom-text">{item.evidence}</div>
          )}
          {item.sourceRef && (
            <a
              className="mt-1 block truncate text-[11px] text-sky-700 underline"
              href={item.sourceRef}
              target="_blank"
              rel="noreferrer"
            >
              {item.sourceRef}
            </a>
          )}
        </div>
      </div>
    </div>
  );
}

function RiskIcon({ color }: { color: string }) {
  const fill =
    color === "red" ? "#ef4444" : color === "orange" ? "#f97316" : "#f59e0b";
  return (
    <svg width="28" height="28" viewBox="0 0 28 28" className="mt-0.5 shrink-0" aria-hidden>
      <circle cx="14" cy="14" r="12" fill={fill} fillOpacity="0.15" />
      <path
        d="M14 7l7 12H7l7-12z"
        fill={fill}
        fillOpacity="0.9"
      />
      <rect x="13" y="12" width="2" height="4" rx="1" fill="#fff" />
      <circle cx="14" cy="18" r="1.1" fill="#fff" />
    </svg>
  );
}

function EvidenceTile({ label, idx }: { label: string; idx: number }) {
  const gid = `adm-ev-${idx}`;
  return (
    <div className="flex flex-col items-center gap-2 rounded-2xl bg-white/70 px-3 py-4 shadow-sm ring-1 ring-black/[0.04]">
      <svg viewBox="0 0 64 64" className="h-14 w-14" aria-hidden>
        <defs>
          <linearGradient id={gid} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%" stopColor="#7c5cfc" />
            <stop offset="100%" stopColor="#38bdf8" />
          </linearGradient>
        </defs>
        <rect width="64" height="64" rx="16" fill={`url(#${gid})`} opacity="0.15" />
        <circle cx="32" cy="28" r="12" fill={`url(#${gid})`} opacity="0.85" />
        <text x="32" y="33" textAnchor="middle" fill="#fff" fontSize="14" fontWeight="700">
          {sourceIcon(label)}
        </text>
        <rect x="16" y="44" width="32" height="6" rx="3" fill={`url(#${gid})`} opacity="0.5" />
      </svg>
      <div className="text-center text-[11px] font-semibold text-axiom-text">{label}</div>
    </div>
  );
}

function computeScore(review: AdmissionReview, red: number, orange: number, yellow: number) {
  const base = conclusionMeta(review.conclusion).score;
  // 微调：触发越多分越低
  return Math.max(5, Math.min(98, base - red * 12 - orange * 6 - yellow * 2));
}

export function AdmissionDashboard({ review, onExport }: Props) {
  const redItems = review.items.filter((i) => i.color === "red");
  const orangeItems = review.items.filter((i) => i.color === "orange");
  const yellowItems = review.items.filter((i) => i.color === "yellow");
  const red = redItems.filter((i) => i.triggered).length;
  const orange = orangeItems.filter((i) => i.triggered).length;
  const yellow = yellowItems.filter((i) => i.triggered).length;
  const clear = review.items.filter((i) => !i.triggered).length;
  const meta = conclusionMeta(review.conclusion);
  const score = computeScore(review, red, orange, yellow);
  const sources = review.evidenceSources?.length
    ? review.evidenceSources
    : ["证据待补"];

  const flow = [
    { t: "取证", d: sources.join(" · ") },
    { t: "匹配", d: `红${red} / 橙${orange} / 黄${yellow}` },
    { t: "研判", d: review.conclusion },
    { t: "落库", d: review.createdAt || "—" },
  ];

  return (
    <section className="space-y-4">
      {/* 视觉头图 */}
      <div className="card overflow-hidden">
        <div className="grid lg:grid-cols-[1.2fr_1fr]">
          <div className="relative min-h-[180px] p-2">
            <ConclusionArt conclusion={review.conclusion} />
          </div>
          <div className="flex flex-col justify-center gap-3 p-6">
            <div className="text-xs font-semibold text-axiom-accent">准入分析报告</div>
            <h2 className="text-2xl font-bold tracking-tight leading-snug">
              {review.partnerName}
            </h2>
            <div className="flex flex-wrap gap-2 text-[11px] text-axiom-muted">
              <span className="rounded-full bg-black/[0.04] px-2.5 py-1">
                {typeLabel(review.partnerType)}
              </span>
              <span className="rounded-full bg-black/[0.04] px-2.5 py-1">
                场景 {review.scenario}
              </span>
              {review.uscc ? (
                <span className="rounded-full bg-black/[0.04] px-2.5 py-1">
                  USCC {review.uscc}
                </span>
              ) : null}
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <span
                className={`rounded-full bg-gradient-to-r ${meta.tone} px-4 py-1.5 text-sm font-bold text-white shadow-sm`}
              >
                {meta.stamp}
              </span>
              <button
                type="button"
                onClick={onExport}
                className="rounded-full bg-black/[0.04] px-3 py-1.5 text-xs font-semibold"
              >
                导出 Markdown
              </button>
            </div>
            <p className="text-xs leading-relaxed text-axiom-muted">{meta.hint}</p>
          </div>
        </div>
      </div>

      {/* 图表面板 */}
      <div className="grid gap-4 lg:grid-cols-3">
        <div className="card flex flex-col items-center p-5">
          <div className="mb-1 self-start text-sm font-semibold">准入健康分</div>
          <AdmissionGauge score={score} label="综合研判分（示意）" />
        </div>
        <div className="card p-5">
          <div className="mb-2 text-sm font-semibold">风险结构</div>
          <RiskDonut red={red} orange={orange} yellow={yellow} clear={clear} />
        </div>
        <div className="card p-5">
          <div className="mb-2 text-sm font-semibold">信号灯</div>
          <TrafficLights red={red} orange={orange} yellow={yellow} />
        </div>
      </div>

      {/* 覆盖率 + 流程 */}
      <div className="grid gap-4 lg:grid-cols-2">
        <div className="card p-5">
          <div className="mb-3 text-sm font-semibold">红橙黄触发覆盖</div>
          <CoverageBars
            items={[
              {
                color: "red",
                label: "红色一票否决",
                total: redItems.length,
                hit: red,
              },
              {
                color: "orange",
                label: "橙色重大违规",
                total: orangeItems.length,
                hit: orange,
              },
              {
                color: "yellow",
                label: "黄色关注事项",
                total: yellowItems.length,
                hit: yellow,
              },
            ]}
          />
        </div>
        <div className="card p-5">
          <div className="mb-3 text-sm font-semibold">分析链路</div>
          <div className="relative flex items-start justify-between gap-2">
            <div className="absolute left-6 right-6 top-5 h-0.5 bg-gradient-to-r from-axiom-accent/40 via-sky-400/40 to-emerald-400/40" />
            {flow.map((f, i) => (
              <div key={f.t} className="relative z-[1] flex flex-1 flex-col items-center text-center">
                <div
                  className={`flex h-10 w-10 items-center justify-center rounded-full text-xs font-bold text-white shadow ${
                    i === 0
                      ? "bg-axiom-accent"
                      : i === 1
                        ? "bg-sky-500"
                        : i === 2
                          ? "bg-amber-500"
                          : "bg-emerald-500"
                  }`}
                >
                  {i + 1}
                </div>
                <div className="mt-2 text-xs font-bold">{f.t}</div>
                <div className="mt-0.5 line-clamp-2 px-1 text-[10px] text-axiom-muted">{f.d}</div>
              </div>
            ))}
          </div>
          <div className="mt-5 grid grid-cols-3 gap-2 sm:grid-cols-4 md:grid-cols-5">
            {sources.map((s, idx) => (
              <EvidenceTile key={`${s}-${idx}`} label={s} idx={idx} />
            ))}
          </div>
        </div>
      </div>

      {/* KPI 彩条 */}
      <div className="grid gap-3 sm:grid-cols-3">
        {[
          {
            label: "红色一票否决",
            n: red,
            art: (
              <svg viewBox="0 0 80 48" className="h-12 w-20" aria-hidden>
                <rect width="80" height="48" rx="12" fill="#fef2f2" />
                <path d="M20 34 L40 12 L60 34 Z" fill="#ef4444" opacity="0.9" />
              </svg>
            ),
            tone: "bg-red-50 text-red-800",
          },
          {
            label: "橙色重大违规",
            n: orange,
            art: (
              <svg viewBox="0 0 80 48" className="h-12 w-20" aria-hidden>
                <rect width="80" height="48" rx="12" fill="#fff7ed" />
                <rect x="28" y="10" width="24" height="28" rx="4" fill="#f97316" opacity="0.9" />
              </svg>
            ),
            tone: "bg-orange-50 text-orange-800",
          },
          {
            label: "黄色关注",
            n: yellow,
            art: (
              <svg viewBox="0 0 80 48" className="h-12 w-20" aria-hidden>
                <rect width="80" height="48" rx="12" fill="#fffbeb" />
                <circle cx="40" cy="24" r="12" fill="#f59e0b" opacity="0.9" />
              </svg>
            ),
            tone: "bg-amber-50 text-amber-900",
          },
        ].map((k) => (
          <div
            key={k.label}
            className={`flex items-center justify-between rounded-3xl px-4 py-3 ${k.tone}`}
          >
            <div>
              <div className="text-[11px] font-semibold opacity-80">{k.label}</div>
              <div className="mt-1 text-3xl font-bold tracking-tight">{k.n}</div>
            </div>
            {k.art}
          </div>
        ))}
      </div>

      {/* 摘要 */}
      <div className="card p-5">
        <div className="mb-2 flex items-center gap-2 text-sm font-semibold">
          <span className="flex h-7 w-7 items-center justify-center rounded-xl bg-axiom-soft text-axiom-accent">
            ≡
          </span>
          结论摘要
        </div>
        <div className="whitespace-pre-wrap text-sm leading-relaxed text-axiom-text">
          {review.summary}
        </div>
      </div>

      {review.aiNotes && (
        <div className="card overflow-hidden">
          <div className="bg-gradient-to-r from-axiom-soft via-sky-50 to-emerald-50 px-5 py-3">
            <div className="flex items-center gap-2 text-sm font-semibold text-axiom-text">
              <svg width="22" height="22" viewBox="0 0 24 24" aria-hidden>
                <circle cx="12" cy="12" r="10" fill="#7c5cfc" fillOpacity="0.2" />
                <path
                  d="M8 12h8M12 8v8"
                  stroke="#7c5cfc"
                  strokeWidth="2"
                  strokeLinecap="round"
                />
              </svg>
              AI 分析报告
            </div>
          </div>
          <div className="whitespace-pre-wrap px-5 py-4 text-sm leading-relaxed text-axiom-text">
            {review.aiNotes}
          </div>
        </div>
      )}

      {/* 触发明细 */}
      {(["red", "orange", "yellow"] as const).map((color) => {
        const triggered = review.items.filter((i) => i.color === color && i.triggered);
        if (triggered.length === 0) return null;
        return (
          <div key={color} className="card p-5">
            <div className="mb-3 flex items-center gap-2">
              <span
                className={`rounded-full px-2.5 py-0.5 text-xs font-bold ${
                  color === "red"
                    ? "bg-red-100 text-red-700"
                    : color === "orange"
                      ? "bg-orange-100 text-orange-700"
                      : "bg-amber-100 text-amber-800"
                }`}
              >
                {colorLabel(color)}
              </span>
              <span className="text-xs text-axiom-muted">触发 {triggered.length} 项</span>
            </div>
            <div className="grid gap-2 md:grid-cols-2">
              {triggered.map((item) => (
                <ItemCard key={item.id} item={item} />
              ))}
            </div>
          </div>
        );
      })}

      {review.remediation.length > 0 && (
        <div className="card p-5">
          <div className="mb-3 flex items-center gap-2 text-sm font-semibold text-axiom-accent">
            <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden>
              <rect width="20" height="20" rx="6" fill="#ece7ff" />
              <path
                d="M5 10h10M10 5v10"
                stroke="#7c5cfc"
                strokeWidth="1.8"
                strokeLinecap="round"
              />
            </svg>
            补证 / 整改清单
          </div>
          <div className="grid gap-2 sm:grid-cols-2">
            {review.remediation.map((r, idx) => (
              <div
                key={r}
                className="flex gap-3 rounded-2xl bg-axiom-soft/50 px-3 py-2.5 text-xs leading-relaxed"
              >
                <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-axiom-accent text-[11px] font-bold text-white">
                  {idx + 1}
                </span>
                <span>{r}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      <details className="card p-5">
        <summary className="cursor-pointer text-sm font-semibold">
          完整意见书 Markdown / 全部检查项
        </summary>
        <pre className="mt-3 max-h-80 overflow-auto whitespace-pre-wrap text-xs">
          {review.markdown}
        </pre>
        <div className="mt-4 grid gap-1 text-xs text-axiom-muted sm:grid-cols-2">
          {review.items.map((i) => (
            <div key={i.id}>
              {i.triggered ? "●" : "○"} [{i.color}] {i.id} {i.title}
            </div>
          ))}
        </div>
      </details>
    </section>
  );
}

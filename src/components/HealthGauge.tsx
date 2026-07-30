type Props = {
  score: number;
  label: string;
};

export function HealthGauge({ score, label }: Props) {
  const pct = Math.max(0, Math.min(100, score));
  const angle = -90 + (pct / 100) * 180;
  return (
    <div className="card flex h-full flex-col items-center justify-center p-5">
      <div className="mb-2 self-start text-base font-semibold">风险健康分</div>
      <div className="relative mt-2 h-[150px] w-[240px]">
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
            stroke="#7c5cfc"
            strokeWidth="18"
            strokeLinecap="round"
            strokeDasharray={`${(pct / 100) * 314} 314`}
          />
          <line
            x1="120"
            y1="120"
            x2={120 + 72 * Math.cos((angle * Math.PI) / 180)}
            y2={120 + 72 * Math.sin((angle * Math.PI) / 180)}
            stroke="#1f2937"
            strokeWidth="3"
            strokeLinecap="round"
          />
          <circle cx="120" cy="120" r="6" fill="#1f2937" />
        </svg>
        <div className="absolute inset-x-0 bottom-2 text-center">
          <div className="text-3xl font-bold tracking-tight">{score.toFixed(1)}</div>
          <div className="text-xs text-axiom-muted">{label}</div>
        </div>
      </div>
    </div>
  );
}

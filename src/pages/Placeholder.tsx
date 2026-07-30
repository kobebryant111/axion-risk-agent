export function Placeholder({ title, desc }: { title: string; desc: string }) {
  return (
    <div className="card flex flex-1 flex-col items-start justify-center p-10">
      <div className="text-2xl font-bold">{title}</div>
      <p className="mt-2 max-w-xl text-sm leading-relaxed text-axiom-muted">
        {desc}
      </p>
      <div className="mt-6 rounded-full bg-axiom-soft px-4 py-2 text-xs font-semibold text-axiom-accent">
        MVP 下一迭代实现
      </div>
    </div>
  );
}

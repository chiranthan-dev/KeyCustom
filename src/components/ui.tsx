import type { ReactNode } from "react";

export function Card({ title, children, aside }: { title: string; children: ReactNode; aside?: ReactNode }) {
  return (
    <section className="rounded-xl border border-zinc-800 bg-zinc-900/60 p-5" aria-label={title}>
      <header className="mb-4 flex items-center justify-between gap-3">
        <h2 className="text-sm font-semibold tracking-wide text-zinc-300 uppercase">{title}</h2>
        {aside}
      </header>
      {children}
    </section>
  );
}

export function Toggle({ checked, onChange, label, disabled }: { checked: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative h-6 w-11 shrink-0 rounded-full transition-colors disabled:opacity-40 ${checked ? "bg-sky-500" : "bg-zinc-700"}`}
    >
      <span className={`absolute top-0.5 left-0.5 h-5 w-5 rounded-full bg-white transition-transform ${checked ? "translate-x-5" : ""}`} />
    </button>
  );
}

export function Button({ children, variant = "secondary", ...props }: React.ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "secondary" | "ghost" }) {
  const styles = {
    primary: "bg-sky-500 text-zinc-950 hover:bg-sky-400",
    secondary: "bg-zinc-800 text-zinc-100 hover:bg-zinc-700",
    ghost: "text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100",
  }[variant];
  return (
    <button type="button" {...props} className={`rounded-lg px-3 py-1.5 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${styles} ${props.className ?? ""}`}>
      {children}
    </button>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className="rounded-md border border-zinc-700 border-b-2 bg-zinc-800 px-1.5 py-0.5 font-mono text-xs text-zinc-200">{children}</kbd>;
}

export function Alert({ tone, children }: { tone: "error" | "warn" | "info"; children: ReactNode }) {
  const styles = {
    error: "border-red-500/40 bg-red-500/10 text-red-200",
    warn: "border-amber-500/40 bg-amber-500/10 text-amber-100",
    info: "border-zinc-700 bg-zinc-800/50 text-zinc-300",
  }[tone];
  return (
    <div role={tone === "error" ? "alert" : "status"} className={`rounded-lg border px-3 py-2 text-sm whitespace-pre-wrap ${styles}`}>
      {children}
    </div>
  );
}

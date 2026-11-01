import { useEffect, useId, useRef, type ButtonHTMLAttributes, type InputHTMLAttributes, type ReactNode, type SelectHTMLAttributes } from "react";

export function Card({ title, children, aside, className = "" }: { title?: string; children: ReactNode; aside?: ReactNode; className?: string }) {
  return (
    <section className={`rounded-xl border border-neutral-800 bg-neutral-900/60 p-5 ${className}`} aria-label={title}>
      {(title || aside) && (
        <header className="mb-4 flex items-center justify-between gap-3">
          {title && <h2 className="font-mono text-[11px] font-medium tracking-[0.12em] text-neutral-400 uppercase">{title}</h2>}
          {aside}
        </header>
      )}
      {children}
    </section>
  );
}

export function PageHeader({ title, description, children }: { title: string; description?: string; children?: ReactNode }) {
  return (
    <header className="mb-5 flex flex-wrap items-end justify-between gap-3">
      <div>
        <p className="font-mono text-[11px] tracking-[0.12em] text-neutral-500 uppercase">KeyForge /</p>
        <h1 className="mt-1 text-2xl font-semibold tracking-tight text-neutral-50">{title}</h1>
        {description && <p className="mt-1 max-w-2xl text-sm text-neutral-400">{description}</p>}
      </div>
      {children && <div className="flex flex-wrap items-center gap-2">{children}</div>}
    </header>
  );
}

export function Toggle({ checked, onChange, label, disabled }: { checked: boolean; onChange: (v: boolean) => void; label: string; disabled?: boolean }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative h-6 w-11 shrink-0 rounded-full transition-colors disabled:opacity-40 ${checked ? "bg-lime-500" : "bg-neutral-700"}`}
    >
      <span className={`absolute top-0.5 left-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform ${checked ? "translate-x-5" : ""}`} />
    </button>
  );
}

type Variant = "primary" | "secondary" | "ghost" | "danger";

export function Button({ children, variant = "secondary", className = "", ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant }) {
  const styles: Record<Variant, string> = {
    primary: "bg-lime-400 text-neutral-950 hover:bg-lime-300",
    secondary: "bg-neutral-800 text-neutral-100 hover:bg-neutral-700",
    ghost: "text-neutral-400 hover:bg-neutral-800 hover:text-neutral-100",
    danger: "bg-red-500/15 text-red-300 hover:bg-red-500/25",
  };
  return (
    <button
      type="button"
      {...props}
      className={`inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 font-mono text-xs font-medium tracking-wider uppercase transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${styles[variant]} ${className}`}
    >
      {children}
    </button>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className="rounded-md border border-b-2 border-neutral-700 bg-neutral-800 px-1.5 py-0.5 font-mono text-xs whitespace-nowrap text-neutral-200">{children}</kbd>;
}

export function Alert({ tone, children, onClose }: { tone: "error" | "warn" | "info" | "ok"; children: ReactNode; onClose?: () => void }) {
  const styles = {
    error: "border-red-500/40 bg-red-500/10 text-red-200",
    warn: "border-amber-500/40 bg-amber-500/10 text-amber-100",
    info: "border-neutral-700 bg-neutral-800/60 text-neutral-300",
    ok: "border-lime-500/40 bg-lime-500/10 text-lime-100",
  }[tone];
  return (
    <div role={tone === "error" ? "alert" : "status"} className={`flex items-start gap-3 rounded-lg border px-3 py-2 text-sm whitespace-pre-wrap ${styles}`}>
      <div className="flex-1">{children}</div>
      {onClose && (
        <button type="button" onClick={onClose} aria-label="Dismiss" className="opacity-60 hover:opacity-100">
          ✕
        </button>
      )}
    </div>
  );
}

export function Field({ label, hint, children, className = "" }: { label: string; hint?: ReactNode; children: (id: string) => ReactNode; className?: string }) {
  const id = useId();
  return (
    <div className={`grid gap-1 text-sm ${className}`}>
      <label htmlFor={id} className="text-neutral-400">
        {label}
      </label>
      {children(id)}
      {hint && <p className="text-xs text-neutral-500">{hint}</p>}
    </div>
  );
}

const control = "rounded-lg border border-neutral-700 bg-neutral-800 px-2.5 py-1.5 text-sm text-neutral-100 placeholder:text-neutral-500 disabled:opacity-50";

export function Input(props: InputHTMLAttributes<HTMLInputElement>) {
  return <input {...props} className={`${control} ${props.className ?? ""}`} />;
}

export function TextArea(props: React.TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea {...props} className={`${control} min-h-20 ${props.className ?? ""}`} />;
}

export function Select(props: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select {...props} className={`${control} ${props.className ?? ""}`} />;
}

export function Checkbox({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: ReactNode }) {
  return (
    <label className="flex cursor-pointer items-center gap-2 text-sm text-neutral-300">
      <input type="checkbox" className="h-4 w-4 accent-lime-500" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );
}

export function Dialog({ title, open, onClose, children, footer, wide }: { title: string; open: boolean; onClose: () => void; children: ReactNode; footer?: ReactNode; wide?: boolean }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) d.showModal();
    if (!open && d.open) d.close();
  }, [open]);
  return (
    <dialog
      ref={ref}
      onClose={onClose}
      aria-label={title}
      className={`m-auto w-[calc(100%-2rem)] ${wide ? "max-w-3xl" : "max-w-lg"} rounded-xl border border-neutral-700 bg-neutral-900 p-0 text-neutral-100 shadow-2xl backdrop:bg-black/60`}
    >
      {open && (
        <div className="flex max-h-[85vh] flex-col">
          <header className="flex items-center justify-between border-b border-neutral-800 px-5 py-3">
            <h2 className="font-semibold">{title}</h2>
            <button type="button" onClick={onClose} aria-label="Close" className="text-neutral-400 hover:text-neutral-100">
              ✕
            </button>
          </header>
          <div className="overflow-y-auto px-5 py-4">{children}</div>
          {footer && <footer className="flex justify-end gap-2 border-t border-neutral-800 px-5 py-3">{footer}</footer>}
        </div>
      )}
    </dialog>
  );
}

export function Empty({ children }: { children: ReactNode }) {
  return <p className="rounded-lg border border-dashed border-neutral-800 px-4 py-6 text-center text-sm text-neutral-500">{children}</p>;
}

export function Badge({ children, tone = "neutral" }: { children: ReactNode; tone?: "neutral" | "accent" | "ok" | "amber" | "red" }) {
  const styles = {
    neutral: "bg-neutral-800 text-neutral-300",
    accent: "bg-lime-500/15 text-lime-300",
    ok: "bg-lime-500/15 text-lime-300",
    amber: "bg-amber-500/15 text-amber-200",
    red: "bg-red-500/15 text-red-300",
  }[tone];
  return <span className={`inline-flex items-center rounded-md px-1.5 py-0.5 font-mono text-[11px] tracking-wide ${styles}`}>{children}</span>;
}

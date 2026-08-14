import { createContext, forwardRef, useEffect, useRef, type ButtonHTMLAttributes, type CSSProperties, type HTMLAttributes, type InputHTMLAttributes, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Checkbox, Switch } from "../ui/Controls";
import AppSelect from "../ui/AppSelect";

const PopupHostContext = createContext(false);

export function MiuixTheme({ children }: { children: ReactNode }) {
  return <div className="mx-theme">{children}</div>;
}

export function Scaffold({ topBar, navigationBar, children, className = "" }: { topBar?: ReactNode; navigationBar?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <PopupHostContext.Provider value>
      <div className={`mx-scaffold ${className}`}>
        {topBar}<div className="mx-scaffold-body">{children}</div>{navigationBar}
        <div id="miuix-popup-host" className="mx-popup-host" aria-live="polite" />
      </div>
    </PopupHostContext.Provider>
  );
}

export function Surface({ className = "", ...props }: HTMLAttributes<HTMLElement>) {
  return <section className={`mx-surface ${className}`} {...props} />;
}

export function Card({ className = "", ...props }: HTMLAttributes<HTMLElement>) {
  return <section className={`mx-card ${className}`} {...props} />;
}

export const Button = forwardRef<HTMLButtonElement, ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "secondary" | "text" | "danger" }>(function Button({ variant = "secondary", className = "", ...props }, ref) {
  return <button ref={ref} className={`mx-button mx-button-${variant} ${className}`} {...props} />;
});

export function TextField({ className = "", ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={`mx-text-field ${className}`} {...props} />;
}

export function Dialog({ open, title, children, confirmLabel = "确定", cancelLabel = "取消", danger = false, busy = false, onConfirm, onDismiss }: { open: boolean; title: string; children: ReactNode; confirmLabel?: string; cancelLabel?: string; danger?: boolean; busy?: boolean; onConfirm: () => void; onDismiss: () => void }) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!open) return;
    cancelRef.current?.focus();
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) onDismiss();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, busy, onDismiss]);
  if (!open) return null;
  return createPortal(
    <div className="mx-dialog-overlay" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && !busy && onDismiss()}>
      <section className="mx-dialog" role="alertdialog" aria-modal="true" aria-labelledby="mx-dialog-title">
        <div className={`mx-dialog-icon ${danger ? "is-danger" : ""}`} aria-hidden="true">{danger ? "!" : "i"}</div>
        <h2 id="mx-dialog-title">{title}</h2>
        <div className="mx-dialog-content">{children}</div>
        <div className="mx-dialog-actions">
          <Button ref={cancelRef} type="button" disabled={busy} onClick={onDismiss}>{cancelLabel}</Button>
          <Button type="button" variant={danger ? "danger" : "primary"} disabled={busy} onClick={onConfirm}>{busy ? "处理中…" : confirmLabel}</Button>
        </div>
      </section>
    </div>,
    document.body,
  );
}

export function NavigationBar({ items, selected, onSelect }: { items: Array<{ id: string; label: string; icon: ReactNode }>; selected: string; onSelect: (id: string) => void }) {
  return (
    <nav className="mx-navigation-bar" aria-label="主导航">
      {items.map((item) => <button key={item.id} aria-current={selected === item.id ? "page" : undefined} onClick={() => onSelect(item.id)}><span className="mx-navigation-icon">{item.icon}</span><span>{item.label}</span></button>)}
    </nav>
  );
}

export function PreferenceGroup({ title, description, children, className = "" }: { title: string; description?: string; children: ReactNode; className?: string }) {
  return <section className={`mx-preference-section ${className}`}><header><h2>{title}</h2>{description && <p>{description}</p>}</header><div className="mx-preference-group">{children}</div></section>;
}

export function Preference({ title, summary, icon, trailing, onClick, disabled = false, children }: { title: string; summary?: ReactNode; icon?: ReactNode; trailing?: ReactNode; onClick?: () => void; disabled?: boolean; children?: ReactNode }) {
  const content = <><div className="mx-preference-leading">{icon}</div><div className="mx-preference-copy"><div className="mx-preference-title">{title}</div>{summary && <div className="mx-preference-summary">{summary}</div>}{children}</div>{trailing && <div className="mx-preference-trailing">{trailing}</div>}</>;
  return onClick ? <button type="button" className="mx-preference" disabled={disabled} onClick={onClick}>{content}</button> : <div className={`mx-preference ${disabled ? "is-disabled" : ""}`}>{content}</div>;
}

export function SwitchPreference({ title, summary, checked, onChange, disabled }: { title: string; summary?: string; checked: boolean; onChange: (value: boolean) => void; disabled?: boolean }) {
  return <Preference title={title} summary={summary} disabled={disabled} onClick={() => !disabled && onChange(!checked)} trailing={<span onClick={(e) => e.stopPropagation()}><Switch label={title} checked={checked} disabled={disabled} onChange={onChange} /></span>} />;
}

export function CheckboxPreference({ title, summary, checked, onChange, disabled }: { title: string; summary?: string; checked: boolean; onChange: (value: boolean) => void; disabled?: boolean }) {
  return <Preference title={title} summary={summary} disabled={disabled} onClick={() => !disabled && onChange(!checked)} trailing={<span onClick={(e) => e.stopPropagation()}><Checkbox label="" checked={checked} disabled={disabled} onChange={onChange} /></span>} />;
}

export function SliderPreference({ title, summary, value, min, max, step = 1, valueLabel, onChange }: { title: string; summary?: string; value: number; min: number; max: number; step?: number; valueLabel?: string; onChange: (value: number) => void }) {
  const progress = `${((value - min) / Math.max(1, max - min)) * 100}%`;
  return <Preference title={title} summary={summary} trailing={<span className="mx-preference-value">{valueLabel ?? value}</span>}><input className="mx-slider" style={{ "--mx-slider-progress": progress } as CSSProperties} type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} /></Preference>;
}

export function SpinnerPreference({ title, summary, value, options, onChange }: { title: string; summary?: string; value: string; options: Array<{ value: string; label: string }>; onChange: (value: string) => void }) {
  return <Preference title={title} summary={summary} trailing={<AppSelect className="mx-spinner" value={value} options={options} onChange={onChange} aria-label={title} />} />;
}

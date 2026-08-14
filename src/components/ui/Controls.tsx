import { useId, useState, type InputHTMLAttributes, type ReactNode } from "react";

export function PasswordInput(props: InputHTMLAttributes<HTMLInputElement>) {
  const [visible, setVisible] = useState(false);
  const label = visible ? "隐藏 API Key" : "显示 API Key";
  return (
    <div className="password-field mt-1">
      <input {...props} type={visible ? "text" : "password"} className={`input password-field-input ${props.className ?? ""}`} />
      <button type="button" className="password-field-toggle" aria-label={label} title={label} onClick={() => setVisible((value) => !value)}>
        {visible ? (
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M3 3l14 14M8.7 5.2A8.2 8.2 0 0 1 10 5c4.4 0 7.5 5 7.5 5a13 13 0 0 1-2.3 2.7M11.8 14.8A8.5 8.5 0 0 1 10 15c-4.4 0-7.5-5-7.5-5a13.8 13.8 0 0 1 2.6-2.9M8.3 8.3a2.4 2.4 0 0 0 3.4 3.4" /></svg>
        ) : (
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M2.5 10S5.6 5 10 5s7.5 5 7.5 5-3.1 5-7.5 5-7.5-5-7.5-5Z" /><circle cx="10" cy="10" r="2.4" /></svg>
        )}
      </button>
    </div>
  );
}

export function Switch({ checked, onChange, label, disabled = false }: { checked: boolean; onChange: (checked: boolean) => void; label: string; disabled?: boolean }) {
  const id = useId();
  return (
    <label className={`miuix-switch ${disabled ? "is-disabled" : ""}`} htmlFor={id}>
      <input id={id} type="checkbox" role="switch" checked={checked} disabled={disabled} onChange={(event) => onChange(event.target.checked)} />
      <span className="miuix-switch-track" aria-hidden="true"><span /></span>
      <span className="sr-only">{label}</span>
    </label>
  );
}

export function Checkbox({ checked, onChange, label, disabled = false }: { checked: boolean; onChange: (checked: boolean) => void; label: ReactNode; disabled?: boolean }) {
  return (
    <label className={`miuix-checkbox ${disabled ? "is-disabled" : ""}`}>
      <input type="checkbox" checked={checked} disabled={disabled} onChange={(event) => onChange(event.target.checked)} />
      <span className="miuix-checkbox-box" aria-hidden="true"><svg viewBox="0 0 12 12"><path d="m2.2 6.1 2.3 2.3 5.3-5.2" /></svg></span>
      <span>{label}</span>
    </label>
  );
}

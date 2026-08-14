import { createPortal } from "react-dom";
import { useEffect, useId, useLayoutEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";

export interface AppSelectOption { value: string; label: string; }
interface Props {
  value: string;
  options: AppSelectOption[];
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
  maxHeight?: number | string;
  "aria-label"?: string;
}

/** Shared glass combobox used everywhere a native select would otherwise appear. */
export default function AppSelect({ value, options, onChange, placeholder, disabled, className = "", maxHeight, "aria-label": ariaLabel }: Props) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const listId = useId();
  const [menuPosition, setMenuPosition] = useState<CSSProperties>();
  const selected = options.find((option) => option.value === value);
  const choose = (next: string) => { onChange(next); setOpen(false); triggerRef.current?.focus(); };
  const move = (delta: number) => {
    if (!options.length) return;
    const index = Math.max(0, options.findIndex((option) => option.value === value));
    onChange(options[(index + delta + options.length) % options.length].value);
  };
  useEffect(() => {
    const close = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!rootRef.current?.contains(target) && !menuRef.current?.contains(target)) setOpen(false);
    };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, []);
  useLayoutEffect(() => {
    if (!open || !triggerRef.current) return;
    const updatePosition = () => {
      const rect = triggerRef.current?.getBoundingClientRect();
      if (!rect) return;
      setMenuPosition({ position: "fixed", zIndex: 1000, top: rect.bottom + 6, left: rect.left, width: rect.width });
    };
    updatePosition();
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [open]);
  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); if (!open) setOpen(true); move(event.key === "ArrowDown" ? 1 : -1); }
    else if (event.key === "Enter" || event.key === " ") { event.preventDefault(); setOpen((current) => !current); }
    else if (event.key === "Home") { event.preventDefault(); options[0] && onChange(options[0].value); }
    else if (event.key === "End") { event.preventDefault(); const last = options[options.length - 1]; if (last) onChange(last.value); }
    else if (event.key === "Escape") { event.preventDefault(); setOpen(false); }
  };
  const menu = open && createPortal(
    <div ref={menuRef} id={listId} className="app-select-menu" role="listbox" aria-label={ariaLabel} style={{ ...menuPosition, ...(maxHeight ? { "--app-select-max-height": typeof maxHeight === "number" ? `${maxHeight}px` : maxHeight } : {}) } as CSSProperties}>
      {options.map((option) => <button key={option.value} type="button" role="option" aria-selected={option.value === value} className="app-select-option" onClick={() => choose(option.value)}>{option.label}</button>)}
    </div>,
    document.body,
  );
  return <div ref={rootRef} className={`app-select ${open ? "is-open" : ""} ${className}`}>
    <button ref={triggerRef} type="button" className="app-select-trigger" disabled={disabled} aria-label={ariaLabel} aria-haspopup="listbox" aria-expanded={open} aria-controls={listId} onClick={() => setOpen((current) => !current)} onKeyDown={onKeyDown}>
      <span>{selected?.label ?? placeholder ?? "请选择"}</span><span className="app-select-chevron" aria-hidden="true">⌄</span>
    </button>
    {menu}
  </div>;
}

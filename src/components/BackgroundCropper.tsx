import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { NormalizedCrop } from "../utils/customBackground";

const VIEW_WIDTH = 260;
const VIEW_HEIGHT = Math.round(VIEW_WIDTH * 720 / 460);

type Size = { width: number; height: number };
type Offset = { x: number; y: number };

function clampOffset(offset: Offset, image: Size, zoom: number): Offset {
  if (image.width <= 0 || image.height <= 0) return { x: 0, y: 0 };
  const baseScale = Math.max(VIEW_WIDTH / image.width, VIEW_HEIGHT / image.height);
  const renderedWidth = image.width * baseScale * zoom;
  const renderedHeight = image.height * baseScale * zoom;
  const maxX = Math.max(0, (renderedWidth - VIEW_WIDTH) / 2);
  const maxY = Math.max(0, (renderedHeight - VIEW_HEIGHT) / 2);
  return {
    x: Math.max(-maxX, Math.min(maxX, offset.x)),
    y: Math.max(-maxY, Math.min(maxY, offset.y)),
  };
}

function normalizedCrop(image: Size, zoom: number, offset: Offset): NormalizedCrop {
  const baseScale = Math.max(VIEW_WIDTH / image.width, VIEW_HEIGHT / image.height);
  const renderScale = baseScale * zoom;
  const renderedWidth = image.width * renderScale;
  const renderedHeight = image.height * renderScale;
  return {
    x: ((renderedWidth - VIEW_WIDTH) / 2 - offset.x) / renderedWidth,
    y: ((renderedHeight - VIEW_HEIGHT) / 2 - offset.y) / renderedHeight,
    width: VIEW_WIDTH / renderedWidth,
    height: VIEW_HEIGHT / renderedHeight,
  };
}

export default function BackgroundCropper({
  file,
  busy,
  onCancel,
  onConfirm,
}: {
  file: File;
  busy: boolean;
  onCancel: () => void;
  onConfirm: (crop: NormalizedCrop) => void;
}) {
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [loadFailed, setLoadFailed] = useState(false);
  const [imageSize, setImageSize] = useState<Size>({ width: 0, height: 0 });
  const [zoom, setZoom] = useState(1);
  const [offset, setOffset] = useState<Offset>({ x: 0, y: 0 });
  const dragRef = useRef<{ pointerId: number; x: number; y: number; origin: Offset } | null>(null);

  // StrictMode 会执行一次模拟卸载。URL 必须在每次 effect 挂载时重新创建，
  // 否则模拟卸载释放后，第二次挂载会继续引用已经失效的 blob URL。
  useEffect(() => {
    const nextUrl = URL.createObjectURL(file);
    setImageUrl(nextUrl);
    setLoadFailed(false);
    setImageSize({ width: 0, height: 0 });
    setZoom(1);
    setOffset({ x: 0, y: 0 });
    return () => URL.revokeObjectURL(nextUrl);
  }, [file]);
  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) onCancel();
    };
    document.addEventListener("keydown", closeOnEscape);
    return () => document.removeEventListener("keydown", closeOnEscape);
  }, [busy, onCancel]);

  const baseScale = imageSize.width > 0
    ? Math.max(VIEW_WIDTH / imageSize.width, VIEW_HEIGHT / imageSize.height)
    : 1;
  const renderedWidth = imageSize.width * baseScale * zoom;
  const renderedHeight = imageSize.height * baseScale * zoom;

  const updateZoom = (value: number, anchor?: Offset) => {
    const nextZoom = Math.max(1, Math.min(3, value));
    setOffset((current) => {
      if (!anchor || zoom <= 0) return clampOffset(current, imageSize, nextZoom);
      const ratio = nextZoom / zoom;
      // 保持鼠标指向的图片位置不动，让滚轮缩放符合图片编辑器习惯。
      return clampOffset({
        x: anchor.x - (anchor.x - current.x) * ratio,
        y: anchor.y - (anchor.y - current.y) * ratio,
      }, imageSize, nextZoom);
    });
    setZoom(nextZoom);
  };

  return createPortal(
    <div className="background-crop-overlay" role="dialog" aria-modal="true" aria-labelledby="background-crop-title">
      <div className="background-crop-dialog">
        <div className="flex items-start justify-between gap-3">
          <div>
            <h3 id="background-crop-title" className="text-[14px] font-semibold text-white">选择背景区域</h3>
            <p className="mt-1 text-[10px] text-white/80">拖动调整位置，滚轮或滑杆缩放，确认后裁切。</p>
          </div>
          <button type="button" className="background-crop-close" aria-label="取消裁切" disabled={busy} onClick={onCancel}>×</button>
        </div>

        <div
          className="background-crop-viewport mt-3"
          style={{ width: VIEW_WIDTH, height: VIEW_HEIGHT }}
          onPointerDown={(event) => {
            if (busy || imageSize.width === 0) return;
            event.currentTarget.setPointerCapture(event.pointerId);
            dragRef.current = { pointerId: event.pointerId, x: event.clientX, y: event.clientY, origin: offset };
          }}
          onPointerMove={(event) => {
            const drag = dragRef.current;
            if (!drag || drag.pointerId !== event.pointerId) return;
            setOffset(clampOffset({
              x: drag.origin.x + event.clientX - drag.x,
              y: drag.origin.y + event.clientY - drag.y,
            }, imageSize, zoom));
          }}
          onPointerUp={(event) => {
            if (dragRef.current?.pointerId === event.pointerId) dragRef.current = null;
          }}
          onPointerCancel={() => { dragRef.current = null; }}
          onWheel={(event) => {
            if (busy || loadFailed || imageSize.width === 0) return;
            event.preventDefault();
            const rect = event.currentTarget.getBoundingClientRect();
            const anchor = {
              x: event.clientX - rect.left - rect.width / 2,
              y: event.clientY - rect.top - rect.height / 2,
            };
            const direction = event.deltaY < 0 ? 1 : -1;
            updateZoom(Number((zoom + direction * 0.1).toFixed(2)), anchor);
          }}
        >
          {imageUrl && !loadFailed && (
            <img
              src={imageUrl}
              alt="待裁切背景预览"
              draggable={false}
              onLoad={(event) => {
                const next = { width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight };
                setImageSize(next);
                setOffset({ x: 0, y: 0 });
              }}
              onError={() => {
                setLoadFailed(true);
                setImageSize({ width: 0, height: 0 });
              }}
              style={{
                width: renderedWidth || undefined,
                height: renderedHeight || undefined,
                transform: `translate(calc(-50% + ${offset.x}px), calc(-50% + ${offset.y}px))`,
              }}
            />
          )}
          {loadFailed && (
            <div className="background-crop-error" role="alert">
              图片预览加载失败，请取消后重新选择 PNG、JPEG 或 WebP 图片。
            </div>
          )}
          <div className="background-crop-grid" aria-hidden="true" />
          <div className="background-crop-hint" aria-hidden="true">拖动选择区域</div>
        </div>

        <label className="mt-3 block text-[11px] text-white/90">
          <span className="flex items-center justify-between"><span>缩放</span><span>{Math.round(zoom * 100)}%</span></span>
          <input
            type="range"
            min="1"
            max="3"
            step="0.01"
            value={zoom}
            disabled={busy || loadFailed || imageSize.width === 0}
            onChange={(event) => updateZoom(Number(event.target.value))}
            className="background-crop-range mt-2 w-full"
          />
        </label>

        <div className="mt-3 flex gap-2">
          <button type="button" className="btn btn-ghost flex-1 py-2 text-[11px]" disabled={busy} onClick={() => { setZoom(1); setOffset({ x: 0, y: 0 }); }}>重置</button>
          <button type="button" className="btn btn-ghost flex-1 py-2 text-[11px]" disabled={busy} onClick={onCancel}>取消</button>
          <button
            type="button"
            className="btn btn-primary flex-[1.35] py-2 text-[11px]"
            disabled={busy || loadFailed || imageSize.width === 0}
            onClick={() => onConfirm(normalizedCrop(imageSize, zoom, offset))}
          >
            {busy ? "处理中…" : "确认裁切"}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}

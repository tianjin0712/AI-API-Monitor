import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import { THEME_OVERRIDE_KEYS } from "../utils/layout";
import type {
  DeleteResult,
  Layout,
  ProviderConfig,
  RefreshSettings,
  UpdateInfo,
} from "../types";

type FormState = {
  id: number | null; // null = 新增
  name: string;
  providerType: string;
  apiUrl: string;
  apiKey: string;
};

const EMPTY_FORM: FormState = {
  id: null,
  name: "",
  providerType: "deepseek",
  apiUrl: "",
  apiKey: "",
};

const TYPE_PRESETS: Record<string, string> = {
  deepseek: "https://api.deepseek.com",
  openai: "https://api.openai.com/v1",
  codex: "https://chatgpt.com/backend-api/codex",
  openrouter: "https://openrouter.ai",
  siliconflow: "https://api.siliconflow.cn/v1",
  claude: "https://api.anthropic.com/v1",
  custom: "",
};

/** 使用 CLI 本地凭证的类型（无需输入 API Key） */
const NO_API_KEY_TYPES = new Set(["codex"]);

/** 各类型的附加说明（显示在表单内） */
const TYPE_HINTS: Record<string, string> = {
  codex: "无需 API Key：自动复用 Codex CLI 登录态（~/.codex/auth.json），请确保已运行 `codex login` 登录 ChatGPT。",
  claude: "需要组织（Organization）管理员 API Key（sk-ant-admin01-...）；个人账户不可用。Anthropic 为后付费账单，无余额查询，仅显示用量与费用。",
  custom: "自定义 OpenAI Admin API：仅适用于同时实现 /organization/usage/completions 与 /organization/costs 的服务，不等同于普通 Chat Completions 兼容接口。",
};

/** 设置页：Provider 增删改查 + 刷新策略 */
export default function Settings({
  layout,
  onLayoutChange,
}: {
  layout: Layout;
  onLayoutChange: (updater: (prev: Layout) => Layout) => void;
}) {
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  const [types, setTypes] = useState<string[]>([]);
  const [form, setForm] = useState<FormState>(EMPTY_FORM);
  const [saving, setSaving] = useState(false);
  const [refresh, setRefresh] = useState<RefreshSettings>({
    foregroundSecs: 10,
    backgroundSecs: 60,
  });
  const [error, setError] = useState<string | null>(null);
  const [alwaysOnTop, setAlwaysOnTop] = useState(false);
  const [migrationFailed, setMigrationFailed] = useState(0);
  const [typeMenuOpen, setTypeMenuOpen] = useState(false);
  const [updateStatus, setUpdateStatus] = useState<string | null>(null);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [updateBusy, setUpdateBusy] = useState<"checking" | "installing" | null>(null);
  const typeMenuRef = useRef<HTMLDivElement>(null);
  const typeTriggerRef = useRef<HTMLButtonElement | null>(null);
  const typeOptionRefs = useRef<Array<HTMLButtonElement | null>>([]);

  // P2：菜单打开时聚焦当前选中项（roving focus 起点）
  useEffect(() => {
    if (typeMenuOpen) {
      const idx = Math.max(0, types.indexOf(form.providerType));
      requestAnimationFrame(() => typeOptionRefs.current[idx]?.focus());
    }
  }, [typeMenuOpen, types, form.providerType]);

  const selectType = (type: string) => {
    setForm({
      ...form,
      providerType: type,
      apiUrl: TYPE_PRESETS[type] ?? form.apiUrl,
    });
    setTypeMenuOpen(false);
    typeTriggerRef.current?.focus(); // 选择后归还焦点
  };

  // P2：完整键盘语义（方向键循环 / Home / End / Enter / Space / Escape 归还焦点）
  const handleOptionKey =
    (idx: number) => (event: React.KeyboardEvent<HTMLButtonElement>) => {
      const last = types.length - 1;
      const move = (to: number) => {
        typeOptionRefs.current[to]?.focus();
        event.preventDefault();
      };
      switch (event.key) {
        case "ArrowDown":
          move(idx === last ? 0 : idx + 1);
          break;
        case "ArrowUp":
          move(idx === 0 ? last : idx - 1);
          break;
        case "Home":
          move(0);
          break;
        case "End":
          move(last);
          break;
        case "Enter":
        case " ":
          selectType(types[idx]);
          event.preventDefault();
          break;
        case "Escape":
          setTypeMenuOpen(false);
          typeTriggerRef.current?.focus();
          break;
      }
    };

  useEffect(() => {
    const closeTypeMenu = (event: PointerEvent) => {
      if (!typeMenuRef.current?.contains(event.target as Node)) {
        setTypeMenuOpen(false);
      }
    };
    document.addEventListener("pointerdown", closeTypeMenu);
    return () => document.removeEventListener("pointerdown", closeTypeMenu);
  }, []);

  const load = useCallback(async () => {
    try {
      setProviders(await api.listProviders());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void load();
    void api.supportedProviderTypes().then(setTypes).catch((e) => setError(String(e)));
    void api
      .getRefreshSettings()
      .then(setRefresh)
      .catch((e) => setError(String(e)));
    void api
      .getWindowState()
      .then((s) => setAlwaysOnTop(s.alwaysOnTop))
      .catch(() => {});
    void api
      .getMigrationStatus()
      .then((n) => setMigrationFailed(n ?? 0))
      .catch(() => {});
  }, [load]);

  const startEdit = (p: ProviderConfig) => {
    setForm({
      id: p.id,
      name: p.name,
      providerType: p.providerType,
      apiUrl: p.apiUrl,
      apiKey: "",
    });
  };

  const cancelEdit = () => setForm(EMPTY_FORM);

  const submit = async () => {
    setError(null);
    if (!form.name.trim() || !form.apiUrl.trim()) {
      setError("请填写名称与 API URL");
      return;
    }
    if (
      form.id === null &&
      !NO_API_KEY_TYPES.has(form.providerType) &&
      !form.apiKey.trim()
    ) {
      setError("新增账户必须填写 API Key");
      return;
    }
    setSaving(true);
    try {
      if (form.id === null) {
        await api.addProvider({
          name: form.name,
          providerType: form.providerType,
          apiUrl: form.apiUrl,
          apiKey: form.apiKey,
        });
      } else {
        await api.updateProvider({
          id: form.id,
          name: form.name,
          apiUrl: form.apiUrl,
          apiKey: form.apiKey.trim() ? form.apiKey : null,
        });
      }
      setForm(EMPTY_FORM);
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const remove = async (p: ProviderConfig) => {
    if (!window.confirm(`删除账户「${p.name}」？（API Key 将一并清除）`)) return;
    try {
      const result: DeleteResult = await api.deleteProvider(p.id);
      await load();
      if (!result.credentialCleaned) {
        setError(result.note ?? "账户已删除，但凭据清理状态未知");
      }
    } catch (e) {
      setError(String(e));
    }
  };

  const saveRefresh = async () => {
    try {
      await api.setRefreshSettings(refresh.foregroundSecs, refresh.backgroundSecs);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="animate-fade-in-up flex flex-col gap-4">
      {migrationFailed > 0 && (
        <div className="rounded-xl border border-warning/30 bg-warning/10 px-3 py-2 text-[12px] text-warning">
          {migrationFailed} 个账户的旧版凭据无法读取（可能已失效），请编辑账户重新录入 API
          Key。
        </div>
      )}
      {error && (
        <div className="rounded-xl border border-danger/30 bg-danger/10 px-3 py-2 text-[12px] text-danger">
          {error}
        </div>
      )}

      {/* 添加 / 编辑表单 */}
      <section className="glass p-4">
        <h2 className="text-[13px] font-semibold text-text-primary">
          {form.id === null ? "添加 API 账户" : `编辑账户：${form.name}`}
        </h2>
        <div className="mt-3 grid grid-cols-2 gap-2.5">
          <label className="text-[12px] text-text-secondary">
            名称
            <input
              className="input mt-1"
              value={form.name}
              placeholder="例如 DeepSeek 主账户"
              onChange={(e) => setForm({ ...form, name: e.target.value })}
            />
          </label>
          <div className="text-[12px] text-text-secondary" ref={typeMenuRef}>
            类型
            <div className="provider-picker mt-1">
              <button
                type="button"
                ref={typeTriggerRef}
                className="provider-picker-trigger"
                aria-haspopup="listbox"
                aria-expanded={typeMenuOpen}
                onClick={() => setTypeMenuOpen((open) => !open)}
                onKeyDown={(event) => {
                  // P2：方向键打开菜单并开始导航
                  if (!typeMenuOpen && (event.key === "ArrowDown" || event.key === "ArrowUp" || event.key === "Enter" || event.key === " ")) {
                    setTypeMenuOpen(true);
                    event.preventDefault();
                  }
                }}
              >
                <span>{form.providerType}</span>
                <span className={`provider-picker-chevron ${typeMenuOpen ? "is-open" : ""}`}>
                  ⌄
                </span>
              </button>
              {typeMenuOpen && (
                <div className="provider-picker-menu" role="listbox" aria-label="Provider 类型">
                  {types.map((type, idx) => (
                    <button
                      type="button"
                      role="option"
                      ref={(el) => {
                        typeOptionRefs.current[idx] = el;
                      }}
                      tabIndex={-1}
                      aria-selected={type === form.providerType}
                      className="provider-picker-option"
                      key={type}
                      onClick={() => selectType(type)}
                      onKeyDown={handleOptionKey(idx)}
                    >
                      {type}
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
          <label className="col-span-2 text-[12px] text-text-secondary">
            Base URL
            {NO_API_KEY_TYPES.has(form.providerType) ? (
              <span className="mt-1 block rounded-lg border border-accent/25 bg-accent/10 px-2.5 py-1.5 text-[11px] text-text-primary">
                {form.providerType === "codex"
                  ? "Codex 使用固定官方地址（不可修改，防止本机凭证泄露）。"
                  : "此类型使用固定地址"}
              </span>
            ) : null}
            <input
              className="input mt-1"
              value={form.apiUrl}
              disabled={NO_API_KEY_TYPES.has(form.providerType)}
              placeholder="https://api.deepseek.com"
              onChange={(e) => setForm({ ...form, apiUrl: e.target.value })}
            />
          </label>
          <label className="col-span-2 text-[12px] text-text-secondary">
            {NO_API_KEY_TYPES.has(form.providerType) ? (
              <span className="block rounded-lg border border-accent/25 bg-accent/10 px-2.5 py-1.5 text-[11px] text-text-primary">
                {form.providerType === "codex"
                  ? "Codex 无需 API Key：自动复用 Codex CLI 登录态（~/.codex/auth.json），请确保已运行 `codex login` 登录 ChatGPT。"
                  : "此类型无需 API Key"}
              </span>
            ) : (
              <>
                API Key
                <span className="ml-2 text-[10px] text-text-muted">
                  {form.id === null
                    ? "加密保存至系统凭据库，绝不落库"
                    : "留空表示不修改"}
                </span>
                <input
                  className="input mt-1"
                  type="password"
                  value={form.apiKey}
                  placeholder={form.id === null ? "sk-..." : "输入新 Key 以替换"}
                  onChange={(e) => setForm({ ...form, apiKey: e.target.value })}
                />
              </>
            )}
          </label>
          {TYPE_HINTS[form.providerType] && !NO_API_KEY_TYPES.has(form.providerType) && (
            <p className="col-span-2 rounded-lg border border-warning/25 bg-warning/10 px-2.5 py-1.5 text-[11px] text-warning">
              {TYPE_HINTS[form.providerType]}
            </p>
          )}
          {form.providerType === "openai" && (
            <p className="col-span-2 rounded-lg border border-warning/25 bg-warning/10 px-2.5 py-1.5 text-[11px] text-warning">
              提示：OpenAI 用量/费用接口需要组织（Organization）管理员权限的
              API Key，普通项目 Key 会返回 403。
            </p>
          )}
        </div>
        <div className="mt-3 flex gap-2">
          <button
            className="btn btn-primary flex-1"
            onClick={() => void submit()}
            disabled={saving}
          >
            {saving ? "保存中…" : form.id === null ? "添加" : "保存修改"}
          </button>
          {form.id !== null && (
            <button className="btn btn-ghost" onClick={cancelEdit}>
              取消
            </button>
          )}
        </div>
      </section>

      {/* 账户列表 */}
      <section className="glass p-4">
        <h2 className="text-[13px] font-semibold text-text-primary">
          已配置账户（{providers.length}）
        </h2>
        {providers.length === 0 ? (
          <p className="mt-3 text-[12px] text-text-muted">暂无账户</p>
        ) : (
          <ul className="mt-2 flex flex-col gap-2">
            {providers.map((p) => (
              <li
                key={p.id}
                className="flex items-center justify-between rounded-xl bg-white/[0.03] px-3 py-2.5"
              >
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-[13px] font-medium text-text-primary">
                      {p.name}
                    </span>
                    <span className="rounded bg-white/5 px-1.5 py-0.5 text-[10px] uppercase text-text-secondary">
                      {p.providerType}
                    </span>
                  </div>
                  <div className="mt-0.5 truncate text-[11px] text-text-muted">
                    {p.apiUrl}
                  </div>
                </div>
                <div className="ml-3 flex shrink-0 gap-1.5">
                  <button
                    className="btn btn-ghost px-2.5 py-1 text-[11px]"
                    onClick={() => startEdit(p)}
                  >
                    编辑
                  </button>
                  <button
                    className="btn btn-danger-ghost px-2.5 py-1 text-[11px]"
                    onClick={() => void remove(p)}
                  >
                    删除
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>

      {/* 刷新策略 */}
      <section className="glass p-4">
        <h2 className="text-[13px] font-semibold text-text-primary">刷新策略</h2>
        <div className="mt-3 grid grid-cols-2 gap-2.5">
          <label className="text-[12px] text-text-secondary">
            前台刷新（秒）
            <input
              className="input mt-1"
              type="number"
              min={10}
              max={3600}
              value={refresh.foregroundSecs}
              onChange={(e) =>
                setRefresh({ ...refresh, foregroundSecs: Number(e.target.value) })
              }
            />
          </label>
          <label className="text-[12px] text-text-secondary">
            后台刷新（秒）
            <input
              className="input mt-1"
              type="number"
              min={60}
              max={3600}
              value={refresh.backgroundSecs}
              onChange={(e) =>
                setRefresh({ ...refresh, backgroundSecs: Number(e.target.value) })
              }
            />
          </label>
        </div>
        <button className="btn btn-ghost mt-3" onClick={() => void saveRefresh()}>
          保存刷新策略
        </button>
      </section>

      {/* 窗口行为 */}
      <section className="glass p-4">
        <h2 className="text-[13px] font-semibold text-text-primary">窗口行为</h2>
        <label className="mt-3 flex cursor-pointer items-center justify-between">
          <span className="text-[13px] text-text-secondary">
            Always On Top
            <span className="ml-2 text-[11px] text-text-muted">窗口保持置顶</span>
          </span>
          <input
            type="checkbox"
            checked={alwaysOnTop}
            onChange={async (e) => {
              const v = e.target.checked;
              setAlwaysOnTop(v);
              try {
                await api.setAlwaysOnTop(v);
              } catch (err) {
                setError(String(err));
                setAlwaysOnTop(!v);
              }
            }}
            className="round-checkbox"
            aria-label="窗口保持置顶"
          />
        </label>
      </section>

      {/* V1.0 主题分享：自定义色值 + 导出/导入 */}
      <section className="glass p-4">
        <h2 className="text-[13px] font-semibold text-text-primary">
          主题分享
        </h2>
        <div className="mt-3 grid grid-cols-2 gap-2.5">
          {THEME_OVERRIDE_KEYS.map((key) => [key, THEME_LABELS[key]] as [string, string]).map(([key, label]) => (
            <label key={key} className="flex items-center justify-between text-[12px] text-text-secondary">
              <span>{label}</span>
              <input
                type="color"
                className="h-7 w-12 cursor-pointer rounded border border-border bg-transparent"
                value={hexOf(layout.themeOverrides?.[key], layout.theme, key)}
                onChange={(e) => {
                  const v = e.target.value;
                  onLayoutChange((prev) => ({
                    ...prev,
                    themeOverrides: { ...(prev.themeOverrides ?? {}), [key]: v },
                  }));
                }}
                aria-label={label}
              />
            </label>
          ))}
        </div>
        <div className="mt-3 flex flex-wrap gap-2">
          <button
            className="btn btn-ghost px-3 py-1 text-[12px]"
            onClick={() => {
              void navigator.clipboard
                .writeText(exportTheme(layout))
                .then(() => setError(null))
                .catch((e) => setError(`复制失败: ${String(e)}`));
            }}
          >
            导出主题（复制 JSON）
          </button>
          <button
            className="btn btn-ghost px-3 py-1 text-[12px]"
            onClick={async () => {
              try {
                const text = await navigator.clipboard.readText();
                const imported = importTheme(text);
                if (!imported) {
                  setError("导入失败：JSON 格式无效（需 { theme, overrides }）");
                  return;
                }
                onLayoutChange((prev) => ({
                  ...prev,
                  theme: imported.theme,
                  themeOverrides: imported.overrides,
                }));
                setError(null);
              } catch (e) {
                setError(`导入失败: ${String(e)}`);
              }
            }}
          >
            从剪贴板导入
          </button>
          <button
            className="btn btn-ghost px-3 py-1 text-[12px]"
            onClick={() =>
              onLayoutChange((prev) => ({ ...prev, themeOverrides: undefined }))
            }
          >
            重置自定义
          </button>
        </div>
        <p className="mt-2 text-[10px] text-text-muted">
          自定义色值随布局保存；导出后可在其他设备「从剪贴板导入」共享主题。
        </p>
      </section>

      {/* V1.0 关于与自动更新 */}
      <section className="glass p-4">
        <h2 className="text-[13px] font-semibold text-text-primary">关于与更新</h2>
        <p className="mt-2 text-[12px] text-text-secondary">AI API Monitor v0.1.0</p>
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <button
            className="btn btn-ghost px-3 py-1 text-[12px]"
            onClick={async () => {
              setUpdateBusy("checking");
              setUpdateStatus("检查中…");
              try {
                const info = await api.checkUpdate();
                setUpdateInfo(info);
                setUpdateStatus(
                  info.available
                    ? `发现新版本 v${info.version}`
                    : "已是最新版本",
                );
              } catch (e) {
                setUpdateStatus(String(e));
              } finally {
                setUpdateBusy(null);
              }
            }}
            disabled={updateBusy !== null}
          >
            {updateBusy === "checking" ? "检查中…" : "检查更新"}
          </button>
          {updateInfo?.available && (
            <button
              className="btn btn-primary px-3 py-1 text-[12px]"
              onClick={async () => {
                setUpdateBusy("installing");
                setUpdateStatus("下载并安装中…");
                try {
                  const msg = await api.installUpdate(updateInfo.version ?? "");
                  setUpdateStatus(msg);
                } catch (e) {
                  setUpdateStatus(String(e));
                } finally {
                  setUpdateBusy(null);
                }
              }}
              disabled={updateBusy !== null}
            >
              {updateBusy === "installing" ? "安装中…" : "下载并安装"}
            </button>
          )}
        </div>
        {updateStatus && (
          <p className="mt-2 text-[11px] text-text-secondary">{updateStatus}</p>
        )}
        <p className="mt-2 text-[10px] text-text-muted">
          自动更新需发布者在构建时配置更新签名与更新源（见 README 发布章节）；
          未配置时将提示"更新器未配置"。
        </p>
      </section>
    </div>
  );
}

// ---- V1.0 主题分享辅助 ----

const THEME_LABELS: Record<string, string> = {
  accent: "强调色",
  surface: "背景",
  card: "卡片",
  "text-primary": "主文字",
  success: "成功色",
  danger: "危险色",
};
const THEME_DEFAULTS: Record<Layout["theme"], Record<string, string>> = {
  dark: {
    accent: "#6c8cff", surface: "#0f1115", card: "#1a1d24",
    "text-primary": "#e6e9ef", success: "#34d399", danger: "#f87171",
  },
  light: {
    accent: "#4c6ef5", surface: "#f2f4f8", card: "#ffffff",
    "text-primary": "#1c2333", success: "#12b886", danger: "#fa5252",
  },
};

function hexOf(value: string | undefined, theme: Layout["theme"], key: string): string {
  return /^#[0-9a-fA-F]{6}$/.test(value ?? "") ? (value as string) : THEME_DEFAULTS[theme][key];
}

/** 导出主题为 JSON 文本 */
function exportTheme(layout: Layout): string {
  return JSON.stringify(
    {
      theme: layout.theme,
      overrides: layout.themeOverrides ?? {},
    },
    null,
    2,
  );
}

/** 解析导入的主题 JSON；无效返回 null */
function importTheme(text: string): { theme: Layout["theme"]; overrides: Record<string, string> } | null {
  try {
    const parsed = JSON.parse(text) as {
      theme?: string;
      overrides?: Record<string, string>;
    };
    if (parsed.theme !== "dark" && parsed.theme !== "light") return null;
    const overrides: Record<string, string> = {};
    for (const [k, v] of Object.entries(parsed.overrides ?? {})) {
      if (THEME_OVERRIDE_KEYS.includes(k as (typeof THEME_OVERRIDE_KEYS)[number]) && typeof v === "string" && /^#[0-9a-fA-F]{6}$/.test(v)) overrides[k] = v;
    }
    return { theme: parsed.theme, overrides };
  } catch {
    return null;
  }
}

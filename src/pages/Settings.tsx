import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import BackgroundCropper from "../components/BackgroundCropper";
import { PasswordInput } from "../components/ui/Controls";
import AppSelect from "../components/ui/AppSelect";
import { Button, Dialog, PreferenceGroup, SliderPreference, SpinnerPreference, SwitchPreference, TextField } from "../components/miuix/Miuix";
import { THEME_OVERRIDE_KEYS } from "../utils/layout";
import {
  CUSTOM_GIF_ID,
  CUSTOM_LUOTIANYI_BACKGROUND_ID,
  LUOTIANYI_BACKGROUNDS,
  LUOTIANYI_GIFS,
  luotianyiGifPath,
  readCustomLuotianyiBackground,
  saveCustomAvatarGif,
  saveCustomLuotianyiBackground,
} from "../utils/themeAssets";
import {
  BACKGROUND_EVENT,
  analyzeCustomBackground,
  clearCustomBackground,
  prepareCustomBackground,
  readCustomBackground,
  saveCustomBackground,
  saveCustomBackgroundPalette,
} from "../utils/customBackground";
import type {
  CustomApiConfig,
  CustomAuthType,
  CustomKeyValue,
  CustomTestResult,
  CustomUnit,
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
  custom: CustomFormState;
};

type CustomFormState = {
  method: "GET" | "POST";
  query: string;
  headers: string;
  authType: CustomAuthType;
  authHeaderName: string;
  authUsername: string;
  body: string;
  remainingPath: string;
  totalPath: string;
  usedPath: string;
  resetTimePath: string;
  unit: CustomUnit;
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

const DEFAULT_PROVIDER_TYPE = "deepseek";

const createEmptyCustom = (): CustomFormState => ({
  method: "GET",
  query: "",
  headers: "",
  authType: "bearer",
  authHeaderName: "",
  authUsername: "",
  body: "",
  remainingPath: "",
  totalPath: "",
  usedPath: "",
  resetTimePath: "",
  unit: "custom",
});

const createEmptyForm = (): FormState => ({
  id: null,
  name: "",
  providerType: DEFAULT_PROVIDER_TYPE,
  // 必须是实际值而非 placeholder，否则首次直接添加会被判定为 URL 为空。
  apiUrl: TYPE_PRESETS[DEFAULT_PROVIDER_TYPE],
  apiKey: "",
  custom: createEmptyCustom(),
});

/** 解析 key=value 逐行文本为结构化键值对。 */
function parseKeyValues(text: string): CustomKeyValue[] {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const index = line.indexOf("=");
      if (index <= 0) return { key: line, value: "" };
      return { key: line.slice(0, index).trim(), value: line.slice(index + 1).trim() };
    });
}

/** 由表单构造非敏感 Custom API 配置（敏感值走 apiKey 字段）。 */
function buildCustomConfig(form: FormState): CustomApiConfig {
  const custom = form.custom;
  return {
    url: form.apiUrl,
    method: custom.method,
    query: parseKeyValues(custom.query),
    headers: parseKeyValues(custom.headers),
    body: custom.body.trim() || null,
    auth: {
      type: custom.authType,
      headerName: custom.authHeaderName.trim() || null,
      username: custom.authUsername.trim() || null,
    },
    responseMapping: {
      remainingPath: custom.remainingPath.trim() || null,
      totalPath: custom.totalPath.trim() || null,
      usedPath: custom.usedPath.trim() || null,
      resetTimePath: custom.resetTimePath.trim() || null,
    },
    unit: custom.unit,
  };
}

/** 编辑时把已保存的 custom_config JSON 回填为表单状态。 */
function parseCustomConfig(json: string | null | undefined): CustomFormState {
  const empty = createEmptyCustom();
  if (!json) return empty;
  try {
    const cfg = JSON.parse(json) as CustomApiConfig;
    return {
      method: cfg.method === "POST" ? "POST" : "GET",
      query: (cfg.query ?? []).map((kv) => `${kv.key}=${kv.value}`).join("\n"),
      headers: (cfg.headers ?? []).map((kv) => `${kv.key}=${kv.value}`).join("\n"),
      authType: cfg.auth?.type ?? "bearer",
      authHeaderName: cfg.auth?.headerName ?? "",
      authUsername: cfg.auth?.username ?? "",
      body: cfg.body ?? "",
      remainingPath: cfg.responseMapping?.remainingPath ?? "",
      totalPath: cfg.responseMapping?.totalPath ?? "",
      usedPath: cfg.responseMapping?.usedPath ?? "",
      resetTimePath: cfg.responseMapping?.resetTimePath ?? "",
      unit: cfg.unit ?? "custom",
    };
  } catch {
    return empty;
  }
}

/** 使用 CLI 本地凭证的类型（无需输入 API Key） */
const NO_API_KEY_TYPES = new Set(["codex"]);

/** 各类型的附加说明（显示在表单内） */
const TYPE_HINTS: Record<string, string> = {
  codex: "无需 API Key：仅通过 `codex login status` 检测公开登录状态，不读取或保存任何 Token、Cookie 或认证文件。",
  claude: "需要组织（Organization）管理员 API Key（sk-ant-admin01-...）；个人账户不可用。Anthropic 为后付费账单，无余额查询，仅显示用量与费用。",
  custom: "通用自定义 API：可配置请求方法、URL、认证方式与 JSON 响应字段映射，接入任意返回余额/额度/用量/重置时间的接口。敏感值经系统凭据库加密保存。",
};

function NumberStepper({
  value,
  min,
  max,
  label,
  onChange,
}: {
  value: number;
  min: number;
  max: number;
  label: string;
  onChange: (value: number) => void;
}) {
  const step = (direction: 1 | -1) => {
    const current = Number.isFinite(value) ? value : min;
    onChange(Math.min(max, Math.max(min, current + direction)));
  };

  return (
    <div className="number-input-shell mt-1">
      <input
        className="input number-input-field"
        type="number"
        min={min}
        max={max}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
      />
      <div className="number-stepper-buttons" aria-hidden="false">
        <button
          type="button"
          className="number-stepper-button"
          aria-label={`增加${label}`}
          disabled={value >= max}
          onClick={() => step(1)}
        >
          <span className="number-stepper-arrow is-up" aria-hidden="true" />
        </button>
        <button
          type="button"
          className="number-stepper-button"
          aria-label={`减少${label}`}
          disabled={value <= min}
          onClick={() => step(-1)}
        >
          <span className="number-stepper-arrow is-down" aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}

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
  const [form, setForm] = useState<FormState>(createEmptyForm);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<CustomTestResult | null>(null);
  const [codexRuntimeStatus, setCodexRuntimeStatus] = useState<{ installed: boolean; loggedIn: boolean; runtimeSource: string | null } | null>(null);
  const [codexLoginPending, setCodexLoginPending] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<ProviderConfig | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [refresh, setRefresh] = useState<RefreshSettings>({
    foregroundSecs: 10,
    backgroundSecs: 60,
  });
  const [appBehavior, setAppBehavior] = useState<{ closeBehavior: "minimize_to_tray" | "quit"; autoStart: boolean }>({ closeBehavior: "minimize_to_tray", autoStart: false });
  const refreshCodexStatus = useCallback(() => {
    void api.getCodexRuntimeStatus().then((status) => {
      setCodexRuntimeStatus(status);
      if (status.loggedIn) setCodexLoginPending(false);
    }).catch(() => setCodexRuntimeStatus(null));
  }, []);

  useEffect(() => {
    if (form.providerType !== "codex") return;
    refreshCodexStatus();
    if (!codexLoginPending) return;
    const timer = window.setInterval(refreshCodexStatus, 2500);
    return () => window.clearInterval(timer);
  }, [form.providerType, codexLoginPending, refreshCodexStatus]);
  const [error, setError] = useState<string | null>(null);
  const [migrationFailed, setMigrationFailed] = useState(0);
  const [updateStatus, setUpdateStatus] = useState<string | null>(null);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [updateBusy, setUpdateBusy] = useState<"checking" | "installing" | null>(null);
  const [backgroundCrop, setBackgroundCrop] = useState<{ file: File; target: "custom" | "luotianyi" } | null>(null);
  const [backgroundCropBusy, setBackgroundCropBusy] = useState(false);
  const [paletteBusy, setPaletteBusy] = useState(false);
  const [customBackground, setCustomBackground] = useState(readCustomBackground);
  const [customLuotianyiBackground, setCustomLuotianyiBackground] = useState(readCustomLuotianyiBackground);
  const [savedThemes, setSavedThemes] = useState<SavedTheme[]>(readSavedThemes);
  const [selectedSavedThemeId, setSelectedSavedThemeId] = useState("");
  const [savedThemeName, setSavedThemeName] = useState("");
  const luotianyiBackgroundScrollerRef = useRef<HTMLDivElement>(null);
  const luotianyiBackgroundDragRef = useRef({ pointerId: -1, startX: 0, scrollLeft: 0, moved: false });
  const suppressLuotianyiBackgroundClickUntilRef = useRef(0);

  useEffect(() => {
    const update = () => setCustomBackground(readCustomBackground());
    window.addEventListener(BACKGROUND_EVENT, update);
    window.addEventListener("storage", update);
    return () => {
      window.removeEventListener(BACKGROUND_EVENT, update);
      window.removeEventListener("storage", update);
    };
  }, []);

  useEffect(() => {
    const scroller = luotianyiBackgroundScrollerRef.current;
    if (!scroller || layout.visualTheme !== "luotianyi") return;
    const handleWheel = (event: WheelEvent) => {
      if (scroller.scrollWidth <= scroller.clientWidth) return;
      event.preventDefault();
      event.stopPropagation();
      scroller.scrollLeft += Math.abs(event.deltaY) >= Math.abs(event.deltaX) ? event.deltaY : event.deltaX;
    };
    scroller.addEventListener("wheel", handleWheel, { passive: false });
    return () => scroller.removeEventListener("wheel", handleWheel);
  }, [layout.visualTheme, customLuotianyiBackground]);

  const selectType = (type: string) => {
    setForm((current) => ({
      ...current,
      providerType: type,
      // 每次选择都应用该类型的标准 Base URL；custom 则留空供用户填写。
      apiUrl: TYPE_PRESETS[type] ?? "",
    }));
    setError(null);
  };

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
      .getAppBehaviorSettings()
      .then(setAppBehavior)
      .catch((e) => setError(String(e)));
    void api
      .getMigrationStatus()
      .then((n) => setMigrationFailed(n ?? 0))
      .catch(() => {});
  }, [load]);

  const changeCloseBehavior = async (value: string) => {
    const closeBehavior = value as "minimize_to_tray" | "quit";
    try {
      await api.setCloseBehavior(closeBehavior);
      setAppBehavior((current) => ({ ...current, closeBehavior }));
    } catch (e) { setError(String(e)); }
  };

  const changeAutoStart = async (enabled: boolean) => {
    try {
      const actual = await api.setAutoStart(enabled);
      setAppBehavior((current) => ({ ...current, autoStart: actual }));
    } catch (e) { setError(String(e)); }
  };

  const startEdit = (p: ProviderConfig) => {
    setForm({
      id: p.id,
      name: p.name,
      providerType: p.providerType,
      apiUrl: p.apiUrl,
      apiKey: "",
      custom: parseCustomConfig(p.customConfig),
    });
    setTestResult(null);
  };

  const cancelEdit = () => {
    setForm(createEmptyForm());
    setTestResult(null);
  };

  const testCustom = async () => {
    setError(null);
    if (!form.apiUrl.trim()) {
      setError("请先填写 API 地址");
      return;
    }
    setTesting(true);
    setTestResult(null);
    try {
      const cfg = buildCustomConfig(form);
      const result = await api.testCustomProvider(
        JSON.stringify(cfg),
        form.apiKey.trim() || null,
      );
      setTestResult(result);
    } catch (e) {
      setTestResult({
        success: false,
        status: null,
        remaining: null,
        total: null,
        used: null,
        unit: form.custom.unit,
        resetTime: null,
        responsePreview: null,
        error: String(e),
      });
    } finally {
      setTesting(false);
    }
  };

  const submit = async () => {
    setError(null);
    if (!form.name.trim() || !form.apiUrl.trim()) {
      setError("请填写名称与 API URL");
      return;
    }
    const customRequiresSecret =
      form.providerType === "custom" && form.custom.authType !== "none";
    if (
      form.id === null &&
      !NO_API_KEY_TYPES.has(form.providerType) &&
      customRequiresSecret &&
      !form.apiKey.trim()
    ) {
      setError("新增账户必须填写认证凭据");
      return;
    }
    setSaving(true);
    try {
      const customConfig =
        form.providerType === "custom"
          ? JSON.stringify(buildCustomConfig(form))
          : null;
      if (form.providerType === "custom") {
        const approved = await api.isCustomEndpointApproved(form.apiUrl);
        if (!approved) {
          const origin = new URL(form.apiUrl).origin;
          const confirmed = window.confirm(
            `自定义网关将接收你的认证凭据。\n\n目标：${origin}\n\n仅在你信任该服务运营方时批准。是否继续？`,
          );
          if (!confirmed) return;
          await api.approveCustomEndpoint(form.apiUrl);
        }
      }
      if (form.id === null) {
        await api.addProvider({
          name: form.name,
          providerType: form.providerType,
          apiUrl: form.apiUrl,
          apiKey: form.apiKey,
          customConfig,
        });
      } else {
        await api.updateProvider({
          id: form.id,
          name: form.name,
          apiUrl: form.apiUrl,
          apiKey: form.apiKey.trim() ? form.apiKey : null,
          customConfig,
        });
      }
      setForm(createEmptyForm());
      await load();
      window.dispatchEvent(new Event("providers-changed"));
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const remove = async (p: ProviderConfig) => {
    setDeleting(true);
    try {
      const result: DeleteResult = await api.deleteProvider(p.id);
      await load();
      window.dispatchEvent(new Event("providers-changed"));
      setPendingDelete(null);
      if (!result.credentialCleaned) {
        setError(result.note ?? "账户已删除，但凭据清理状态未知");
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setDeleting(false);
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

  const applyPalette = useCallback((palette: Awaited<ReturnType<typeof analyzeCustomBackground>>, visualTheme: "custom" | "luotianyi") => {
    onLayoutChange((prev) => ({
      ...prev,
      visualTheme,
      theme: palette.mode,
      glassOpacity: palette.mode === "light" ? 0.72 : 0.62,
      miniTextColor: palette.miniTextColor ?? palette.overrides["text-primary"],
      themeOverrides: palette.overrides,
    }));
  }, [onLayoutChange]);

  const analyzeAndApply = useCallback(async (image: string, visualTheme: "custom" | "luotianyi", persistPalette = false) => {
    setPaletteBusy(true);
    try {
      const palette = await analyzeCustomBackground(image);
      if (persistPalette) saveCustomBackgroundPalette(palette);
      applyPalette(palette, visualTheme);
      setError(null);
    } catch (e) {
      setError(`颜色提取失败: ${String(e)}`);
    } finally {
      setPaletteBusy(false);
    }
  }, [applyPalette]);

  const selectVisualTheme = (theme: "default" | "luotianyi" | "custom") => {
    if (theme === "custom" && customBackground.palette) {
      applyPalette(customBackground.palette, "custom");
      return;
    }
    if (theme === "luotianyi") {
      const selected = layout.luotianyiBackground === CUSTOM_LUOTIANYI_BACKGROUND_ID
        ? customLuotianyiBackground
        : LUOTIANYI_BACKGROUNDS.find(([id]) => id === layout.luotianyiBackground)?.[2] ?? LUOTIANYI_BACKGROUNDS[0][2];
      onLayoutChange((prev) => ({ ...prev, visualTheme: "luotianyi" }));
      if (selected) void analyzeAndApply(selected, "luotianyi");
      return;
    }
    onLayoutChange((prev) => ({ ...prev, visualTheme: theme }));
  };

  const applyBackgroundCrop = async (crop: Parameters<typeof prepareCustomBackground>[1]) => {
    if (!backgroundCrop || !crop) return;
    setBackgroundCropBusy(true);
    try {
      const prepared = await prepareCustomBackground(backgroundCrop.file, crop);
      const asset = await api.importAsset(
        backgroundCrop.target === "luotianyi" ? "custom-luotianyi-background.jpg" : "custom-background.jpg",
        prepared.bytes,
      );
      if (backgroundCrop.target === "luotianyi") {
        saveCustomLuotianyiBackground(asset);
        setCustomLuotianyiBackground(asset.url);
        onLayoutChange((prev) => ({
          ...prev,
          visualTheme: "luotianyi",
          theme: prepared.palette.mode,
          miniTextColor: prepared.palette.miniTextColor ?? prepared.palette.overrides["text-primary"],
          themeOverrides: prepared.palette.overrides,
          luotianyiBackground: CUSTOM_LUOTIANYI_BACKGROUND_ID,
        }));
      } else {
        saveCustomBackground(asset, prepared.palette);
        applyPalette(prepared.palette, "custom");
      }
      setBackgroundCrop(null);
      setError(null);
    } catch (e) {
      setError(`背景导入失败: ${String(e)}`);
    } finally {
      setBackgroundCropBusy(false);
    }
  };

  const reanalyzeCurrentTheme = () => {
    if (layout.visualTheme === "custom") {
      if (!customBackground.image) {
        setError("请先导入自定义背景图片");
        return;
      }
      void analyzeAndApply(customBackground.image, "custom", true);
      return;
    }
    if (layout.visualTheme === "luotianyi") {
      const image = layout.luotianyiBackground === CUSTOM_LUOTIANYI_BACKGROUND_ID
        ? customLuotianyiBackground
        : LUOTIANYI_BACKGROUNDS.find(([id]) => id === layout.luotianyiBackground)?.[2] ?? LUOTIANYI_BACKGROUNDS[0][2];
      if (image) void analyzeAndApply(image, "luotianyi");
    }
  };

  const persistSavedThemes = (themes: SavedTheme[]) => {
    setSavedThemes(themes);
    localStorage.setItem(SAVED_THEMES_KEY, JSON.stringify(themes));
  };

  const saveCurrentTheme = () => {
    const name = savedThemeName.trim() || `自定义主题 ${savedThemes.length + 1}`;
    const saved: SavedTheme = {
      id: crypto.randomUUID(),
      name: name.slice(0, 24),
      theme: layout.theme,
      overrides: { ...(layout.themeOverrides ?? {}) },
      miniTextColor: layout.miniTextColor,
    };
    persistSavedThemes([...savedThemes, saved]);
    setSelectedSavedThemeId(saved.id);
    setSavedThemeName(saved.name);
    setError(null);
  };

  const renameSavedTheme = () => {
    const name = savedThemeName.trim();
    if (!selectedSavedThemeId || !name) {
      setError("请先选择主题方案并输入新名称");
      return;
    }
    persistSavedThemes(savedThemes.map((theme) => theme.id === selectedSavedThemeId
      ? { ...theme, name: name.slice(0, 24) }
      : theme));
    setError(null);
  };

  return (
    <div className="settings-panel animate-fade-in-up flex flex-col gap-4">
      <Dialog
        open={pendingDelete !== null}
        title="删除 API 账户？"
        confirmLabel="删除账户"
        danger
        busy={deleting}
        onDismiss={() => setPendingDelete(null)}
        onConfirm={() => pendingDelete && void remove(pendingDelete)}
      >
        <p>将删除账户 <strong>“{pendingDelete?.name}”</strong>，并清除系统凭据库中保存的 API Key。</p>
        <p className="mx-dialog-note">此操作无法撤销，但不会影响其他 Provider。</p>
      </Dialog>
      {backgroundCrop && (
        <BackgroundCropper
          file={backgroundCrop.file}
          busy={backgroundCropBusy}
          onCancel={() => setBackgroundCrop(null)}
          onConfirm={(crop) => void applyBackgroundCrop(crop)}
        />
      )}
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
      <section className="settings-section mx-card">
        <div className="section-heading"><div className="section-heading-copy"><h2>应用行为</h2><p className="section-description">控制关闭主窗口和登录系统后的启动方式。</p></div></div>
        <div className="mt-3">
          <SpinnerPreference
            title="关闭按钮行为"
            summary="设置点击关闭按钮时的默认操作"
            value={appBehavior.closeBehavior}
            options={[{ value: "minimize_to_tray", label: "缩小到托盘" }, { value: "quit", label: "直接退出" }]}
            onChange={(value) => void changeCloseBehavior(value)}
          />
          <SwitchPreference
            title="开机自启动"
            summary="登录系统后自动启动 AI API Monitor"
            checked={appBehavior.autoStart}
            onChange={(checked) => void changeAutoStart(checked)}
          />
        </div>
      </section>

      <section className="settings-section mx-card">
        <div className="section-heading">
          <div className="section-heading-copy">
            <h2>{form.id === null ? "添加 API 账户" : `编辑账户：${form.name}`}</h2>
            <p className="section-description">连接服务并安全保存凭据，不会改变 Provider 的请求逻辑。</p>
          </div>
        </div>
        <div className="provider-form-grid mt-3 grid grid-cols-2 gap-2.5">
          <label className="provider-form-field text-[12px] text-text-secondary">
            名称
            <TextField
              className="mt-1"
              value={form.name}
              placeholder="例如 DeepSeek 主账户"
              onChange={(e) => setForm({ ...form, name: e.target.value })}
            />
          </label>
          <div className="provider-form-field text-[12px] text-text-secondary">
            类型
            <AppSelect className="mt-1" value={form.providerType} options={types.map((type) => ({ value: type, label: type }))} onChange={selectType} aria-label="Provider 类型" />
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
            <TextField
              className="mt-1"
              value={form.apiUrl}
              disabled={form.providerType !== "custom"}
              placeholder="https://api.deepseek.com"
              onChange={(e) => setForm({ ...form, apiUrl: e.target.value })}
            />
          </label>
          <label className="col-span-2 text-[12px] text-text-secondary">
            {NO_API_KEY_TYPES.has(form.providerType) ? (
              <div className="rounded-lg border border-accent/25 bg-accent/10 px-2.5 py-2 text-[11px] text-text-primary">
                {form.providerType === "codex"
                  ? codexRuntimeStatus?.loggedIn
                    ? `已连接 ${codexRuntimeStatus.runtimeSource?.startsWith("desktop") ? "ChatGPT/Codex Desktop" : "Codex CLI"}，认证由官方客户端管理。`
                    : codexRuntimeStatus?.installed
                      ? "已检测到官方 Codex Runtime，但当前未登录。"
                      : "未检测到 ChatGPT/Codex Desktop 或 Codex CLI。"
                  : "此类型无需 API Key"}
                {form.providerType === "codex" && codexRuntimeStatus?.installed && !codexRuntimeStatus.loggedIn && (
                  <Button
                    type="button"
                    variant="primary"
                    className="mt-2 w-full"
                    disabled={codexLoginPending}
                    onClick={() => {
                      setCodexLoginPending(true);
                      void api.startCodexLogin().catch((error) => {
                        setCodexLoginPending(false);
                        setError(String(error));
                      });
                    }}
                  >
                    {codexLoginPending ? "等待官方登录完成…" : "登录 ChatGPT"}
                  </Button>
                )}
              </div>
            ) : (
              <>
                {form.providerType === "custom"
                  ? "认证凭据（Token / Key / 密码 / Header 值）"
                  : "API Key"}
                <span className="ml-2 text-[10px] text-text-muted">
                  {form.id === null
                    ? "加密保存至系统凭据库，绝不落库"
                    : "留空表示不修改"}
                </span>
                <PasswordInput
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
          {form.providerType === "custom" && (
            <>
              <label className="col-span-2 text-[12px] text-text-secondary">
                请求方式
                <AppSelect
                  className="mt-1"
                  value={form.custom.method}
                  options={[{ value: "GET", label: "GET" }, { value: "POST", label: "POST" }]}
                  onChange={(value) =>
                    setForm({ ...form, custom: { ...form.custom, method: value as "GET" | "POST" } })
                  }
                  aria-label="请求方式"
                />
              </label>
              <label className="col-span-2 text-[12px] text-text-secondary">
                Query 参数（每行 key=value）
                <textarea
                  className="mt-1 w-full rounded-lg border border-border bg-control px-2.5 py-1.5 text-[12px] text-text-primary"
                  rows={2}
                  value={form.custom.query}
                  placeholder={"key1=value1\nkey2=value2"}
                  onChange={(e) => setForm({ ...form, custom: { ...form.custom, query: e.target.value } })}
                />
              </label>
              <label className="col-span-2 text-[12px] text-text-secondary">
                Headers（每行 key=value；认证头请在下方认证方式配置）
                <textarea
                  className="mt-1 w-full rounded-lg border border-border bg-control px-2.5 py-1.5 text-[12px] text-text-primary"
                  rows={2}
                  value={form.custom.headers}
                  placeholder={"X-Custom=abc"}
                  onChange={(e) => setForm({ ...form, custom: { ...form.custom, headers: e.target.value } })}
                />
              </label>
              <label className="text-[12px] text-text-secondary">
                认证方式
                <AppSelect
                  className="mt-1"
                  value={form.custom.authType}
                  options={[
                    { value: "bearer", label: "Bearer Token" },
                    { value: "apiKey", label: "API Key Header" },
                    { value: "basic", label: "Basic Auth" },
                    { value: "none", label: "无认证" },
                    { value: "customHeader", label: "自定义 Header" },
                  ]}
                  onChange={(value) =>
                    setForm({ ...form, custom: { ...form.custom, authType: value as CustomAuthType } })
                  }
                  aria-label="认证方式"
                />
              </label>
              {(form.custom.authType === "apiKey" || form.custom.authType === "customHeader") && (
                <label className="text-[12px] text-text-secondary">
                  Header 名称
                  <TextField
                    className="mt-1"
                    value={form.custom.authHeaderName}
                    placeholder="X-API-Key"
                    onChange={(e) =>
                      setForm({ ...form, custom: { ...form.custom, authHeaderName: e.target.value } })
                    }
                  />
                </label>
              )}
              {form.custom.authType === "basic" && (
                <label className="text-[12px] text-text-secondary">
                  用户名
                  <TextField
                    className="mt-1"
                    value={form.custom.authUsername}
                    placeholder="username"
                    onChange={(e) =>
                      setForm({ ...form, custom: { ...form.custom, authUsername: e.target.value } })
                    }
                  />
                </label>
              )}
              {form.custom.method === "POST" && (
                <label className="col-span-2 text-[12px] text-text-secondary">
                  Request Body（JSON）
                  <textarea
                    className="mt-1 w-full rounded-lg border border-border bg-control px-2.5 py-1.5 text-[12px] text-text-primary"
                    rows={3}
                    value={form.custom.body}
                    placeholder={'{"key":"value"}'}
                    onChange={(e) => setForm({ ...form, custom: { ...form.custom, body: e.target.value } })}
                  />
                </label>
              )}
              <label className="text-[12px] text-text-secondary">
                剩余额度字段（点路径）
                <TextField className="mt-1" value={form.custom.remainingPath} placeholder="data.remaining" onChange={(e) => setForm({ ...form, custom: { ...form.custom, remainingPath: e.target.value } })} />
              </label>
              <label className="text-[12px] text-text-secondary">
                总额度字段
                <TextField className="mt-1" value={form.custom.totalPath} placeholder="data.total" onChange={(e) => setForm({ ...form, custom: { ...form.custom, totalPath: e.target.value } })} />
              </label>
              <label className="text-[12px] text-text-secondary">
                已使用字段
                <TextField className="mt-1" value={form.custom.usedPath} placeholder="data.used" onChange={(e) => setForm({ ...form, custom: { ...form.custom, usedPath: e.target.value } })} />
              </label>
              <label className="text-[12px] text-text-secondary">
                重置时间字段
                <TextField className="mt-1" value={form.custom.resetTimePath} placeholder="data.resetTime" onChange={(e) => setForm({ ...form, custom: { ...form.custom, resetTimePath: e.target.value } })} />
              </label>
              <label className="text-[12px] text-text-secondary">
                单位
                <AppSelect
                  className="mt-1"
                  value={form.custom.unit}
                  options={[
                    { value: "token", label: "Token" },
                    { value: "count", label: "次数" },
                    { value: "currency", label: "金额" },
                    { value: "custom", label: "自定义" },
                  ]}
                  onChange={(value) =>
                    setForm({ ...form, custom: { ...form.custom, unit: value as CustomUnit } })
                  }
                  aria-label="单位"
                />
              </label>
              <div className="col-span-2 flex items-center gap-2">
                <Button onClick={() => void testCustom()} disabled={testing}>
                  {testing ? "测试中…" : "测试连接"}
                </Button>
              </div>
              {testResult && (
                <div className="col-span-2 rounded-lg border border-border bg-control/50 px-2.5 py-2 text-[11px] text-text-secondary">
                  {testResult.success ? (
                    <>
                      <div className="text-success">连接成功（HTTP {testResult.status}）</div>
                      <div className="mt-1">剩余：{testResult.remaining ?? "—"} · 总额：{testResult.total ?? "—"} · 已用：{testResult.used ?? "—"} · 单位：{testResult.unit}</div>
                      {testResult.resetTime && <div>重置时间：{testResult.resetTime}</div>}
                      {testResult.responsePreview && (
                        <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap break-all rounded bg-black/20 p-2 text-[10px]">{testResult.responsePreview}</pre>
                      )}
                    </>
                  ) : (
                    <div className="text-danger">测试失败：{testResult.error ?? "未知错误"}</div>
                  )}
                </div>
              )}
            </>
          )}
        </div>
        <div className="mt-3 flex gap-2">
          <Button
            variant="primary"
            className="flex-1"
            onClick={() => void submit()}
            disabled={saving}
          >
            {saving ? "保存中…" : form.id === null ? "添加" : "保存修改"}
          </Button>
          {form.id !== null && (
            <Button onClick={cancelEdit}>
              取消
            </Button>
          )}
        </div>
      </section>

      {/* 账户列表 */}
      <section className="settings-section mx-card">
        <div className="section-heading"><div className="section-heading-copy">
          <h2>已配置账户（{providers.length}）</h2>
          <p className="section-description">快速识别类型、地址与脱敏后的凭据状态。</p>
        </div></div>
        {providers.length === 0 ? (
          <p className="mt-3 text-[12px] text-text-muted">暂无账户</p>
        ) : (
          <ul className="mt-2 flex flex-col gap-2">
            {providers.map((p) => (
              <li
                key={p.id}
                className="account-list-item flex items-center justify-between rounded-xl px-3 py-2.5"
              >
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-[13px] font-medium text-text-primary">
                      {p.name}
                    </span>
                    <span className="provider-badge">
                      {p.providerType}
                    </span>
                  </div>
                  <div className="mt-0.5 truncate text-[11px] text-text-muted">
                    {p.apiUrl}
                  </div>
                  {p.keyHint && (
                    <div className="mt-0.5 font-mono text-[10px] text-text-muted" aria-label="API Key 已脱敏">
                      {p.keyHint}
                    </div>
                  )}
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
                    onClick={() => setPendingDelete(p)}
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
      <section className="settings-section mx-card">
        <div className="section-heading"><div className="section-heading-copy">
          <h2>刷新策略</h2>
          <p className="section-description">分别控制前台与后台轮询频率。</p>
        </div></div>
        <div className="mt-3 grid grid-cols-2 gap-2.5">
          <label className="text-[12px] text-text-secondary">
            前台刷新（秒）
            <NumberStepper
              label="前台刷新秒数"
              min={10}
              max={3600}
              value={refresh.foregroundSecs}
              onChange={(value) =>
                setRefresh({ ...refresh, foregroundSecs: value })
              }
            />
          </label>
          <label className="text-[12px] text-text-secondary">
            后台刷新（秒）
            <NumberStepper
              label="后台刷新秒数"
              min={60}
              max={3600}
              value={refresh.backgroundSecs}
              onChange={(value) =>
                setRefresh({ ...refresh, backgroundSecs: value })
              }
            />
          </label>
        </div>
        <button className="btn btn-ghost mt-3" onClick={() => void saveRefresh()}>
          保存刷新策略
        </button>
      </section>

      <section className="settings-section mx-card">
        <div className="section-heading"><div className="section-heading-copy"><h2>悬浮窗额度切换</h2><p className="section-description">长方形悬浮窗在多个 Provider 之间切换显示，不会触发新的 API 请求。</p></div></div>
        <div className="mt-3">
          <SpinnerPreference
            title="额度切换方式"
            value={layout.floatingScrollMode ?? "auto"}
            options={[{ value: "auto", label: "自动滚动" }, { value: "wheel", label: "鼠标滚轮" }]}
            onChange={(value) => onLayoutChange((prev) => ({ ...prev, floatingScrollMode: value as "auto" | "wheel" }))}
          />
        </div>
      </section>

      {/* V1.0 主题分享：自定义色值 + 导出/导入 */}
      <PreferenceGroup title="外观与主题" description="壁纸、动态取色与手动颜色共同驱动全部语义组件。" className="appearance-preferences settings-section">
        <SwitchPreference title="深色模式" summary="在亮色与深色配色之间切换" checked={layout.theme === "dark"} onChange={(checked) => onLayoutChange((prev) => ({ ...prev, theme: checked ? "dark" : "light" }))} />
        <div className="theme-segment grid grid-cols-3 gap-2" aria-label="主题类型">
          {(["default", "luotianyi", "custom"] as const).map((theme) => (
            <button key={theme} type="button" className={`theme-choice ${((layout.visualTheme ?? "default") === theme) ? "is-active" : ""}`} onClick={() => selectVisualTheme(theme)}>
              {theme === "default" ? "默认" : theme === "luotianyi" ? "洛天依" : "自定义 +"}
            </button>
          ))}
        </div>
        {layout.visualTheme === "luotianyi" && (
          <div className="wallpaper-panel">
            <div className="wallpaper-panel-heading">
              <div><span>壁纸</span><small>横向拖动查看更多</small></div>
              <span>{LUOTIANYI_BACKGROUNDS.find(([id]) => id === (layout.luotianyiBackground ?? LUOTIANYI_BACKGROUNDS[0][0]))?.[1] ?? "当前壁纸"}</span>
            </div>
            <div
              ref={luotianyiBackgroundScrollerRef}
              className="luotianyi-background-scroller"
              onPointerDown={(event) => {
                if (event.button !== 0) return;
                const scroller = luotianyiBackgroundScrollerRef.current;
                if (!scroller) return;
                luotianyiBackgroundDragRef.current = {
                  pointerId: event.pointerId,
                  startX: event.clientX,
                  scrollLeft: scroller.scrollLeft,
                  moved: false,
                };
                scroller.classList.add("is-dragging");
              }}
              onPointerMove={(event) => {
                const drag = luotianyiBackgroundDragRef.current;
                const scroller = luotianyiBackgroundScrollerRef.current;
                if (!scroller || drag.pointerId !== event.pointerId) return;
                const distance = event.clientX - drag.startX;
                if (Math.abs(distance) > 4 && !drag.moved) {
                  drag.moved = true;
                  scroller.setPointerCapture(event.pointerId);
                }
                scroller.scrollLeft = drag.scrollLeft - distance;
              }}
              onPointerUp={(event) => {
                const drag = luotianyiBackgroundDragRef.current;
                const scroller = luotianyiBackgroundScrollerRef.current;
                if (!scroller || drag.pointerId !== event.pointerId) return;
                if (drag.moved) suppressLuotianyiBackgroundClickUntilRef.current = Date.now() + 180;
                if (scroller.hasPointerCapture(event.pointerId)) scroller.releasePointerCapture(event.pointerId);
                scroller.classList.remove("is-dragging");
                drag.pointerId = -1;
              }}
              onPointerCancel={() => {
                luotianyiBackgroundScrollerRef.current?.classList.remove("is-dragging");
                luotianyiBackgroundDragRef.current.pointerId = -1;
              }}
              onClickCapture={(event) => {
                if (Date.now() >= suppressLuotianyiBackgroundClickUntilRef.current) return;
                event.preventDefault();
                event.stopPropagation();
              }}
            >
              {LUOTIANYI_BACKGROUNDS.map(([id, label, path]) => (
                <button
                  key={id}
                  type="button"
                  className={`luotianyi-background-choice ${(layout.luotianyiBackground ?? LUOTIANYI_BACKGROUNDS[0][0]) === id ? "is-active" : ""}`}
                  aria-label={`选择${label}背景`}
                  aria-pressed={(layout.luotianyiBackground ?? LUOTIANYI_BACKGROUNDS[0][0]) === id}
                  onClick={() => {
                    onLayoutChange((prev) => ({ ...prev, luotianyiBackground: id }));
                    void analyzeAndApply(path, "luotianyi");
                  }}
                >
                  <img src={path} alt="" />
                </button>
              ))}
              {customLuotianyiBackground && (
                <button
                  type="button"
                  className={`luotianyi-background-choice ${layout.luotianyiBackground === CUSTOM_LUOTIANYI_BACKGROUND_ID ? "is-active" : ""}`}
                  aria-label="选择我的背景"
                  aria-pressed={layout.luotianyiBackground === CUSTOM_LUOTIANYI_BACKGROUND_ID}
                  onClick={() => {
                    onLayoutChange((prev) => ({ ...prev, luotianyiBackground: CUSTOM_LUOTIANYI_BACKGROUND_ID }));
                    void analyzeAndApply(customLuotianyiBackground, "luotianyi");
                  }}
                >
                  <img src={customLuotianyiBackground} alt="" />
                </button>
              )}
              <label className="luotianyi-background-add" aria-label="添加自己的背景">
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path d="M12 5v14M5 12h14" />
                </svg>
                <input
                  type="file"
                  accept="image/png,image/jpeg,image/webp"
                  className="hidden"
                  onChange={(event) => {
                    const file = event.target.files?.[0];
                    if (!file) return;
                    setBackgroundCrop({ file, target: "luotianyi" });
                    setError(null);
                    event.target.value = "";
                  }}
                />
              </label>
            </div>
          </div>
        )}
        {layout.visualTheme === "custom" && <div className="theme-control-panel mt-3 p-3">
          <div className="text-[12px] font-medium text-text-primary">自定义主背景</div>
          <p className="mt-1 text-[10px] text-text-muted">导入图片后可拖动、缩放并选择裁切区域，确认后自动压缩并提取配色。</p>
          <div className="mt-2 flex gap-2">
            <label className="btn btn-primary flex-1 cursor-pointer px-3 py-1.5 text-[11px]">
              导入背景图片
              <input
                type="file"
                accept="image/png,image/jpeg,image/webp"
                className="hidden"
                onChange={(event) => {
                  const file = event.target.files?.[0];
                  if (!file) return;
                  setBackgroundCrop({ file, target: "custom" });
                  setError(null);
                  event.target.value = "";
                }}
              />
            </label>
            <button type="button" className="btn btn-ghost px-3 py-1.5 text-[11px]" onClick={() => void clearCustomBackground()}>恢复主题背景</button>
          </div>
        </div>}
        <SliderPreference title="表面透明度" summary="仅影响壁纸上的内容表面，背景图片保持清晰" min={15} max={90} value={Math.round((layout.glassOpacity ?? 0.58) * 100)} valueLabel={`${Math.round((layout.glassOpacity ?? 0.58) * 100)}%`} onChange={(value) => onLayoutChange((prev) => ({ ...prev, glassOpacity: value / 100 }))} />
        <SliderPreference title="背景模糊度" summary="仅在壁纸背景下启用；无壁纸时使用实色表面" min={0} max={32} value={Math.round(layout.glassBlur ?? 18)} valueLabel={`${Math.round(layout.glassBlur ?? 18)} px`} onChange={(value) => onLayoutChange((prev) => ({ ...prev, glassBlur: value }))} />
        <div className="theme-control-panel mt-3 p-3">
          <div className="flex items-center gap-3">
            <img
              className="h-16 w-16 shrink-0 rounded-xl bg-white/5 object-contain"
              src={luotianyiGifPath(layout.avatarGif)}
              alt="当前动态角色图标"
            />
            <div className="min-w-0 flex-1"><SpinnerPreference title="动态角色图标" value={layout.avatarGif ?? ""} options={[{value:"",label:"关闭"}, ...LUOTIANYI_GIFS.map(([value,label])=>({value,label})), ...(layout.avatarGif === CUSTOM_GIF_ID ? [{value:CUSTOM_GIF_ID,label:"自定义 GIF"}] : [])]} onChange={(value) => onLayoutChange((prev) => ({...prev,avatarGif:value || undefined}))} /></div>
          </div>
          <label className="btn btn-ghost mt-2 w-full cursor-pointer px-3 py-1.5 text-[11px]">
            导入自定义 GIF
            <input
              type="file"
              accept="image/gif"
              className="hidden"
              onChange={(event) => {
                const file = event.target.files?.[0];
                if (!file) return;
                if (file.type !== "image/gif" || file.size > 20 * 1024 * 1024) {
                  setError("请选择不超过 20 MB 的 GIF 文件");
                  event.target.value = "";
                  return;
                }
                void file.arrayBuffer().then(async (buffer) => {
                  try {
                    const asset = await api.importAsset(file.name, new Uint8Array(buffer));
                    saveCustomAvatarGif(asset);
                    onLayoutChange((prev) => ({ ...prev, avatarGif: CUSTOM_GIF_ID }));
                    setError(null);
                  } catch (error) {
                    setError(`GIF 导入失败: ${String(error)}`);
                  }
                }).catch(() => setError("GIF 读取失败"));
                event.target.value = "";
              }}
            />
          </label>
          <p className="mt-2 text-[10px] text-text-muted">可独立用于默认主题、洛天依主题、Mini 窗口和悬浮图标。</p>
        </div>
        <div className="theme-control-panel mt-3 p-3">
          <div className="flex items-center justify-between gap-3">
            <div className="min-w-0">
              <div className="settings-row-title">Mini 窗口字体颜色</div>
              <p className="settings-row-description">仅影响紧凑悬浮窗；导入或重新提取壁纸颜色时会自动选择高对比文字色。</p>
            </div>
            <input
              type="color"
              className="h-8 w-14 shrink-0 cursor-pointer"
              value={layout.miniTextColor ?? hexOf(layout.themeOverrides?.["text-primary"], layout.theme, "text-primary")}
              onChange={(event) => onLayoutChange((prev) => ({ ...prev, miniTextColor: event.target.value }))}
              aria-label="Mini 窗口字体颜色"
            />
          </div>
          <div className="mini-text-preview mt-3" style={{ color: layout.miniTextColor ?? "var(--color-text-primary)" }}>
            <span className="font-bold">9.35 CNY</span>
            <span>剩余额度</span>
          </div>
          {(layout.visualTheme === "custom" || layout.visualTheme === "luotianyi") && (
            <button type="button" className="btn btn-ghost mt-3 w-full px-3 py-1.5 text-[11px]" disabled={paletteBusy} onClick={reanalyzeCurrentTheme}>
              {paletteBusy ? "正在识别…" : "根据当前壁纸自动识别"}
            </button>
          )}
        </div>
        {(layout.visualTheme === "custom" || layout.visualTheme === "luotianyi") && <details className="theme-color-disclosure">
          <summary>
            <span><strong>手动颜色选项</strong><small>按需调整 Theme Token</small></span>
            <span className="theme-color-disclosure-meta">{THEME_OVERRIDE_KEYS.length} 项</span>
            <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4 6 4 4 4-4" /></svg>
          </summary>
          <div className="theme-color-disclosure-body">
          <div className="mb-3 flex gap-2">
            <button type="button" className="btn btn-ghost flex-1 px-3 py-1.5 text-[11px]" disabled={paletteBusy} onClick={reanalyzeCurrentTheme}>
              {paletteBusy ? "正在提取…" : "重新提取颜色"}
            </button>
            <button
              type="button"
              className="btn btn-ghost flex-1 px-3 py-1.5 text-[11px]"
              onClick={() => onLayoutChange((prev) => ({ ...prev, themeOverrides: undefined }))}
            >
              恢复默认配色
            </button>
          </div>
          <div className="grid grid-cols-2 gap-2.5">
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
          </div>
        </details>}
        {layout.visualTheme === "custom" && <div className="theme-control-panel mt-3 p-3">
          <div className="grid grid-cols-[1fr_auto] gap-2">
            <input
              className="input py-1.5 text-[11px]"
              value={savedThemeName}
              maxLength={24}
              placeholder="主题方案名称"
              onChange={(event) => setSavedThemeName(event.target.value)}
            />
            <button type="button" className="btn btn-primary px-3 py-1.5 text-[11px]" onClick={saveCurrentTheme}>保存</button>
          </div>
          {savedThemes.length > 0 && <div className="mt-2 grid grid-cols-[1fr_auto_auto] gap-2">
            <AppSelect
              className="saved-theme-select"
              value={selectedSavedThemeId}
              placeholder="选择已保存主题"
              options={savedThemes.map((theme) => ({ value: theme.id, label: theme.name }))}
              onChange={(id) => {
                const saved = savedThemes.find((theme) => theme.id === id);
                setSelectedSavedThemeId(id);
                setSavedThemeName(saved?.name ?? "");
              }}
              aria-label="选择已保存主题"
            />
            <button
              type="button"
              className="btn btn-ghost px-3 py-1.5 text-[11px]"
              disabled={!selectedSavedThemeId}
              onClick={() => {
                const saved = savedThemes.find((theme) => theme.id === selectedSavedThemeId);
                if (!saved) return;
                onLayoutChange((prev) => ({ ...prev, theme: saved.theme, themeOverrides: { ...saved.overrides }, miniTextColor: saved.miniTextColor }));
              }}
            >
              应用
            </button>
            <button type="button" className="btn btn-ghost px-3 py-1.5 text-[11px]" disabled={!selectedSavedThemeId} onClick={renameSavedTheme}>重命名</button>
          </div>}
        </div>}
        {layout.visualTheme === "custom" && <div className="mt-3 flex flex-wrap gap-2">
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
                  miniTextColor: imported.miniTextColor,
                }));
                setError(null);
              } catch (e) {
                setError(`导入失败: ${String(e)}`);
              }
            }}
          >
            从剪贴板导入
          </button>
        </div>}
        <p className="mt-2 text-[10px] text-text-muted">
          自定义色值随布局保存；导出后可在其他设备「从剪贴板导入」共享主题。
        </p>
      </PreferenceGroup>

      {/* V1.0 关于与自动更新 */}
      <section className="settings-section mx-card">
        <div className="section-heading"><div className="section-heading-copy">
          <h2>关于与更新</h2>
          <p className="section-description">版本信息、更新检查与项目入口。</p>
        </div></div>
        <p className="mt-2 text-[12px] text-text-secondary">AI API Monitor v0.1.0</p>
        <p className="mt-1 text-[10px] text-text-muted">本软件使用 MiSans 字体。MiSans © Xiaomi Inc.，字体文件按小米官方许可嵌入使用。</p>
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
  "accent-dim": "强调暗色",
  "accent-contrast": "按钮文字",
  surface: "背景",
  card: "卡片",
  "card-hover": "卡片悬停",
  control: "输入控件",
  "control-hover": "控件悬停",
  border: "边框",
  "text-primary": "主文字",
  "text-secondary": "次级文字",
  "text-muted": "弱化文字",
  success: "成功色",
  warning: "警告色",
  danger: "危险色",
};
const THEME_DEFAULTS: Record<Layout["theme"], Record<string, string>> = {
  dark: {
    accent: "#6c8cff", "accent-dim": "#4a63c9", "accent-contrast": "#0b0e14",
    surface: "#0f1115", card: "#1a1d24", "card-hover": "#22262f",
    control: "#20242d", "control-hover": "#292e39", border: "#2a2f3a",
    "text-primary": "#e6e9ef", "text-secondary": "#9aa3b2", "text-muted": "#6b7280",
    success: "#34d399", warning: "#fbbf24", danger: "#f87171",
  },
  light: {
    accent: "#4c6ef5", "accent-dim": "#3b5bdb", "accent-contrast": "#ffffff",
    surface: "#f2f4f8", card: "#ffffff", "card-hover": "#f6f8fb",
    control: "#ffffff", "control-hover": "#f1f4f9", border: "#dde2ea",
    "text-primary": "#1c2333", "text-secondary": "#5b6472", "text-muted": "#7b8492",
    success: "#12b886", warning: "#f59f00", danger: "#fa5252",
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
      miniTextColor: layout.miniTextColor,
    },
    null,
    2,
  );
}

/** 解析导入的主题 JSON；无效返回 null */
function importTheme(text: string): { theme: Layout["theme"]; overrides: Record<string, string>; miniTextColor?: string } | null {
  try {
    const parsed = JSON.parse(text) as {
      theme?: string;
      overrides?: Record<string, string>;
      miniTextColor?: string;
    };
    if (parsed.theme !== "dark" && parsed.theme !== "light") return null;
    const overrides: Record<string, string> = {};
    for (const [k, v] of Object.entries(parsed.overrides ?? {})) {
      if (THEME_OVERRIDE_KEYS.includes(k as (typeof THEME_OVERRIDE_KEYS)[number]) && typeof v === "string" && /^#[0-9a-fA-F]{6}$/.test(v)) overrides[k] = v;
    }
    return { theme: parsed.theme, overrides, miniTextColor: /^#[0-9a-fA-F]{6}$/.test(parsed.miniTextColor ?? "") ? parsed.miniTextColor : undefined };
  } catch {
    return null;
  }
}

const SAVED_THEMES_KEY = "ai-monitor-saved-custom-themes";
type SavedTheme = {
  id: string;
  name: string;
  theme: Layout["theme"];
  overrides: Record<string, string>;
  miniTextColor?: string;
};

function readSavedThemes(): SavedTheme[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(SAVED_THEMES_KEY) ?? "[]") as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.flatMap((entry): SavedTheme[] => {
      if (!entry || typeof entry !== "object") return [];
      const candidate = entry as Partial<SavedTheme>;
      if (typeof candidate.id !== "string" || typeof candidate.name !== "string"
        || (candidate.theme !== "dark" && candidate.theme !== "light")
        || !candidate.overrides || typeof candidate.overrides !== "object") return [];
      const overrides: Record<string, string> = {};
      for (const [key, value] of Object.entries(candidate.overrides)) {
        if (THEME_OVERRIDE_KEYS.includes(key as (typeof THEME_OVERRIDE_KEYS)[number])
          && typeof value === "string" && /^#[0-9a-fA-F]{6}$/.test(value)) overrides[key] = value;
      }
      const miniTextColor = typeof candidate.miniTextColor === "string" && /^#[0-9a-fA-F]{6}$/.test(candidate.miniTextColor)
        ? candidate.miniTextColor
        : undefined;
      return [{ id: candidate.id, name: candidate.name.slice(0, 24), theme: candidate.theme, overrides, miniTextColor }];
    });
  } catch {
    return [];
  }
}

import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import type {
  DeleteResult,
  ProviderConfig,
  RefreshSettings,
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
};

/** 使用 CLI 本地凭证的类型（无需输入 API Key） */
const NO_API_KEY_TYPES = new Set(["codex"]);

/** 设置页：Provider 增删改查 + 刷新策略 */
export default function Settings() {
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
          <label className="text-[12px] text-text-secondary">
            类型
            <select
              className="input mt-1"
              value={form.providerType}
              onChange={(e) => {
                const t = e.target.value;
                setForm({
                  ...form,
                  providerType: t,
                  apiUrl: TYPE_PRESETS[t] ?? form.apiUrl,
                });
              }}
            >
              {types.map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </select>
          </label>
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
            className="h-4 w-4 accent-(--color-accent)"
          />
        </label>
      </section>
    </div>
  );
}

import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import {
  Activity,
  ChevronDown,
  CheckCircle2,
  Clock3,
  FileText,
  FileKey2,
  KeyRound,
  Loader2,
  Plus,
  Power,
  RefreshCw,
  Rocket,
  ShieldCheck,
  Trash2,
  UserRoundPlus,
  Zap
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useState } from "react";

type ProfileKind = "chat_gpt_login" | "proxy_api_key" | "custom";
type CodexSystem = "account" | "api";

type CurrentCodexState = {
  codex_dir: string;
  config_path: string;
  auth_path: string;
  config_exists: boolean;
  auth_exists: boolean;
  config_hash?: string;
  auth_hash?: string;
  model?: string;
  base_url?: string;
  account_email?: string;
  account_name?: string;
  account_plan?: string;
  account_id?: string;
  auth_mode: string;
  active_profile_id?: string;
  session_size: number;
};

type ProfileSummary = {
  id: string;
  workspace_id: string;
  name: string;
  kind: ProfileKind;
  notes: string;
  created_at: string;
  updated_at: string;
  config_hash?: string;
  auth_hash?: string;
  model?: string;
  base_url?: string;
  account_email?: string;
  account_name?: string;
  account_plan?: string;
  account_id?: string;
  has_config: boolean;
  has_auth: boolean;
  codex_system: CodexSystem;
  is_active: boolean;
};

type AppState = {
  current: CurrentCodexState;
  profiles: ProfileSummary[];
};

type ClearCodexStateResult = {
  message: string;
  backup_dir?: string;
  removed: string[];
  app_state: AppState;
};

type SwitchProfileResult = {
  message: string;
  app_state: AppState;
};

type SystemProbeStatus = "ok" | "warning" | "error";

type SystemProbeCheck = {
  status: SystemProbeStatus;
  title: string;
  requirement: string;
  detail: string;
  suggestion: string;
};

type SystemProbeReport = {
  generated_at: string;
  summary: string;
  codex_ready: boolean;
  codex_ready_title: string;
  codex_ready_detail: string;
  checks: SystemProbeCheck[];
};

type ActionFeedback = {
  kind: "success" | "info" | "error";
  title: string;
  detail: string;
};

type UpdateState = {
  available: boolean;
  currentVersion?: string;
  version?: string;
  date?: string;
  body?: string;
  progress?: string;
};

type ImportForm = {
  name: string;
  kind: ProfileKind;
  notes: string;
};

type ProxyForm = {
  name: string;
  base_url: string;
  api_key: string;
  model: string;
  review_model: string;
  reasoning_effort: string;
  notes: string;
  codex_system: CodexSystem;
};

type GogoaisLoginForm = {
  username: string;
  password: string;
};

type GogoaisCodexKeyResult = {
  api_key: string;
  base_url?: string;
  openai_base_url?: string;
  api_key_name?: string;
  expires_at?: string;
  service_status?: string;
  quota?: number;
};

const defaultProxyForm: ProxyForm = {
  name: "Codex 中转",
  base_url: "https://code.gogoais.com",
  api_key: "",
  model: "gpt-5.5",
  review_model: "gpt-5.5",
  reasoning_effort: "xhigh",
  notes: "",
  codex_system: "account"
};

const kindLabel: Record<ProfileKind, string> = {
  chat_gpt_login: "Plus/Pro 登录",
  proxy_api_key: "中转 API Key",
  custom: "自定义"
};

const systemLabel: Record<CodexSystem, string> = {
  account: "沿用 ChatGPT 登录态",
  api: "只用 API Key"
};

const probeStatusLabel: Record<SystemProbeStatus, string> = {
  ok: "正常",
  warning: "提醒",
  error: "失败"
};

const appBuildLabel = "v0.1.2-windows-process";

function formatSystemProbeReport(report: SystemProbeReport) {
  const lines = [
    "Codex 使用环境检测报告",
    `生成时间：${report.generated_at}`,
    `结论：${report.codex_ready_title}`,
    `是否具备 Codex 使用环境条件：${report.codex_ready ? "是" : "否"}`,
    `说明：${report.codex_ready_detail}`,
    `摘要：${report.summary}`,
    "",
    "逐项结果："
  ];
  report.checks.forEach((check, index) => {
    lines.push(
      `${index + 1}. ${check.title}：${probeStatusLabel[check.status]}`,
      `   条件：${check.requirement}`,
      `   结果：${check.detail}`,
      `   建议：${check.suggestion}`
    );
  });
  return lines.join("\n");
}

function accountLabel(account: Pick<ProfileSummary, "account_email" | "account_name" | "account_id">) {
  return account.account_email ?? account.account_name ?? account.account_id ?? "未识别账号";
}

function planLabel(plan?: string) {
  return plan ? plan.toUpperCase() : "未识别";
}

function formatBytes(bytes?: number) {
  if (!bytes) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value >= 10 || unitIndex === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex]}`;
}

function formatBeijingDateTime(value?: string) {
  if (!value) {
    return "";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  const parts = new Intl.DateTimeFormat("zh-CN", {
    timeZone: "Asia/Shanghai",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
    hourCycle: "h23"
  }).formatToParts(date);
  const get = (type: string) => parts.find((part) => part.type === type)?.value ?? "";
  return `${get("year")}-${get("month")}-${get("day")} ${get("hour")}:${get("minute")}:${get("second")} 北京时间`;
}

function serviceStatusLabel(status?: string) {
  if (!status) {
    return "";
  }
  return status.toLowerCase() === "active" ? "active（可用）" : status;
}

function gogoaisKeyErrorMessage(err: unknown) {
  const message = String(err);
  const lower = message.toLowerCase();
  if (
    lower.includes("invalid username or password") ||
    lower.includes("unauthorized") ||
    lower.includes("401")
  ) {
    return "gogoais 账号或密码不正确，请检查后重试。";
  }
  return message;
}

function updateErrorMessage(err: unknown) {
  const message = String(err);
  if (
    message.includes("does not have any endpoints") ||
    message.includes("Updater does not have any endpoints") ||
    message.includes("pubkey")
  ) {
    return "在线更新还没有配置 GitHub Releases endpoint 或 updater 公钥。请检查 tauri.conf.json / tauri.updater.conf.json。";
  }
  if (message.includes("latest.json") && (message.includes("404") || message.includes("Not Found"))) {
    return "更新源已经配置，但 GitHub Releases 还没有发布 latest.json。推送 v* 标签并发布 release 后即可检查更新。";
  }
  return message;
}

function isTauriRuntime() {
  return "__TAURI_INTERNALS__" in window;
}

function previewState(): AppState {
  return {
    current: {
      codex_dir: "~/.codex",
      config_path: "~/.codex/config.toml",
      auth_path: "~/.codex/auth.json",
      config_exists: false,
      auth_exists: false,
      account_email: undefined,
      account_name: undefined,
      account_plan: undefined,
      account_id: undefined,
      auth_mode: "浏览器预览",
      active_profile_id: undefined,
      session_size: 0
    },
    profiles: []
  };
}

function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState("");
  const [notice, setNotice] = useState("");
  const [lastAction, setLastAction] = useState<ActionFeedback | null>(null);
  const [mode, setMode] = useState<"import" | "proxy">("proxy");
  const [pendingSwitch, setPendingSwitch] = useState<ProfileSummary | null>(null);
  const [pendingReset, setPendingReset] = useState(false);
  const [restartCodex, setRestartCodex] = useState(true);
  const [systemProbe, setSystemProbe] = useState<SystemProbeReport | null>(null);
  const [probeExpanded, setProbeExpanded] = useState(true);
  const [availableUpdate, setAvailableUpdate] = useState<Update | null>(null);
  const [updateState, setUpdateState] = useState<UpdateState | null>(null);
  const [downloadedBytes, setDownloadedBytes] = useState(0);
  const [downloadTotal, setDownloadTotal] = useState<number | null>(null);
  const [importForm, setImportForm] = useState<ImportForm>({
    name: "我的 Plus/Pro 账号",
    kind: "chat_gpt_login",
    notes: ""
  });
  const [proxyForm, setProxyForm] = useState<ProxyForm>(defaultProxyForm);
  const [gogoaisLogin, setGogoaisLogin] = useState<GogoaisLoginForm>({
    username: "",
    password: ""
  });

  const activeProfile = useMemo(
    () => state?.profiles.find((profile) => profile.is_active),
    [state]
  );

  function clearProbeReport() {
    setSystemProbe(null);
    setProbeExpanded(true);
  }

  async function load() {
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    setBusy("refresh");
    try {
      if (!isTauriRuntime()) {
        setState(previewState());
        return;
      }
      setState(await invoke<AppState>("get_app_state"));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy("");
    }
  }

  useEffect(() => {
    load();
  }, []);

  useEffect(() => {
    const email = state?.current.account_email;
    if (!email) {
      return;
    }
    setImportForm((current) => {
      if (current.name !== "我的 Plus/Pro 账号") {
        return current;
      }
      return { ...current, name: email };
    });
  }, [state?.current.account_email]);

  async function runAction(action: string, task: () => Promise<AppState>) {
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    setBusy(action);
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中执行账号写入和切换操作");
      }
      setState(await task());
    } catch (err) {
      setError(String(err));
      setLastAction({ kind: "error", title: "操作失败", detail: String(err) });
    } finally {
      setBusy("");
    }
  }

  function submitImport(event: FormEvent) {
    event.preventDefault();
    runAction("import", () =>
      invoke<AppState>("import_current_profile", { input: importForm })
    );
  }

  function submitProxy(event: FormEvent) {
    event.preventDefault();
    runAction("proxy", () =>
      invoke<AppState>("create_proxy_profile", { input: proxyForm })
    ).then(() => setProxyForm({ ...defaultProxyForm, api_key: "" }));
  }

  function switchTo(profile: ProfileSummary) {
    setPendingSwitch(profile);
    setRestartCodex(true);
  }

  async function confirmSwitch() {
    if (!pendingSwitch) {
      return;
    }
    const profile = pendingSwitch;
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    setBusy(`switch-${profile.id}`);
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中执行账号写入和切换操作");
      }
      let message = `${profile.name} 已切换`;
      if (restartCodex) {
        const result = await invoke<SwitchProfileResult>("switch_profile_and_restart", {
          input: { id: profile.id }
        });
        setState(result.app_state);
        message = `${profile.name} 已切换。${result.message}`;
      } else {
        const nextState = await invoke<AppState>("switch_profile", {
          input: { id: profile.id }
        });
        setState(nextState);
      }
      setPendingSwitch(null);
      setNotice(message);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy("");
    }
  }

  function remove(profile: ProfileSummary) {
    runAction(`delete-${profile.id}`, () =>
      invoke<AppState>("delete_profile", { id: profile.id })
    );
  }

  async function restartCodexApp() {
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    setBusy("restart-codex");
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中重启 Codex app");
      }
      const message = await invoke<string>("restart_codex_app");
      setNotice(message);
      setLastAction({ kind: "success", title: "已重启 Codex", detail: message });
    } catch (err) {
      setError(String(err));
      setLastAction({ kind: "error", title: "重启失败", detail: String(err) });
    } finally {
      setBusy("");
    }
  }

  async function quitCodexApp() {
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    setBusy("quit-codex");
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中关闭 Codex app");
      }
      const message = await invoke<string>("quit_codex_app");
      setNotice(message);
      setLastAction({ kind: "success", title: "已关闭 Codex", detail: message });
    } catch (err) {
      setError(String(err));
      setLastAction({ kind: "error", title: "关闭失败", detail: String(err) });
    } finally {
      setBusy("");
    }
  }

  async function openConfigToml() {
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    setBusy("open-config");
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中打开 config.toml");
      }
      const message = await invoke<string>("open_codex_config");
      setNotice(message);
      setLastAction({ kind: "success", title: "已打开 config.toml", detail: message });
    } catch (err) {
      setError(String(err));
      setLastAction({ kind: "error", title: "打开失败", detail: String(err) });
    } finally {
      setBusy("");
    }
  }

  async function detectCodexEnvironment() {
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    setBusy("detect-codex-env");
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中检测 Codex 使用环境");
      }
      const report = await invoke<SystemProbeReport>("detect_codex_environment");
      setSystemProbe(report);
      setProbeExpanded(true);
      try {
        const clipboardMessage = await invoke<string>("copy_text_to_clipboard", {
          text: formatSystemProbeReport(report)
        });
        const detail = `${report.summary} ${clipboardMessage}。`;
        setNotice(detail);
        setLastAction({ kind: "success", title: "Codex 环境检测完成", detail });
      } catch (clipboardErr) {
        const detail = `${report.summary} 但复制到剪贴板失败：${String(clipboardErr)}`;
        setNotice(report.summary);
        setLastAction({ kind: "error", title: "检测完成，但复制失败", detail });
      }
    } catch (err) {
      setError(String(err));
      setLastAction({ kind: "error", title: "Codex 环境检测失败", detail: String(err) });
    } finally {
      setBusy("");
    }
  }

  async function checkForUpdates() {
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    setAvailableUpdate(null);
    setUpdateState(null);
    setDownloadedBytes(0);
    setDownloadTotal(null);
    setBusy("check-update");
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中检查更新");
      }
      const update = await check();
      if (!update) {
        const detail = "当前已经是最新版本。";
        setNotice(detail);
        setLastAction({ kind: "info", title: "没有可用更新", detail });
        return;
      }
      setAvailableUpdate(update);
      setUpdateState({
        available: true,
        currentVersion: update.currentVersion,
        version: update.version,
        date: update.date,
        body: update.body
      });
      const detail = `发现新版本 ${update.version}，当前版本 ${update.currentVersion}。`;
      setNotice(detail);
      setLastAction({ kind: "success", title: "发现新版本", detail });
    } catch (err) {
      const detail = updateErrorMessage(err);
      setError(detail);
      setLastAction({ kind: "error", title: "检查更新失败", detail });
    } finally {
      setBusy("");
    }
  }

  async function installAvailableUpdate() {
    if (!availableUpdate) {
      return;
    }
    setError("");
    setNotice(`正在下载 ${availableUpdate.version}...`);
    setLastAction({
      kind: "info",
      title: "正在安装更新",
      detail: "下载完成后会安装更新并重启切号器。"
    });
    setDownloadedBytes(0);
    setDownloadTotal(null);
    setBusy("install-update");
    try {
      await availableUpdate.downloadAndInstall((event: DownloadEvent) => {
        if (event.event === "Started") {
          setDownloadTotal(event.data.contentLength ?? null);
          setDownloadedBytes(0);
          setUpdateState((current) => ({
            ...(current ?? { available: true }),
            progress: event.data.contentLength
              ? `开始下载，大小 ${formatBytes(event.data.contentLength)}。`
              : "开始下载更新包。"
          }));
        }
        if (event.event === "Progress") {
          setDownloadedBytes((current) => current + event.data.chunkLength);
        }
        if (event.event === "Finished") {
          setUpdateState((current) => ({
            ...(current ?? { available: true }),
            progress: "下载完成，正在安装并重启。"
          }));
        }
      });
      setNotice("更新已安装，正在重启切号器。");
      setLastAction({ kind: "success", title: "更新已安装", detail: "切号器会立即重启到新版本。" });
      await relaunch();
    } catch (err) {
      const detail = updateErrorMessage(err);
      setError(detail);
      setLastAction({ kind: "error", title: "安装更新失败", detail });
    } finally {
      setBusy("");
    }
  }

  async function fetchGogoaisCodexKey() {
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    setBusy("fetch-gogoais-key");
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中获取中转 API Key");
      }
      const result = await invoke<GogoaisCodexKeyResult>("fetch_gogoais_codex_key", {
        input: gogoaisLogin
      });
      const nextBaseUrl = result.base_url ?? result.openai_base_url ?? proxyForm.base_url;
      setProxyForm((current) => ({
        ...current,
        api_key: result.api_key,
        base_url: nextBaseUrl,
        codex_system: current.codex_system
      }));
      const expiresAt = formatBeijingDateTime(result.expires_at);
      const serviceStatus = serviceStatusLabel(result.service_status);
      const detail = [
        "已获取并填入 API Key。",
        result.api_key_name ? `Key：${result.api_key_name}` : "",
        serviceStatus ? `服务状态：${serviceStatus}` : "",
        expiresAt ? `到期：${expiresAt}` : "",
        nextBaseUrl ? `Base URL：${nextBaseUrl}` : ""
      ]
        .filter(Boolean)
        .join(" ");
      setNotice(detail);
      setLastAction({ kind: "success", title: "中转 API Key 已填入", detail });
      setGogoaisLogin((current) => ({ ...current, password: "" }));
    } catch (err) {
      const detail = gogoaisKeyErrorMessage(err);
      setError(detail);
      setLastAction({ kind: "error", title: "获取中转 API Key 失败", detail });
    } finally {
      setBusy("");
    }
  }

  async function resetAccountState() {
    setPendingReset(false);
    setError("");
    clearProbeReport();
    setNotice("正在停止 Codex、备份并重置账号状态...");
    setLastAction({
      kind: "info",
      title: "正在重置账号状态",
      detail: "正在停止 Codex、备份 auth.json/config.toml，并删除这两个 live 状态文件。"
    });
    setBusy("reset-account");
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中重置账号状态");
      }
      const result = await invoke<ClearCodexStateResult>("clear_codex_state");
      setState(result.app_state);
      setNotice(
        result.backup_dir
          ? `${result.message} 备份：${result.backup_dir}`
          : result.message
      );
      setLastAction({
        kind: result.removed.length > 0 ? "success" : "info",
        title: result.removed.length > 0 ? "重置完成" : "无需重置",
        detail: result.backup_dir
          ? `${result.message}；备份：${result.backup_dir}`
          : result.message
      });
    } catch (err) {
      setError(String(err));
      setLastAction({ kind: "error", title: "重置失败", detail: String(err) });
    } finally {
      setBusy("");
    }
  }

  return (
    <>
    <main className="app-shell">
      <section className="topbar">
        <div>
          <p className="eyebrow">Codex Account Switcher</p>
          <h1>Codex 切号器</h1>
          <p className="build-label">{appBuildLabel}</p>
        </div>
        <div className="topbar-actions">
          <button
            className="ghost topbar-action"
            onClick={detectCodexEnvironment}
            disabled={!!busy}
            title="检测 Codex 使用环境"
          >
            {busy === "detect-codex-env" ? <Loader2 className="spin" /> : <Activity />}
            检测 Codex 使用环境
          </button>
          <button className="ghost topbar-action" onClick={checkForUpdates} disabled={!!busy}>
            {busy === "check-update" ? <Loader2 className="spin" /> : <Rocket />}
            检查更新
          </button>
          <button className="ghost topbar-action" onClick={restartCodexApp} disabled={!!busy}>
            {busy === "restart-codex" ? <Loader2 className="spin" /> : <Power />}
            重启 Codex
          </button>
          <button className="danger text-danger topbar-action" onClick={quitCodexApp} disabled={!!busy}>
            {busy === "quit-codex" ? <Loader2 className="spin" /> : <Power />}
            关闭 Codex
          </button>
          <button className="icon-button" onClick={load} disabled={!!busy} title="刷新状态">
            {busy === "refresh" ? <Loader2 className="spin" /> : <RefreshCw />}
          </button>
        </div>
      </section>

      {error && <div className="error-banner">{error}</div>}
      {notice && systemProbe ? (
        <button
          className="notice-banner notice-toggle"
          type="button"
          onClick={() => setProbeExpanded((expanded) => !expanded)}
          aria-expanded={probeExpanded}
          title="展开/折叠检测结果"
        >
          <span>{notice}</span>
          <ChevronDown className={probeExpanded ? "expanded" : ""} />
        </button>
      ) : (
        notice && <div className="notice-banner">{notice}</div>
      )}

      {systemProbe && probeExpanded && (
        <section className="probe-panel probe-panel-global" role="status">
          <div className={systemProbe.codex_ready ? "probe-verdict ok" : "probe-verdict error"}>
            <strong>{systemProbe.codex_ready_title}</strong>
            <p>{systemProbe.codex_ready_detail}</p>
            <small>{systemProbe.summary}</small>
          </div>
          <div className="probe-list">
            {systemProbe.checks.map((check) => (
              <div className={`probe-item ${check.status}`} key={check.title}>
                <div className="probe-item-head">
                  <span>{check.title}</span>
                  <b>{probeStatusLabel[check.status]}</b>
                </div>
                <small>{check.requirement}</small>
                <p>{check.detail}</p>
                <em>{check.suggestion}</em>
              </div>
            ))}
          </div>
        </section>
      )}

      {updateState?.available && (
        <section className="update-panel" role="status">
          <div>
            <p className="eyebrow">Update Available</p>
            <h2>发现新版本 {updateState.version}</h2>
            <p>
              当前版本 {updateState.currentVersion ?? "未知"}。
              {updateState.date ? ` 发布于 ${new Date(updateState.date).toLocaleString()}。` : ""}
            </p>
            {updateState.body && <pre>{updateState.body}</pre>}
            {(downloadedBytes > 0 || updateState.progress) && (
              <div className="update-progress">
                <span>
                  {updateState.progress ??
                    `已下载 ${formatBytes(downloadedBytes)}${
                      downloadTotal ? ` / ${formatBytes(downloadTotal)}` : ""
                    }`}
                </span>
                {downloadTotal && (
                  <progress value={downloadedBytes} max={downloadTotal}>
                    {Math.round((downloadedBytes / downloadTotal) * 100)}%
                  </progress>
                )}
              </div>
            )}
          </div>
          <button className="primary update-install" onClick={installAvailableUpdate} disabled={!!busy || !availableUpdate}>
            {busy === "install-update" ? <Loader2 className="spin" /> : <Rocket />}
            安装并重启
          </button>
        </section>
      )}

      <section className="workspace">
        <aside className="side-panel">
          <div className="status-block">
            <div className="status-title">
              <ShieldCheck />
              <span>当前本机状态</span>
            </div>
            <div className="active-pill">
              {activeProfile ? activeProfile.name : "未匹配到已保存档案"}
            </div>
            <dl>
              <div>
                <dt>账号邮箱</dt>
                <dd>{state?.current.account_email ?? state?.current.account_name ?? "未识别"}</dd>
              </div>
              <div>
                <dt>订阅</dt>
                <dd>{planLabel(state?.current.account_plan)}</dd>
              </div>
              <div>
                <dt>授权方式</dt>
                <dd>{state?.current.auth_mode ?? "读取中"}</dd>
              </div>
              <div>
                <dt>模型</dt>
                <dd>{state?.current.model ?? "未设置"}</dd>
              </div>
              <div>
                <dt>Base URL</dt>
                <dd>{state?.current.base_url ?? "默认 OpenAI"}</dd>
              </div>
              <div>
                <dt>目录</dt>
                <dd className="path">{state?.current.codex_dir ?? "~/.codex"}</dd>
              </div>
              <div>
                <dt>聊天会话</dt>
                <dd>所有档案共享</dd>
              </div>
              <div>
                <dt>会话大小</dt>
                <dd>{formatBytes(state?.current.session_size)}</dd>
              </div>
            </dl>
            <div className="status-actions">
              <button className="danger text-danger reset-account" onClick={() => setPendingReset(true)} disabled={!!busy}>
                {busy === "reset-account" ? <Loader2 className="spin" /> : <Trash2 />}
                重置账号状态
              </button>
              <button className="ghost reset-account" onClick={openConfigToml} disabled={!!busy}>
                {busy === "open-config" ? <Loader2 className="spin" /> : <FileText />}
                打开 config.toml
              </button>
            </div>
            {lastAction && (
              <div className={`action-feedback ${lastAction.kind}`} role="status">
                <strong>{lastAction.title}</strong>
                <p>{lastAction.detail}</p>
              </div>
            )}
          </div>

          <div className="segmented">
            <button className={mode === "proxy" ? "selected" : ""} onClick={() => setMode("proxy")}>
              <KeyRound />
              新建中转
            </button>
            <button className={mode === "import" ? "selected" : ""} onClick={() => setMode("import")}>
              <UserRoundPlus />
              导入当前
            </button>
          </div>

          {mode === "import" ? (
            <form className="form-panel" onSubmit={submitImport}>
              <label>
                档案名称
                <input
                  value={importForm.name}
                  onChange={(event) => setImportForm({ ...importForm, name: event.target.value })}
                  placeholder="例如：主力 Plus 账号"
                />
              </label>
              <label>
                类型
                <select
                  value={importForm.kind}
                  onChange={(event) =>
                    setImportForm({ ...importForm, kind: event.target.value as ProfileKind })
                  }
                >
                  <option value="chat_gpt_login">Plus/Pro 登录</option>
                  <option value="proxy_api_key">中转 API Key</option>
                  <option value="custom">自定义</option>
                </select>
              </label>
              <label>
                备注
                <textarea
                  value={importForm.notes}
                  onChange={(event) => setImportForm({ ...importForm, notes: event.target.value })}
                  placeholder="用途、额度或账号标记"
                />
              </label>
              <button className="primary" disabled={!!busy}>
                {busy === "import" ? <Loader2 className="spin" /> : <Plus />}
                保存当前状态
              </button>
            </form>
          ) : (
            <form className="form-panel" onSubmit={submitProxy}>
              <label>
                档案名称
                <input
                  value={proxyForm.name}
                  onChange={(event) => setProxyForm({ ...proxyForm, name: event.target.value })}
                />
              </label>
              <label>
                Base URL
                <input
                  value={proxyForm.base_url}
                  onChange={(event) => setProxyForm({ ...proxyForm, base_url: event.target.value })}
                />
              </label>
              <label>
                认证方式
                <select
                  value={proxyForm.codex_system}
                  onChange={(event) =>
                    setProxyForm({ ...proxyForm, codex_system: event.target.value as CodexSystem })
                  }
                >
                  <option value="account">沿用 ChatGPT 登录态，API Key 兜底</option>
                  <option value="api">只用 API Key</option>
                </select>
              </label>
              <p className="field-hint">
                沿用登录态会保留当前 ChatGPT token，并把中转 Base URL 写进 config；只用 API Key 会写入 OPENAI_API_KEY。
              </p>
              <div className="key-fetch-panel">
                <div>
                  <strong>通过 gogoais 账号获取 API Key</strong>
                  <small>账号密码只用于本次请求，成功后自动填入下方 API Key 和 Base URL，不保存密码。</small>
                </div>
                <label>
                  账号
                  <input
                    value={gogoaisLogin.username}
                    onChange={(event) =>
                      setGogoaisLogin({ ...gogoaisLogin, username: event.target.value })
                    }
                    placeholder="邮箱或用户名"
                    autoComplete="username"
                  />
                </label>
                <label>
                  密码
                  <input
                    type="password"
                    value={gogoaisLogin.password}
                    onChange={(event) =>
                      setGogoaisLogin({ ...gogoaisLogin, password: event.target.value })
                    }
                    placeholder="gogoais 密码"
                    autoComplete="current-password"
                  />
                </label>
                <button
                  className="ghost fetch-key-button"
                  type="button"
                  onClick={fetchGogoaisCodexKey}
                  disabled={!!busy || !gogoaisLogin.username.trim() || !gogoaisLogin.password.trim()}
                >
                  {busy === "fetch-gogoais-key" ? <Loader2 className="spin" /> : <KeyRound />}
                  获取并填入
                </button>
              </div>
              <label>
                API Key{proxyForm.codex_system === "account" ? "（无登录态时兜底）" : ""}
                <input
                  type="password"
                  value={proxyForm.api_key}
                  onChange={(event) => setProxyForm({ ...proxyForm, api_key: event.target.value })}
                  placeholder={proxyForm.codex_system === "account" ? "可留空，优先使用当前账号登录态" : "sk-..."}
                />
              </label>
              <div className="two-col">
                <label>
                  模型
                  <input
                    value={proxyForm.model}
                    onChange={(event) => setProxyForm({ ...proxyForm, model: event.target.value })}
                  />
                </label>
                <label>
                  Review 模型
                  <input
                    value={proxyForm.review_model}
                    onChange={(event) => setProxyForm({ ...proxyForm, review_model: event.target.value })}
                  />
                </label>
              </div>
              <label>
                推理强度
                <select
                  value={proxyForm.reasoning_effort}
                  onChange={(event) => setProxyForm({ ...proxyForm, reasoning_effort: event.target.value })}
                >
                  <option value="minimal">minimal</option>
                  <option value="low">low</option>
                  <option value="medium">medium</option>
                  <option value="high">high</option>
                  <option value="xhigh">xhigh</option>
                </select>
              </label>
              <button className="primary" disabled={!!busy}>
                {busy === "proxy" ? <Loader2 className="spin" /> : <Zap />}
                保存中转档案
              </button>
            </form>
          )}
        </aside>

        <section className="profiles">
          <div className="section-head">
            <div>
              <p className="eyebrow">Profiles</p>
              <h2>账号档案</h2>
            </div>
            <span>{state?.profiles.length ?? 0} 个</span>
          </div>

          <div className="profile-grid">
            {state?.profiles.map((profile) => (
              <article className={profile.is_active ? "profile-card active" : "profile-card"} key={profile.id}>
                <div className="profile-main">
                  <div className="profile-icon">
                    {profile.kind === "proxy_api_key" ? <KeyRound /> : <FileKey2 />}
                  </div>
                  <div>
                    <h3>{profile.name}</h3>
                    <p>{kindLabel[profile.kind]} · {accountLabel(profile)}</p>
                  </div>
                  {profile.is_active && (
                    <span className="live-mark">
                      <CheckCircle2 />
                      已启用
                    </span>
                  )}
                </div>
                <dl className="meta-grid">
                  <div>
                    <dt>账号</dt>
                    <dd>{accountLabel(profile)}</dd>
                  </div>
                  <div>
                    <dt>订阅</dt>
                    <dd>{planLabel(profile.account_plan)}</dd>
                  </div>
                  <div>
                    <dt>模型</dt>
                    <dd>{profile.model ?? "未设置"}</dd>
                  </div>
                  <div>
                    <dt>Base URL</dt>
                    <dd>{profile.base_url ?? "默认 OpenAI"}</dd>
                  </div>
                  <div>
                    <dt>体系</dt>
                    <dd>{systemLabel[profile.codex_system]}</dd>
                  </div>
                  <div>
                    <dt>config</dt>
                    <dd>{profile.config_hash ?? "无"}</dd>
                  </div>
                  <div>
                    <dt>auth</dt>
                    <dd>{profile.auth_hash ?? "无"}</dd>
                  </div>
                </dl>
                {profile.notes && <p className="notes">{profile.notes}</p>}
                <div className="card-actions">
                  <span>
                    <Clock3 />
                    {new Date(profile.updated_at).toLocaleString()}
                  </span>
                  <div>
                    <button
                      className="ghost"
                      onClick={() => switchTo(profile)}
                      disabled={!!busy || profile.is_active}
                    >
                      {busy === `switch-${profile.id}` ? <Loader2 className="spin" /> : <Zap />}
                      切换
                    </button>
                    <button className="danger" onClick={() => remove(profile)} disabled={!!busy} title="删除档案">
                      <Trash2 />
                    </button>
                  </div>
                </div>
              </article>
            ))}
            {state && state.profiles.length === 0 && (
              <div className="empty-state">
                <FileKey2 />
                <h3>还没有账号档案</h3>
                <p>先导入当前 Plus/Pro 登录状态，或创建一个中转 API Key 档案。</p>
              </div>
            )}
          </div>
        </section>
      </section>
    </main>
    {pendingSwitch && (
      <div className="modal-backdrop" role="presentation">
        <section className="switch-modal" role="dialog" aria-modal="true" aria-labelledby="switch-title">
          <div className="modal-icon">
            <Zap />
          </div>
          <div>
            <p className="eyebrow">Switch Profile</p>
            <h2 id="switch-title">确认切换账号</h2>
          </div>
          <dl className="confirm-grid">
            <div>
              <dt>目标档案</dt>
              <dd>{pendingSwitch.name}</dd>
            </div>
            <div>
              <dt>账号</dt>
              <dd>{accountLabel(pendingSwitch)}</dd>
            </div>
            <div>
              <dt>类型</dt>
              <dd>{kindLabel[pendingSwitch.kind]}</dd>
            </div>
            <div>
              <dt>体系</dt>
              <dd>{systemLabel[pendingSwitch.codex_system]}</dd>
            </div>
            <div>
              <dt>订阅</dt>
              <dd>{planLabel(pendingSwitch.account_plan)}</dd>
            </div>
          </dl>
          <div className="switch-note">
            只切换 auth.json 和 config.toml；所有账号/API 档案继续共用同一套 Codex thread。
          </div>
          <label className="check-row">
            <input
              type="checkbox"
              checked={restartCodex}
              onChange={(event) => setRestartCodex(event.target.checked)}
            />
            <span>
              <strong>切换后重启 Codex app</strong>
              <small>macOS 使用 open -a Codex，Windows 使用 taskkill/PowerShell，Linux 尝试桌面入口或 codex 命令。</small>
            </span>
          </label>
          <div className="modal-actions">
            <button className="ghost" onClick={() => setPendingSwitch(null)} disabled={!!busy}>
              取消
            </button>
            <button className="primary modal-primary" onClick={confirmSwitch} disabled={!!busy}>
              {busy === `switch-${pendingSwitch.id}` ? <Loader2 className="spin" /> : <Power />}
              确认切换
            </button>
          </div>
        </section>
      </div>
    )}
    {pendingReset && (
      <div className="modal-backdrop" role="presentation">
        <section className="switch-modal delete-modal" role="dialog" aria-modal="true" aria-labelledby="delete-title">
          <div className="modal-icon danger-icon">
            <Trash2 />
          </div>
          <div>
            <p className="eyebrow">Reset Account State</p>
            <h2 id="delete-title">确认重置账号状态</h2>
          </div>
          <div className="delete-warning">
            <strong>这个操作会关闭当前正在运行的 Codex app。</strong>
            <p>
              切号器会先备份 live 的 auth.json 和 config.toml，然后删除这两个文件，
              最后刷新当前状态。下次打开 Codex 时需要重新登录或重新配置 API Key。
            </p>
          </div>
          <dl className="confirm-grid">
            <div>
              <dt>将删除</dt>
              <dd>auth.json 和 config.toml</dd>
            </div>
            <div>
              <dt>Codex app</dt>
              <dd>删除前会关闭</dd>
            </div>
            <div>
              <dt>备份</dt>
              <dd>先备份 auth.json/config.toml</dd>
            </div>
            <div>
              <dt>影响范围</dt>
              <dd>只处理当前 live ~/.codex 状态</dd>
            </div>
          </dl>
          <div className="modal-actions">
            <button className="ghost" onClick={() => setPendingReset(false)} disabled={!!busy}>
              取消
            </button>
            <button
              className="danger text-danger modal-primary"
              onClick={resetAccountState}
              disabled={!!busy}
            >
              {busy === "reset-account" ? <Loader2 className="spin" /> : <Trash2 />}
              确认重置
            </button>
          </div>
        </section>
      </div>
    )}
    </>
  );
}

export default App;

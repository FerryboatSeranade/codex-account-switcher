import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import {
  Activity,
  AlertTriangle,
  ChevronDown,
  CheckCircle2,
  Clock3,
  Code2,
  Download,
  ExternalLink,
  FileText,
  FileKey2,
  Globe2,
  KeyRound,
  Loader2,
  Plus,
  Power,
  RefreshCw,
  Rocket,
  Settings2,
  ShieldCheck,
  Terminal,
  Trash2,
  UserRoundPlus,
  Zap
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useRef, useState } from "react";

type ProfileKind = "chat_gpt_login" | "proxy_api_key" | "custom";
type CodexSystem = "account" | "api";
type ClientPreference = "codex_app" | "vscode_extension" | "cli_other";

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
  client_preference: ClientPreference;
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

type ClientPreferenceResult = {
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

type InstallProgressStatus = "started" | "running" | "ok" | "warning" | "error" | "finished";

type InstallProgressEntry = {
  run_id: string;
  order: number;
  status: InstallProgressStatus;
  step: string;
  title: string;
  detail: string;
  timestamp: string;
};

type ActionFeedback = {
  kind: "success" | "info" | "error";
  title: string;
  detail: string;
  action?: "restart-admin";
};

type UpdateState = {
  available: boolean;
  currentVersion?: string;
  version?: string;
  date?: string;
  body?: string;
  progress?: string;
};

type HostsEntry = {
  line_number: number;
  ip: string;
  names: string[];
  managed: boolean;
  comment?: string;
};

type HostsState = {
  path: string;
  exists: boolean;
  entries: HostsEntry[];
  managed_entries: HostsEntry[];
};

type HostsWriteResult = {
  message: string;
  backup_dir?: string;
  dns_flush_message?: string;
  hosts_state: HostsState;
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

type HostsForm = {
  ip: string;
  hostname: string;
  aliases: string;
  comment: string;
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
  codex_system: "api"
};

const defaultHostsForm: HostsForm = {
  ip: "127.0.0.1",
  hostname: "",
  aliases: "",
  comment: ""
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

const clientPreferenceMeta: Record<
  ClientPreference,
  {
    label: string;
    detail: string;
    refreshHint: string;
    topbarAction: string;
    resetWarning: string;
  }
> = {
  codex_app: {
    label: "Codex App",
    detail: "切换后可自动关闭并重新打开桌面 Codex。",
    refreshHint: "请重启 Codex App，让新的 auth.json/config.toml 生效。",
    topbarAction: "重启 Codex",
    resetWarning: "这个操作会关闭当前正在运行的 Codex App。"
  },
  vscode_extension: {
    label: "VS Code 扩展",
    detail: "只写入 ~/.codex，不自动关闭 VS Code。",
    refreshHint: "请在 VS Code 中执行 Reload Window，或重启 VS Code，让 Codex 扩展重新读取 ~/.codex。",
    topbarAction: "VS Code 提示",
    resetWarning: "这个操作不会关闭 VS Code，只会备份并修改 ~/.codex 状态文件。"
  },
  cli_other: {
    label: "CLI/其他",
    detail: "只写入 ~/.codex，由用户重启终端或相关进程。",
    refreshHint: "请重启当前终端里的 Codex CLI/相关进程，让它重新读取 ~/.codex。",
    topbarAction: "刷新提示",
    resetWarning: "这个操作不会关闭其他客户端，只会备份并修改 ~/.codex 状态文件。"
  }
};

const clientPreferenceOptions: ClientPreference[] = ["codex_app", "vscode_extension", "cli_other"];

const probeStatusLabel: Record<SystemProbeStatus, string> = {
  ok: "正常",
  warning: "提醒",
  error: "失败"
};

const installProgressStatusLabel: Record<InstallProgressStatus, string> = {
  started: "开始",
  running: "进行中",
  ok: "完成",
  warning: "需确认",
  error: "失败",
  finished: "结束"
};

const appBuildLabel = "v0.1.17-install-progress";
const AUTO_UPDATE_INTERVAL_MS = 24 * 60 * 60 * 1000;
const AUTO_UPDATE_LAST_CHECK_KEY = "codex-account-switcher:last-auto-update-check";

function readLastAutoUpdateCheck() {
  try {
    const raw = window.localStorage.getItem(AUTO_UPDATE_LAST_CHECK_KEY);
    const timestamp = raw ? Number(raw) : 0;
    return Number.isFinite(timestamp) ? timestamp : 0;
  } catch {
    return 0;
  }
}

function rememberAutoUpdateCheck() {
  try {
    window.localStorage.setItem(AUTO_UPDATE_LAST_CHECK_KEY, String(Date.now()));
  } catch {
    // Losing this timestamp only means the next launch may check again.
  }
}

function shouldRunAutoUpdateCheck() {
  const lastCheck = readLastAutoUpdateCheck();
  if (!lastCheck) {
    return true;
  }
  const elapsed = Date.now() - lastCheck;
  return elapsed < 0 || elapsed >= AUTO_UPDATE_INTERVAL_MS;
}

function autoUpdateCheckDelay() {
  const lastCheck = readLastAutoUpdateCheck();
  if (!lastCheck) {
    return 3000;
  }
  const elapsed = Date.now() - lastCheck;
  if (elapsed < 0 || elapsed >= AUTO_UPDATE_INTERVAL_MS) {
    return 3000;
  }
  return AUTO_UPDATE_INTERVAL_MS - elapsed;
}

function needsAdminRestart(detail: string) {
  return (
    detail.includes("以管理员身份重启切号器") ||
    detail.includes("Windows 拒绝当前切号器") ||
    detail.includes("Windows 拒绝写入 hosts") ||
    detail.includes("拒绝访问")
  );
}

function formatSystemProbeReport(report: SystemProbeReport) {
  const reportTitle = report.codex_ready_title.includes("安装")
    ? "Codex 安装/检测报告"
    : "Codex 使用环境检测报告";
  const lines = [
    reportTitle,
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

function formatHostsEntry(entry: HostsEntry) {
  return `${entry.ip} ${entry.names.join(" ")}`;
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
    client_preference: "codex_app",
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
  const [proxyTab, setProxyTab] = useState<"login" | "config">("login");
  const [gogoaisLoggedIn, setGogoaisLoggedIn] = useState(false);
  const [pendingSwitch, setPendingSwitch] = useState<ProfileSummary | null>(null);
  const [pendingReset, setPendingReset] = useState(false);
  const [restartCodex, setRestartCodex] = useState(true);
  const [systemProbe, setSystemProbe] = useState<SystemProbeReport | null>(null);
  const [probeExpanded, setProbeExpanded] = useState(true);
  const [installProgress, setInstallProgress] = useState<InstallProgressEntry[]>([]);
  const [installProgressExpanded, setInstallProgressExpanded] = useState(true);
  const [availableUpdate, setAvailableUpdate] = useState<Update | null>(null);
  const [updateState, setUpdateState] = useState<UpdateState | null>(null);
  const [downloadedBytes, setDownloadedBytes] = useState(0);
  const [downloadTotal, setDownloadTotal] = useState<number | null>(null);
  const [hostsState, setHostsState] = useState<HostsState | null>(null);
  const [hostsExpanded, setHostsExpanded] = useState(false);
  const autoUpdateCheckInFlight = useRef(false);
  const [importForm, setImportForm] = useState<ImportForm>({
    name: "我的 Plus/Pro 账号",
    kind: "chat_gpt_login",
    notes: ""
  });
  const [proxyForm, setProxyForm] = useState<ProxyForm>(defaultProxyForm);
  const [hostsForm, setHostsForm] = useState<HostsForm>(defaultHostsForm);
  const [gogoaisLogin, setGogoaisLogin] = useState<GogoaisLoginForm>({
    username: "",
    password: ""
  });

  const activeProfile = useMemo(
    () => state?.profiles.find((profile) => profile.is_active),
    [state]
  );
  const clientPreference = state?.client_preference ?? "codex_app";
  const clientPreferenceInfo = clientPreferenceMeta[clientPreference];
  const manageCodexApp = clientPreference === "codex_app";
  const activeInstallProgress = installProgress.length > 0;
  const latestInstallProgress = activeInstallProgress
    ? installProgress[installProgress.length - 1]
    : null;

  function clearProbeReport() {
    setSystemProbe(null);
    setProbeExpanded(true);
  }

  function clearInstallProgress() {
    setInstallProgress([]);
    setInstallProgressExpanded(true);
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
      try {
        setHostsState(await invoke<HostsState>("get_hosts_state"));
      } catch (hostsErr) {
        setHostsState(null);
        setLastAction({
          kind: "error",
          title: "hosts 状态读取失败",
          detail: String(hostsErr)
        });
      }
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
    if (!isTauriRuntime()) {
      return;
    }

    let disposed = false;
    let unlisten: (() => void) | undefined;

    listen<InstallProgressEntry>("install_codex_progress", (event) => {
      if (disposed) {
        return;
      }
      const next = event.payload;
      setInstallProgress((current) => {
        const sameRun = current.length === 0 || current[current.length - 1].run_id === next.run_id;
        const base = sameRun ? current : [];
        return [...base, next].sort((left, right) => left.order - right.order);
      });
      setInstallProgressExpanded(true);
    })
      .then((dispose) => {
        if (disposed) {
          dispose();
        } else {
          unlisten = dispose;
        }
      })
      .catch(() => {
        // Progress is best-effort; the final report still contains the authoritative result.
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!isTauriRuntime()) {
      return;
    }

    let disposed = false;
    let timer: number | undefined;

    const scheduleNextCheck = (delay: number) => {
      timer = window.setTimeout(async () => {
        if (disposed) {
          return;
        }
        if (shouldRunAutoUpdateCheck()) {
          await runUpdateCheck({ automatic: true });
        }
        if (!disposed) {
          scheduleNextCheck(autoUpdateCheckDelay());
        }
      }, delay);
    };

    scheduleNextCheck(autoUpdateCheckDelay());

    return () => {
      disposed = true;
      if (timer !== undefined) {
        window.clearTimeout(timer);
      }
    };
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
    if (proxyTab === "login") {
      fetchGogoaisCodexKey();
      return;
    }
    runAction("proxy", () =>
      invoke<AppState>("create_proxy_profile", { input: proxyForm })
    ).then(() => {
      setProxyForm({ ...defaultProxyForm, api_key: "" });
      setGogoaisLoggedIn(false);
      setProxyTab("login");
    });
  }

  function switchTo(profile: ProfileSummary) {
    setPendingSwitch(profile);
    setRestartCodex(manageCodexApp);
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
          input: { id: profile.id, restart_codex_app: manageCodexApp }
        });
        setState(result.app_state);
        message = `${profile.name} 已切换。${result.message}`;
      } else {
        const nextState = await invoke<AppState>("switch_profile", {
          input: { id: profile.id }
        });
        setState(nextState);
        message = `${profile.name} 已切换。${clientPreferenceInfo.refreshHint}`;
      }
      setPendingSwitch(null);
      setNotice(message);
      setLastAction({ kind: "success", title: "切换完成", detail: message });
    } catch (err) {
      const detail = String(err);
      setError(detail);
      setLastAction({
        kind: "error",
        title: "切换失败",
        detail,
        action: needsAdminRestart(detail) ? "restart-admin" : undefined
      });
    } finally {
      setBusy("");
    }
  }

  async function saveClientPreference(preference: ClientPreference) {
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    setBusy("client-preference");
    try {
      if (!isTauriRuntime()) {
        setState((current) =>
          current ? { ...current, client_preference: preference } : current
        );
        return;
      }
      const result = await invoke<ClientPreferenceResult>("set_client_preference", {
        input: { preference }
      });
      setState(result.app_state);
      setNotice(result.message);
      setLastAction({ kind: "success", title: "目标客户端已更新", detail: result.message });
    } catch (err) {
      const detail = String(err);
      setError(detail);
      setLastAction({ kind: "error", title: "保存目标客户端失败", detail });
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
    if (!manageCodexApp) {
      const detail = clientPreferenceInfo.refreshHint;
      setNotice(detail);
      setLastAction({ kind: "info", title: clientPreferenceInfo.topbarAction, detail });
      return;
    }
    setBusy("restart-codex");
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中重启 Codex app");
      }
      const message = await invoke<string>("restart_codex_app");
      setNotice(message);
      setLastAction({ kind: "success", title: "已重启 Codex", detail: message });
    } catch (err) {
      const detail = String(err);
      setError(detail);
      setLastAction({
        kind: "error",
        title: "重启失败",
        detail,
        action: needsAdminRestart(detail) ? "restart-admin" : undefined
      });
    } finally {
      setBusy("");
    }
  }

  async function quitCodexApp() {
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    if (!manageCodexApp) {
      const detail = "当前目标客户端不是 Codex App，切号器不会关闭 VS Code 或终端。";
      setNotice(detail);
      setLastAction({ kind: "info", title: "无需关闭 Codex App", detail });
      return;
    }
    setBusy("quit-codex");
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中关闭 Codex app");
      }
      const message = await invoke<string>("quit_codex_app");
      setNotice(message);
      setLastAction({ kind: "success", title: "已关闭 Codex", detail: message });
    } catch (err) {
      const detail = String(err);
      setError(detail);
      setLastAction({
        kind: "error",
        title: "关闭失败",
        detail,
        action: needsAdminRestart(detail) ? "restart-admin" : undefined
      });
    } finally {
      setBusy("");
    }
  }

  async function restartSwitcherAsAdmin() {
    setError("");
    setNotice("正在请求 Windows 管理员权限...");
    setBusy("restart-admin");
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中以管理员身份重启切号器");
      }
      await invoke<string>("restart_switcher_as_admin");
      setLastAction({
        kind: "info",
        title: "等待管理员确认",
        detail: "如果 Windows 弹出 UAC，请确认后在管理员窗口里重试关闭 Codex 或重置账号状态。"
      });
    } catch (err) {
      const detail = String(err);
      setError(detail);
      setLastAction({ kind: "error", title: "管理员重启失败", detail });
    } finally {
      setBusy("");
    }
  }

  async function openCodexFile(name: "config.toml" | "auth.json") {
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    const action = `open-${name}`;
    setBusy(action);
    try {
      if (!isTauriRuntime()) {
        throw new Error(`请在 Tauri 桌面窗口中打开 ${name}`);
      }
      const message = await invoke<string>("open_codex_file", { name });
      setNotice(message);
      setLastAction({ kind: "success", title: `已打开 ${name}`, detail: message });
    } catch (err) {
      setError(String(err));
      setLastAction({ kind: "error", title: "打开失败", detail: String(err) });
    } finally {
      setBusy("");
    }
  }

  async function refreshHostsState() {
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    setBusy("hosts-refresh");
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中读取 hosts");
      }
      const result = await invoke<HostsState>("get_hosts_state");
      setHostsState(result);
      const detail = `已读取 hosts：${result.path}`;
      setNotice(detail);
      setLastAction({ kind: "success", title: "hosts 已刷新", detail });
    } catch (err) {
      const detail = String(err);
      setError(detail);
      setLastAction({ kind: "error", title: "读取 hosts 失败", detail });
    } finally {
      setBusy("");
    }
  }

  async function openHostsFile() {
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    setBusy("open-hosts");
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中打开 hosts");
      }
      const message = await invoke<string>("open_hosts_file");
      setNotice(message);
      setLastAction({ kind: "success", title: "已打开 hosts", detail: message });
    } catch (err) {
      const detail = String(err);
      setError(detail);
      setLastAction({ kind: "error", title: "打开 hosts 失败", detail });
    } finally {
      setBusy("");
    }
  }

  function applyHostsResult(result: HostsWriteResult, successTitle: string) {
    setHostsState(result.hosts_state);
    const detail = [
      result.message,
      result.backup_dir ? `备份：${result.backup_dir}` : "",
      result.dns_flush_message ?? ""
    ]
      .filter(Boolean)
      .join(" ");
    setNotice(detail);
    setLastAction({ kind: "success", title: successTitle, detail });
  }

  async function submitHostsMapping(event: FormEvent) {
    event.preventDefault();
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    setBusy("hosts-save");
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中写入 hosts");
      }
      const result = await invoke<HostsWriteResult>("upsert_hosts_mapping", {
        input: hostsForm
      });
      applyHostsResult(result, "hosts 映射已保存");
      setHostsExpanded(true);
      setHostsForm((current) => ({
        ...current,
        hostname: "",
        aliases: "",
        comment: ""
      }));
    } catch (err) {
      const detail = String(err);
      setError(detail);
      setLastAction({
        kind: "error",
        title: "写入 hosts 失败",
        detail,
        action: needsAdminRestart(detail) ? "restart-admin" : undefined
      });
    } finally {
      setBusy("");
    }
  }

  async function deleteHostsEntry(hostname: string) {
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    setBusy(`hosts-delete-${hostname}`);
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中删除 hosts 映射");
      }
      const result = await invoke<HostsWriteResult>("delete_hosts_mapping", { hostname });
      applyHostsResult(result, "hosts 映射已删除");
    } catch (err) {
      const detail = String(err);
      setError(detail);
      setLastAction({
        kind: "error",
        title: "删除 hosts 映射失败",
        detail,
        action: needsAdminRestart(detail) ? "restart-admin" : undefined
      });
    } finally {
      setBusy("");
    }
  }

  async function detectCodexEnvironment() {
    setError("");
    setNotice("");
    setLastAction(null);
    clearProbeReport();
    clearInstallProgress();
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

  async function installCodexEnvironment() {
    setError("");
    setNotice("正在检测并安装 Codex 组件，这可能需要几分钟...");
    setLastAction(null);
    clearProbeReport();
    clearInstallProgress();
    setBusy("install-codex-env");
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中安装 Codex");
      }
      const report = await invoke<SystemProbeReport>("install_codex_environment");
      setSystemProbe(report);
      setProbeExpanded(true);
      try {
        const clipboardMessage = await invoke<string>("copy_text_to_clipboard", {
          text: formatSystemProbeReport(report)
        });
        const detail = `${report.summary} ${clipboardMessage}。`;
        setNotice(detail);
        setLastAction({
          kind: report.codex_ready ? "success" : "info",
          title: report.codex_ready ? "Codex 已就绪" : "Codex 安装已执行",
          detail
        });
      } catch (clipboardErr) {
        const detail = `${report.summary} 但复制到剪贴板失败：${String(clipboardErr)}`;
        setNotice(report.summary);
        setLastAction({
          kind: report.codex_ready ? "success" : "info",
          title: report.codex_ready ? "Codex 已就绪" : "Codex 安装已执行",
          detail
        });
      }
    } catch (err) {
      const detail = String(err);
      setError(detail);
      setLastAction({
        kind: "error",
        title: "Codex 安装失败",
        detail,
        action: needsAdminRestart(detail) ? "restart-admin" : undefined
      });
    } finally {
      setBusy("");
    }
  }

  async function runUpdateCheck({ automatic = false } = {}) {
    if (automatic && autoUpdateCheckInFlight.current) {
      return;
    }
    if (automatic) {
      autoUpdateCheckInFlight.current = true;
    } else {
      setError("");
      setNotice("");
      setLastAction(null);
      clearProbeReport();
      setAvailableUpdate(null);
      setUpdateState(null);
      setDownloadedBytes(0);
      setDownloadTotal(null);
      setBusy("check-update");
    }
    try {
      if (!isTauriRuntime()) {
        throw new Error("请在 Tauri 桌面窗口中检查更新");
      }
      const update = await check();
      if (automatic) {
        rememberAutoUpdateCheck();
      }
      if (!update) {
        if (automatic) {
          return;
        }
        const detail = "当前已经是最新版本。";
        setNotice(detail);
        setLastAction({ kind: "info", title: "没有可用更新", detail });
        return;
      }
      setDownloadedBytes(0);
      setDownloadTotal(null);
      setAvailableUpdate(update);
      setUpdateState({
        available: true,
        currentVersion: update.currentVersion,
        version: update.version,
        date: update.date,
        body: update.body
      });
      const detail = automatic
        ? `自动检测发现新版本 ${update.version}，当前版本 ${update.currentVersion}。`
        : `发现新版本 ${update.version}，当前版本 ${update.currentVersion}。`;
      setNotice(detail);
      setLastAction({ kind: "success", title: automatic ? "自动发现新版本" : "发现新版本", detail });
    } catch (err) {
      if (automatic) {
        rememberAutoUpdateCheck();
      }
      const detail = updateErrorMessage(err);
      if (automatic) {
        setLastAction({ kind: "error", title: "自动检查更新失败", detail });
      } else {
        setError(detail);
        setLastAction({ kind: "error", title: "检查更新失败", detail });
      }
    } finally {
      if (automatic) {
        autoUpdateCheckInFlight.current = false;
      } else {
        setBusy("");
      }
    }
  }

  async function checkForUpdates() {
    await runUpdateCheck();
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
        codex_system: "api"
      }));
      const expiresAt = formatBeijingDateTime(result.expires_at);
      const serviceStatus = serviceStatusLabel(result.service_status);
      const detail = [
        "已获取并填入 API Key，认证方式已切到只用 API Key。",
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
      setGogoaisLoggedIn(true);
      setProxyTab("config");
    } catch (err) {
      const detail = gogoaisKeyErrorMessage(err);
      setError(detail);
      setLastAction({ kind: "error", title: "获取中转 API Key 失败", detail });
      setGogoaisLoggedIn(false);
    } finally {
      setBusy("");
    }
  }

  async function resetAccountState() {
    setPendingReset(false);
    setError("");
    clearProbeReport();
    setNotice(
      manageCodexApp
        ? "正在停止 Codex App、备份并重置账号状态..."
        : "正在备份并重置账号状态..."
    );
    setLastAction({
      kind: "info",
      title: "正在重置账号状态",
      detail: manageCodexApp
        ? "正在停止 Codex App、备份 auth.json/config.toml，并删除这两个 live 状态文件。"
        : "正在备份 auth.json/config.toml，并删除这两个 live 状态文件。"
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
      const detail = String(err);
      setError(detail);
      setLastAction({
        kind: "error",
        title: "重置失败",
        detail,
        action: needsAdminRestart(detail) ? "restart-admin" : undefined
      });
    } finally {
      setBusy("");
    }
  }

  return (
    <>
    <main className="app-shell">
      <section className="topbar">
        <div>
          <p className="eyebrow">Profile Switcher</p>
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
          <button
            className="ghost topbar-action"
            onClick={installCodexEnvironment}
            disabled={!!busy}
            title="检测并安装 Node.js、Codex CLI 和 Codex App"
          >
            {busy === "install-codex-env" ? <Loader2 className="spin" /> : <Download />}
            安装 Codex
          </button>
          <button className="ghost topbar-action" onClick={checkForUpdates} disabled={!!busy}>
            {busy === "check-update" ? <Loader2 className="spin" /> : <Rocket />}
            检查更新
          </button>
          <button className="ghost topbar-action" onClick={restartCodexApp} disabled={!!busy}>
            {busy === "restart-codex" ? <Loader2 className="spin" /> : <Power />}
            {clientPreferenceInfo.topbarAction}
          </button>
          <button
            className="danger text-danger topbar-action"
            onClick={quitCodexApp}
            disabled={!!busy || !manageCodexApp}
            title={manageCodexApp ? "关闭 Codex App" : "当前目标客户端不是 Codex App"}
          >
            {busy === "quit-codex" ? <Loader2 className="spin" /> : <Power />}
            关闭 Codex App
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

      {activeInstallProgress && latestInstallProgress && (
        <section className="install-progress-panel" role="status">
          <button
            className="install-progress-head"
            type="button"
            onClick={() => setInstallProgressExpanded((expanded) => !expanded)}
            aria-expanded={installProgressExpanded}
            title="展开/折叠安装进度"
          >
            <div>
              <p className="eyebrow">Install Progress</p>
              <h2>安装进度</h2>
              <span>{latestInstallProgress.title}：{latestInstallProgress.detail}</span>
            </div>
            <div className="install-progress-head-meta">
              {busy === "install-codex-env" ? <Loader2 className="spin" /> : <CheckCircle2 />}
              <ChevronDown className={installProgressExpanded ? "expanded" : ""} />
            </div>
          </button>
          {installProgressExpanded && (
            <ol className="install-progress-list">
              {installProgress.map((entry, index) => (
                <li className={`install-progress-item ${entry.status}`} key={`${entry.run_id}-${entry.order}-${index}`}>
                  <div className="install-progress-icon">
                    {entry.status === "running" ? (
                      <Loader2 className="spin" />
                    ) : entry.status === "warning" || entry.status === "error" ? (
                      <AlertTriangle />
                    ) : entry.status === "started" ? (
                      <Clock3 />
                    ) : (
                      <CheckCircle2 />
                    )}
                  </div>
                  <div className="install-progress-content">
                    <div className="install-progress-title">
                      <strong>{entry.title}</strong>
                      <span>{installProgressStatusLabel[entry.status]}</span>
                      <time>{new Date(entry.timestamp).toLocaleTimeString()}</time>
                    </div>
                    <p>{entry.detail}</p>
                  </div>
                </li>
              ))}
            </ol>
          )}
        </section>
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
            <div className="client-preference-block">
              <div className="status-title">
                {clientPreference === "vscode_extension" ? (
                  <Code2 />
                ) : clientPreference === "cli_other" ? (
                  <Terminal />
                ) : (
                  <Power />
                )}
                <span>目标客户端</span>
              </div>
              <div className="client-options" role="radiogroup" aria-label="目标客户端偏好">
                {clientPreferenceOptions.map((preference) => {
                  const meta = clientPreferenceMeta[preference];
                  const selected = clientPreference === preference;
                  return (
                    <button
                      className={selected ? "client-option selected" : "client-option"}
                      key={preference}
                      type="button"
                      role="radio"
                      aria-checked={selected}
                      onClick={() => saveClientPreference(preference)}
                      disabled={!!busy || selected}
                    >
                      <strong>{meta.label}</strong>
                      <small>{meta.detail}</small>
                    </button>
                  );
                })}
              </div>
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
                <dt>客户端偏好</dt>
                <dd>{clientPreferenceInfo.label}</dd>
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
              <button className="ghost reset-account" onClick={() => openCodexFile("config.toml")} disabled={!!busy}>
                {busy === "open-config.toml" ? <Loader2 className="spin" /> : <FileText />}
                打开 config.toml
              </button>
              <button className="ghost reset-account" onClick={() => openCodexFile("auth.json")} disabled={!!busy}>
                {busy === "open-auth.json" ? <Loader2 className="spin" /> : <FileKey2 />}
                打开 auth.json
              </button>
            </div>
            {lastAction && (
              <div className={`action-feedback ${lastAction.kind}`} role="status">
                <strong>{lastAction.title}</strong>
                <p>{lastAction.detail}</p>
                {lastAction.action === "restart-admin" && (
                  <button
                    className="primary action-feedback-button"
                    type="button"
                    onClick={restartSwitcherAsAdmin}
                    disabled={!!busy}
                  >
                    {busy === "restart-admin" ? <Loader2 className="spin" /> : <ShieldCheck />}
                    以管理员身份重启切号器
                  </button>
                )}
              </div>
            )}
          </div>

          <div className="hosts-block">
            <div className="status-title">
              <Globe2 />
              <span>本地 DNS hosts</span>
            </div>
            <button
              className="hosts-summary"
              type="button"
              onClick={() => setHostsExpanded((expanded) => !expanded)}
              aria-expanded={hostsExpanded}
              title="展开/折叠 hosts 映射"
            >
              <span>{hostsState?.managed_entries.length ?? 0} 条映射</span>
              <ChevronDown className={hostsExpanded ? "expanded" : ""} />
            </button>
            {hostsExpanded && (
              <div className="hosts-content">
                <div className="hosts-path-row">
                  <span className="path">{hostsState?.path ?? "读取中"}</span>
                  <button className="ghost mini-button" type="button" onClick={openHostsFile} disabled={!!busy}>
                    {busy === "open-hosts" ? <Loader2 className="spin" /> : <ExternalLink />}
                    打开
                  </button>
                  <button className="ghost mini-button" type="button" onClick={refreshHostsState} disabled={!!busy}>
                    {busy === "hosts-refresh" ? <Loader2 className="spin" /> : <RefreshCw />}
                    刷新
                  </button>
                </div>
                <form className="hosts-form" onSubmit={submitHostsMapping}>
                  <div className="two-col">
                    <label>
                      IP
                      <input
                        value={hostsForm.ip}
                        onChange={(event) => setHostsForm({ ...hostsForm, ip: event.target.value })}
                        placeholder="127.0.0.1"
                      />
                    </label>
                    <label>
                      域名
                      <input
                        value={hostsForm.hostname}
                        onChange={(event) =>
                          setHostsForm({ ...hostsForm, hostname: event.target.value })
                        }
                        placeholder="example.local"
                      />
                    </label>
                  </div>
                  <label>
                    别名
                    <input
                      value={hostsForm.aliases}
                      onChange={(event) => setHostsForm({ ...hostsForm, aliases: event.target.value })}
                      placeholder="api.example.local cdn.example.local"
                    />
                  </label>
                  <label>
                    备注
                    <input
                      value={hostsForm.comment}
                      onChange={(event) => setHostsForm({ ...hostsForm, comment: event.target.value })}
                      placeholder="本地调试"
                    />
                  </label>
                  <button
                    className="primary"
                    disabled={!!busy || !hostsForm.ip.trim() || !hostsForm.hostname.trim()}
                  >
                    {busy === "hosts-save" ? <Loader2 className="spin" /> : <Globe2 />}
                    保存 hosts 映射
                  </button>
                </form>
                <div className="hosts-list">
                  {hostsState?.managed_entries.map((entry) => (
                    <div className="hosts-entry" key={`${entry.line_number}-${entry.ip}-${entry.names.join("-")}`}>
                      <span>{formatHostsEntry(entry)}</span>
                      <button
                        className="danger"
                        type="button"
                        onClick={() => deleteHostsEntry(entry.names[0])}
                        disabled={!!busy}
                        title="删除 hosts 映射"
                      >
                        {busy === `hosts-delete-${entry.names[0]}` ? <Loader2 className="spin" /> : <Trash2 />}
                      </button>
                    </div>
                  ))}
                  {hostsState && hostsState.managed_entries.length === 0 && (
                    <p className="hosts-empty">暂无本工具管理的 hosts 映射</p>
                  )}
                </div>
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
              <div className="proxy-tabs" role="tablist" aria-label="中转设置">
                <button
                  className={`proxy-tab ${proxyTab === "login" ? "selected" : ""} ${
                    gogoaisLoggedIn ? "logged-in" : "pending"
                  }`}
                  type="button"
                  role="tab"
                  aria-selected={proxyTab === "login"}
                  onClick={() => setProxyTab("login")}
                >
                  {gogoaisLoggedIn ? <CheckCircle2 /> : <AlertTriangle />}
                  {gogoaisLoggedIn ? "已登录" : "待登录"}
                </button>
                <button
                  className={proxyTab === "config" ? "proxy-tab selected" : "proxy-tab"}
                  type="button"
                  role="tab"
                  aria-selected={proxyTab === "config"}
                  onClick={() => setProxyTab("config")}
                >
                  <Settings2 />
                  中转配置
                </button>
              </div>

              {proxyTab === "login" ? (
                <div className="key-fetch-panel">
                  <label>
                    账号
                    <input
                      value={gogoaisLogin.username}
                      onChange={(event) => {
                        setGogoaisLogin({ ...gogoaisLogin, username: event.target.value });
                        setGogoaisLoggedIn(false);
                      }}
                      placeholder="邮箱或用户名"
                      autoComplete="username"
                    />
                  </label>
                  <label>
                    密码
                    <input
                      type="password"
                      value={gogoaisLogin.password}
                      onChange={(event) => {
                        setGogoaisLogin({ ...gogoaisLogin, password: event.target.value });
                        setGogoaisLoggedIn(false);
                      }}
                      placeholder="密码"
                      autoComplete="current-password"
                    />
                  </label>
                  <button
                    className="primary fetch-key-button"
                    type="button"
                    onClick={fetchGogoaisCodexKey}
                    disabled={!!busy || !gogoaisLogin.username.trim() || !gogoaisLogin.password.trim()}
                  >
                    {busy === "fetch-gogoais-key" ? <Loader2 className="spin" /> : <KeyRound />}
                    获取并填入
                  </button>
                </div>
              ) : (
                <div className="proxy-config-panel">
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
                    只用 API Key 会写入 OPENAI_API_KEY 和完整 provider 配置；沿用登录态只适合已有 ChatGPT tokens 的账号体系。
                  </p>
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
                </div>
              )}
              {proxyTab === "config" && (
                <button className="primary" disabled={!!busy}>
                  {busy === "proxy" ? <Loader2 className="spin" /> : <Zap />}
                  保存中转档案
                </button>
              )}
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
            当前目标客户端：{clientPreferenceInfo.label}。
          </div>
          {manageCodexApp ? (
            <label className="check-row">
              <input
                type="checkbox"
                checked={restartCodex}
                onChange={(event) => setRestartCodex(event.target.checked)}
              />
              <span>
                <strong>切换后重启 Codex App</strong>
                <small>macOS 使用 open -a Codex，Windows 使用 taskkill/PowerShell，Linux 尝试桌面入口或 codex 命令。</small>
              </span>
            </label>
          ) : (
            <div className="check-row info-row">
              {clientPreference === "vscode_extension" ? <Code2 /> : <Terminal />}
              <span>
                <strong>切换后只写入 ~/.codex</strong>
                <small>{clientPreferenceInfo.refreshHint}</small>
              </span>
            </div>
          )}
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
            <strong>{clientPreferenceInfo.resetWarning}</strong>
            <p>
              切号器会先备份 live 的 auth.json 和 config.toml，然后删除这两个文件，
              最后刷新当前状态。{clientPreferenceInfo.refreshHint}
            </p>
          </div>
          <dl className="confirm-grid">
            <div>
              <dt>将删除</dt>
              <dd>auth.json 和 config.toml</dd>
            </div>
            <div>
              <dt>目标客户端</dt>
              <dd>{clientPreferenceInfo.label}</dd>
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

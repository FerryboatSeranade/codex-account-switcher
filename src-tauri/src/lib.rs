use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Utc};
#[cfg(target_os = "windows")]
use encoding_rs::GBK;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    net::IpAddr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use toml_edit::{value, DocumentMut, Item};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Profile {
    id: String,
    #[serde(default)]
    workspace_id: String,
    #[serde(default)]
    isolate_sessions: bool,
    #[serde(default)]
    codex_system: CodexSystem,
    name: String,
    kind: ProfileKind,
    notes: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    config_toml: Option<String>,
    auth_json: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ProfileKind {
    ChatGptLogin,
    ProxyApiKey,
    Custom,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CodexSystem {
    #[default]
    Account,
    Api,
}

#[derive(Debug, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    active_profile_id: Option<String>,
    #[serde(default)]
    client_preference: ClientPreference,
    profiles: Vec<Profile>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ClientPreference {
    #[default]
    CodexApp,
    VscodeExtension,
    CliOther,
}

#[derive(Debug, Serialize)]
struct ProfileSummary {
    id: String,
    workspace_id: String,
    name: String,
    kind: ProfileKind,
    notes: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    config_hash: Option<String>,
    auth_hash: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    account_email: Option<String>,
    account_name: Option<String>,
    account_plan: Option<String>,
    account_id: Option<String>,
    has_config: bool,
    has_auth: bool,
    codex_system: CodexSystem,
    is_active: bool,
}

#[derive(Debug, Serialize)]
struct CurrentCodexState {
    codex_dir: String,
    config_path: String,
    auth_path: String,
    config_exists: bool,
    auth_exists: bool,
    config_hash: Option<String>,
    auth_hash: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    account_email: Option<String>,
    account_name: Option<String>,
    account_plan: Option<String>,
    account_id: Option<String>,
    auth_mode: String,
    active_profile_id: Option<String>,
    session_size: u64,
}

#[derive(Debug, Clone, Default)]
struct AccountInfo {
    email: Option<String>,
    name: Option<String>,
    plan: Option<String>,
    account_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct AppState {
    current: CurrentCodexState,
    client_preference: ClientPreference,
    profiles: Vec<ProfileSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum DiagnosticLevel {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Serialize)]
struct DiagnosticCheck {
    level: DiagnosticLevel,
    title: String,
    detail: String,
}

#[derive(Debug, Serialize)]
struct RecentCodexError {
    occurred_at: Option<String>,
    level: Option<String>,
    target: Option<String>,
    message: String,
    hint: String,
}

#[derive(Debug, Serialize)]
struct CodexDiagnosticReport {
    generated_at: DateTime<Utc>,
    summary: String,
    checks: Vec<DiagnosticCheck>,
    recent_errors: Vec<RecentCodexError>,
}

#[derive(Debug, Serialize)]
struct ClearCodexStateResult {
    message: String,
    backup_dir: Option<String>,
    removed: Vec<String>,
    app_state: AppState,
}

#[derive(Debug, Serialize)]
struct DeleteCodexFileResult {
    message: String,
    backup_dir: Option<String>,
    removed: Option<String>,
    app_state: AppState,
}

#[derive(Debug, Serialize, Clone)]
struct HostsEntry {
    line_number: usize,
    ip: String,
    names: Vec<String>,
    managed: bool,
    comment: Option<String>,
}

#[derive(Debug, Serialize)]
struct HostsState {
    path: String,
    exists: bool,
    entries: Vec<HostsEntry>,
    managed_entries: Vec<HostsEntry>,
}

#[derive(Debug, Serialize)]
struct HostsWriteResult {
    message: String,
    backup_dir: Option<String>,
    dns_flush_message: Option<String>,
    hosts_state: HostsState,
}

#[derive(Debug, Serialize)]
struct RestoreAccountModeResult {
    message: String,
    backup_dir: Option<String>,
    used_profile_name: Option<String>,
    app_state: AppState,
}

#[derive(Debug, Serialize)]
struct SwitchProfileResult {
    message: String,
    app_state: AppState,
}

#[derive(Debug, Serialize)]
struct ClientPreferenceResult {
    message: String,
    app_state: AppState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum SystemProbeStatus {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Serialize)]
struct SystemProbeCheck {
    status: SystemProbeStatus,
    title: String,
    requirement: String,
    detail: String,
    suggestion: String,
}

#[derive(Debug, Serialize)]
struct SystemProbeReport {
    generated_at: DateTime<Utc>,
    summary: String,
    codex_ready: bool,
    codex_ready_title: String,
    codex_ready_detail: String,
    checks: Vec<SystemProbeCheck>,
}

#[derive(Debug, Deserialize)]
struct ImportInput {
    name: String,
    notes: Option<String>,
    kind: ProfileKind,
}

#[derive(Debug, Deserialize)]
struct ProxyProfileInput {
    name: String,
    base_url: String,
    api_key: String,
    model: String,
    review_model: String,
    reasoning_effort: String,
    notes: Option<String>,
    codex_system: Option<CodexSystem>,
}

#[derive(Debug, Deserialize)]
struct GogoaisCodexKeyInput {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct HostsMappingInput {
    ip: String,
    hostname: String,
    aliases: Option<String>,
    comment: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClientPreferenceInput {
    preference: ClientPreference,
}

#[derive(Debug, Serialize)]
struct GogoaisCodexKeyResult {
    api_key: String,
    base_url: Option<String>,
    openai_base_url: Option<String>,
    api_key_name: Option<String>,
    expires_at: Option<String>,
    service_status: Option<String>,
    quota: Option<i64>,
}

fn gogoais_error_message(status: reqwest::StatusCode, value: Option<&serde_json::Value>) -> String {
    let message = value
        .and_then(|parsed| {
            string_at(parsed, &["error"])
                .or_else(|| string_at(parsed, &["message"]))
                .or_else(|| string_at(parsed, &["detail"]))
        })
        .unwrap_or_else(|| status.to_string());
    let lower = message.to_lowercase();
    if status == reqwest::StatusCode::UNAUTHORIZED || lower.contains("invalid username or password")
    {
        "gogoais 账号或密码不正确，请检查后重试。".to_string()
    } else {
        format!("gogoais 获取失败：{}", message)
    }
}

#[derive(Debug, Deserialize)]
struct SwitchInput {
    id: String,
    #[serde(default)]
    restart_codex_app: bool,
}

fn codex_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "无法定位用户主目录".to_string())?;
    Ok(home.join(".codex"))
}

fn app_dir() -> Result<PathBuf, String> {
    let base = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .ok_or_else(|| "无法定位应用数据目录".to_string())?;
    Ok(base.join("codex-account-switcher"))
}

fn store_path() -> Result<PathBuf, String> {
    Ok(app_dir()?.join("profiles.json"))
}

fn managed_session_paths() -> &'static [&'static str] {
    &[
        "sessions",
        "archived_sessions",
        "session_index.jsonl",
        "history.jsonl",
        "state_5.sqlite",
        "state_5.sqlite-shm",
        "state_5.sqlite-wal",
        "goals_1.sqlite",
        "goals_1.sqlite-shm",
        "goals_1.sqlite-wal",
    ]
}

fn read_optional(path: &Path) -> Result<Option<String>, String> {
    if !path.exists() {
        return Ok(None);
    }
    fs::read_to_string(path)
        .map(Some)
        .map_err(|err| format!("读取 {} 失败：{}", path.to_string_lossy(), err))
}

fn write_optional(path: &Path, content: &Option<String>) -> Result<(), String> {
    match content {
        Some(value) => fs::write(path, value)
            .map_err(|err| format!("写入 {} 失败：{}", path.to_string_lossy(), err)),
        None => {
            if path.exists() {
                fs::remove_file(path)
                    .map_err(|err| format!("删除 {} 失败：{}", path.to_string_lossy(), err))?;
            }
            Ok(())
        }
    }
}

fn path_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    if path.is_file() {
        return path.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    }
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| path_size(&entry.path()))
        .sum()
}

fn short_hash(content: &Option<String>) -> Option<String> {
    content.as_ref().map(|value| {
        let digest = Sha256::digest(value.as_bytes());
        format!("{:x}", digest)[..12].to_string()
    })
}

fn load_store() -> Result<Store, String> {
    let path = store_path()?;
    if !path.exists() {
        return Ok(Store {
            active_profile_id: None,
            client_preference: ClientPreference::default(),
            profiles: vec![],
        });
    }
    let raw = fs::read_to_string(&path).map_err(|err| format!("读取档案库失败：{}", err))?;
    let mut store: Store =
        serde_json::from_str(&raw).map_err(|err| format!("解析档案库失败：{}", err))?;
    let mut changed = false;
    for profile in &mut store.profiles {
        if profile.workspace_id.is_empty() {
            profile.workspace_id = Uuid::new_v4().to_string();
            changed = true;
        }
        if profile.isolate_sessions {
            profile.isolate_sessions = false;
            changed = true;
        }
    }
    if changed {
        save_store(&store)?;
    }
    Ok(store)
}

fn save_store(store: &Store) -> Result<(), String> {
    let dir = app_dir()?;
    fs::create_dir_all(&dir).map_err(|err| format!("创建应用数据目录失败：{}", err))?;
    let raw =
        serde_json::to_string_pretty(store).map_err(|err| format!("序列化档案库失败：{}", err))?;
    fs::write(store_path()?, raw).map_err(|err| format!("保存档案库失败：{}", err))
}

fn extract_toml_value(raw: &Option<String>, key: &str) -> Option<String> {
    let raw = raw.as_ref()?;
    raw.lines().find_map(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            return None;
        }
        let (left, right) = trimmed.split_once('=')?;
        if left.trim() != key {
            return None;
        }
        Some(right.trim().trim_matches('"').to_string())
    })
}

fn extract_base_url(raw: &Option<String>) -> Option<String> {
    extract_toml_value(raw, "openai_base_url").or_else(|| extract_toml_value(raw, "base_url"))
}

fn auth_mode(auth: &Option<String>) -> String {
    let Some(raw) = auth else {
        return "未发现 auth.json".to_string();
    };
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(value)
            if value
                .get("OPENAI_API_KEY")
                .and_then(|key| key.as_str())
                .is_some_and(|key| !key.trim().is_empty()) =>
        {
            "API Key".to_string()
        }
        Ok(value)
            if value
                .get("tokens")
                .and_then(|tokens| tokens.get("id_token"))
                .is_some()
                || value.get("refresh_token").is_some() =>
        {
            "ChatGPT 登录授权".to_string()
        }
        Ok(_) => "自定义授权文件".to_string(),
        Err(_) => "auth.json 格式异常".to_string(),
    }
}

fn string_at<'a>(value: &'a serde_json::Value, keys: &[&str]) -> Option<String> {
    let mut cursor = value;
    for key in keys {
        cursor = cursor.get(*key)?;
    }
    cursor.as_str().map(ToString::to_string)
}

fn number_at(value: &serde_json::Value, keys: &[&str]) -> Option<i64> {
    let mut cursor = value;
    for key in keys {
        cursor = cursor.get(*key)?;
    }
    cursor.as_i64()
}

fn decode_jwt_payload(token: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn account_info(auth: &Option<String>) -> AccountInfo {
    let Some(raw) = auth else {
        return AccountInfo::default();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return AccountInfo::default();
    };

    let mut info = AccountInfo {
        account_id: string_at(&value, &["tokens", "account_id"]),
        ..AccountInfo::default()
    };

    if let Some(token) = string_at(&value, &["tokens", "id_token"]) {
        if let Some(payload) = decode_jwt_payload(&token) {
            info.email = string_at(&payload, &["email"]);
            info.name = string_at(&payload, &["name"]);
            info.plan = string_at(
                &payload,
                &["https://api.openai.com/auth", "chatgpt_plan_type"],
            );
            info.account_id = info.account_id.or_else(|| {
                string_at(
                    &payload,
                    &["https://api.openai.com/auth", "chatgpt_account_id"],
                )
            });
        }
    }

    info
}

fn json_key_set(raw: &Option<String>) -> HashSet<String> {
    let Some(raw) = raw else {
        return HashSet::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return HashSet::new();
    };
    value
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default()
}

fn auth_has_api_key(auth: &Option<String>) -> bool {
    let Some(raw) = auth else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return false;
    };
    value
        .get("OPENAI_API_KEY")
        .and_then(|key| key.as_str())
        .is_some_and(|key| !key.trim().is_empty())
}

fn auth_has_login_tokens(auth: &Option<String>) -> bool {
    let Some(raw) = auth else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return false;
    };
    value
        .get("tokens")
        .and_then(|tokens| tokens.get("id_token"))
        .is_some()
        || value.get("refresh_token").is_some()
}

fn config_uses_openai_auth(config: &Option<String>) -> bool {
    let Some(raw) = config else {
        return false;
    };
    raw.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == "requires_openai_auth = true"
            || trimmed == "requires_openai_auth=true"
            || trimmed == "requires_openai_auth = \"true\""
    })
}

fn config_provider(config: &Option<String>) -> Option<String> {
    extract_toml_value(config, "model_provider")
}

fn active_profile<'a>(
    store: &'a Store,
    current_config: &Option<String>,
    current_auth: &Option<String>,
) -> Option<&'a Profile> {
    if let Some(active_id) = &store.active_profile_id {
        if let Some(profile) = store
            .profiles
            .iter()
            .find(|profile| &profile.id == active_id)
        {
            return Some(profile);
        }
    }
    let current_config_hash = short_hash(current_config);
    let current_auth_hash = short_hash(current_auth);
    store.profiles.iter().find(|profile| {
        short_hash(&profile.config_toml) == current_config_hash
            && short_hash(&profile.auth_json) == current_auth_hash
    })
}

fn current_session_size() -> Result<u64, String> {
    let dir = codex_dir()?;
    Ok(managed_session_paths()
        .iter()
        .map(|relative| path_size(&dir.join(relative)))
        .sum())
}

fn current_files() -> Result<(Option<String>, Option<String>), String> {
    let dir = codex_dir()?;
    Ok((
        read_optional(&dir.join("config.toml"))?,
        read_optional(&dir.join("auth.json"))?,
    ))
}

fn current_state(active_profile_id: Option<String>) -> Result<CurrentCodexState, String> {
    let dir = codex_dir()?;
    let (config, auth) = current_files()?;
    let account = account_info(&auth);
    Ok(CurrentCodexState {
        codex_dir: dir.to_string_lossy().to_string(),
        config_path: dir.join("config.toml").to_string_lossy().to_string(),
        auth_path: dir.join("auth.json").to_string_lossy().to_string(),
        config_exists: config.is_some(),
        auth_exists: auth.is_some(),
        config_hash: short_hash(&config),
        auth_hash: short_hash(&auth),
        model: extract_toml_value(&config, "model"),
        base_url: extract_base_url(&config),
        account_email: account.email,
        account_name: account.name,
        account_plan: account.plan,
        account_id: account.account_id,
        auth_mode: auth_mode(&auth),
        active_profile_id,
        session_size: current_session_size()?,
    })
}

fn summarize(
    profile: &Profile,
    current_config: &Option<String>,
    current_auth: &Option<String>,
    active_profile_id: Option<&str>,
) -> ProfileSummary {
    let profile_config_hash = short_hash(&profile.config_toml);
    let profile_auth_hash = short_hash(&profile.auth_json);
    let account = account_info(&profile.auth_json);
    ProfileSummary {
        id: profile.id.clone(),
        workspace_id: profile.workspace_id.clone(),
        name: profile.name.clone(),
        kind: profile.kind.clone(),
        notes: profile.notes.clone(),
        created_at: profile.created_at,
        updated_at: profile.updated_at,
        config_hash: profile_config_hash.clone(),
        auth_hash: profile_auth_hash.clone(),
        model: extract_toml_value(&profile.config_toml, "model"),
        base_url: extract_base_url(&profile.config_toml),
        account_email: account.email,
        account_name: account.name,
        account_plan: account.plan,
        account_id: account.account_id,
        has_config: profile.config_toml.is_some(),
        has_auth: profile.auth_json.is_some(),
        codex_system: profile.codex_system.clone(),
        is_active: active_profile_id == Some(profile.id.as_str())
            || (profile_config_hash == short_hash(current_config)
                && profile_auth_hash == short_hash(current_auth)),
    }
}

fn backup_current() -> Result<Option<String>, String> {
    let dir = codex_dir()?;
    let config_path = dir.join("config.toml");
    let auth_path = dir.join("auth.json");
    if !config_path.exists() && !auth_path.exists() {
        return Ok(None);
    }

    let stamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let backup_dir = app_dir()?.join("backups").join(stamp);
    fs::create_dir_all(&backup_dir).map_err(|err| format!("创建备份目录失败：{}", err))?;
    if config_path.exists() {
        fs::copy(&config_path, backup_dir.join("config.toml"))
            .map_err(|err| format!("备份 config.toml 失败：{}", err))?;
    }
    if auth_path.exists() {
        fs::copy(&auth_path, backup_dir.join("auth.json"))
            .map_err(|err| format!("备份 auth.json 失败：{}", err))?;
    }
    Ok(Some(backup_dir.to_string_lossy().to_string()))
}

fn default_account_config_document() -> DocumentMut {
    r#"model_provider = "openai"
model = "gpt-5.5"
review_model = "gpt-5.5"
model_reasoning_effort = "xhigh"
disable_response_storage = true
network_access = "enabled"
windows_wsl_setup_acknowledged = true
model_context_window = 1000000
model_auto_compact_token_limit = 900000
"#
    .parse::<DocumentMut>()
    .unwrap_or_default()
}

fn account_mode_config(raw: Option<&str>) -> String {
    let mut doc = raw
        .and_then(|value| value.parse::<DocumentMut>().ok())
        .unwrap_or_else(default_account_config_document);

    doc["model_provider"] = value("openai");
    if !doc.contains_key("model") {
        doc["model"] = value("gpt-5.5");
    }
    if !doc.contains_key("review_model") {
        doc["review_model"] = value("gpt-5.5");
    }
    if !doc.contains_key("model_reasoning_effort") {
        doc["model_reasoning_effort"] = value("xhigh");
    }
    if !doc.contains_key("disable_response_storage") {
        doc["disable_response_storage"] = value(true);
    }
    if !doc.contains_key("network_access") {
        doc["network_access"] = value("enabled");
    }
    if !doc.contains_key("windows_wsl_setup_acknowledged") {
        doc["windows_wsl_setup_acknowledged"] = value(true);
    }
    if !doc.contains_key("model_context_window") {
        doc["model_context_window"] = value(1_000_000);
    }
    if !doc.contains_key("model_auto_compact_token_limit") {
        doc["model_auto_compact_token_limit"] = value(900_000);
    }

    doc.remove("openai_base_url");
    doc.remove("chatgpt_base_url");

    if let Some(providers) = doc.get_mut("model_providers").and_then(Item::as_table_mut) {
        providers.remove("OpenAI");
        providers.remove("openai");
        if providers.is_empty() {
            doc.remove("model_providers");
        }
    }
    format!("{}\n", doc.to_string().trim_end())
}

fn update_recent_thread_providers() {
    let Ok(dir) = codex_dir() else {
        return;
    };
    let db_path = dir.join("state_5.sqlite");
    if !db_path.exists() {
        return;
    }
    let script = "update threads set model_provider='openai' where model_provider='OpenAI';";
    let _ = Command::new("sqlite3").arg(db_path).arg(script).status();
}

fn command_stdout(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("执行 {program} 失败：{err}"))?;
    if !output.status.success() {
        let stderr = command_output_text(&output.stderr);
        return Err(if stderr.is_empty() {
            format!("{program} 退出码异常")
        } else {
            stderr
        });
    }
    Ok(command_output_text(&output.stdout))
}

#[cfg(target_os = "windows")]
fn command_output_text(bytes: &[u8]) -> String {
    decode_windows_output(bytes)
}

#[cfg(not(target_os = "windows"))]
fn command_output_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

#[cfg(target_os = "windows")]
struct WindowsCommandError {
    program: String,
    status: String,
    stdout: String,
    stderr: String,
}

#[cfg(target_os = "windows")]
impl WindowsCommandError {
    fn detail(&self) -> String {
        let mut parts = Vec::new();
        if !self.stdout.trim().is_empty() {
            parts.push(format!(
                "stdout: {}",
                first_non_empty_lines(&self.stdout, 4)
            ));
        }
        if !self.stderr.trim().is_empty() {
            parts.push(format!(
                "stderr: {}",
                first_non_empty_lines(&self.stderr, 4)
            ));
        }
        if parts.is_empty() {
            format!("{} 退出码异常：{}", self.program, self.status)
        } else {
            format!(
                "{} 退出码异常：{}；{}",
                self.program,
                self.status,
                parts.join("；")
            )
        }
    }

    fn combined_text(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }

    fn is_access_denied(&self) -> bool {
        let text = self.combined_text().to_ascii_lowercase();
        text.contains("access is denied")
            || text.contains("access denied")
            || self.combined_text().contains("拒绝访问")
    }

    fn has_successful_termination(&self) -> bool {
        let text = self.combined_text().to_ascii_lowercase();
        text.contains("success")
            || self.combined_text().contains("成功")
            || self.combined_text().contains("已成功")
    }
}

#[cfg(target_os = "windows")]
fn text_has_access_denied(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("access is denied")
        || lower.contains("access denied")
        || text.contains("拒绝访问")
}

#[cfg(target_os = "windows")]
fn decode_windows_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    match String::from_utf8(bytes.to_vec()) {
        Ok(value) => value.trim().to_string(),
        Err(_) => {
            let (decoded, _, _) = GBK.decode(bytes);
            decoded.trim().to_string()
        }
    }
}

#[cfg(target_os = "windows")]
fn ps_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(target_os = "windows")]
fn first_non_empty_lines(text: &str, max_lines: usize) -> String {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(max_lines)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        String::new()
    } else {
        lines.join(" / ")
    }
}

#[cfg(target_os = "windows")]
fn command_status_detail(program: &str, args: &[&str]) -> Result<(), WindowsCommandError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| WindowsCommandError {
            program: program.to_string(),
            status: "未启动".to_string(),
            stdout: String::new(),
            stderr: format!("执行 {program} 失败：{err}"),
        })?;
    if output.status.success() {
        return Ok(());
    }

    Err(WindowsCommandError {
        program: program.to_string(),
        status: output.status.to_string(),
        stdout: decode_windows_output(&output.stdout),
        stderr: decode_windows_output(&output.stderr),
    })
}

fn write_to_command_stdin(program: &str, args: &[&str], text: &str) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|err| format!("启动 {program} 失败：{err}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| format!("{program} 无法接收剪贴板内容"))?;
    stdin
        .write_all(text.as_bytes())
        .map_err(|err| format!("写入 {program} 失败：{err}"))?;
    drop(stdin);
    let status = child
        .wait()
        .map_err(|err| format!("等待 {program} 完成失败：{err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} 退出码异常"))
    }
}

#[cfg(target_os = "windows")]
fn windows_powershell_status(script: &str) -> Result<(), String> {
    let args = [
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        script,
    ];
    command_status_detail("powershell.exe", &args)
        .map_err(|err| err.detail())
        .or_else(|powershell_err| {
            command_status_detail("pwsh", &args).map_err(|pwsh_err| {
                format!(
                    "powershell.exe: {powershell_err}；pwsh: {}",
                    pwsh_err.detail()
                )
            })
        })
}

#[cfg(target_os = "windows")]
fn windows_powershell_stdout(script: &str) -> Result<String, String> {
    let args = [
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        script,
    ];
    command_stdout("powershell.exe", &args).or_else(|powershell_err| {
        command_stdout("pwsh", &args)
            .map_err(|pwsh_err| format!("powershell.exe: {powershell_err}；pwsh: {pwsh_err}"))
    })
}

#[cfg(target_os = "windows")]
fn windows_process_exists(image_name: &str) -> Result<bool, String> {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {image_name}"), "/NH"])
        .output()
        .map_err(|err| format!("执行 tasklist 失败：{err}"))?;
    if !output.status.success() {
        let stderr = decode_windows_output(&output.stderr);
        return Err(if stderr.is_empty() {
            format!("tasklist 退出码异常：{}", output.status)
        } else {
            stderr
        });
    }

    let needle = image_name.to_ascii_lowercase();
    let stdout = decode_windows_output(&output.stdout).to_ascii_lowercase();
    Ok(stdout.lines().any(|line| line.contains(&needle)))
}

#[cfg(target_os = "windows")]
fn codex_windows_process_exists() -> Result<bool, String> {
    windows_process_exists("Codex.exe")
}

fn system_probe_check(
    status: SystemProbeStatus,
    title: &str,
    requirement: &str,
    detail: impl Into<String>,
    suggestion: impl Into<String>,
) -> SystemProbeCheck {
    SystemProbeCheck {
        status,
        title: title.to_string(),
        requirement: requirement.to_string(),
        detail: detail.into(),
        suggestion: suggestion.into(),
    }
}

#[cfg(target_os = "macos")]
fn quit_codex_process() -> Result<(), String> {
    let _ = Command::new("osascript")
        .args(["-e", r#"tell application "Codex" to quit"#])
        .status();
    thread::sleep(Duration::from_millis(900));
    let _ = Command::new("pkill").args(["-x", "Codex"]).status();
    thread::sleep(Duration::from_millis(500));
    Ok(())
}

#[cfg(target_os = "windows")]
fn quit_codex_process() -> Result<(), String> {
    let mut failures = Vec::new();
    let mut any_success = false;
    let mut access_denied = false;

    match codex_windows_process_exists() {
        Ok(false) => return Ok(()),
        Ok(true) => {}
        Err(err) => failures.push(format!("检测 Codex.exe 进程失败：{err}")),
    }

    let close_script = r#"
$ErrorActionPreference = "SilentlyContinue"
Get-Process -Name Codex | ForEach-Object { $_.CloseMainWindow() | Out-Null }
Start-Sleep -Milliseconds 900
"#;
    if let Err(err) = windows_powershell_status(close_script) {
        failures.push(format!("温和关闭失败：{err}"));
    }

    match codex_windows_process_exists() {
        Ok(false) => return Ok(()),
        Ok(true) => {}
        Err(err) => failures.push(format!("温和关闭后检测进程失败：{err}")),
    }

    match command_status_detail("taskkill", &["/F", "/T", "/IM", "Codex.exe"]) {
        Ok(()) => any_success = true,
        Err(err) => {
            any_success = any_success || err.has_successful_termination();
            access_denied = access_denied || err.is_access_denied();
            failures.push(format!("taskkill 失败：{}", err.detail()));
        }
    }
    thread::sleep(Duration::from_millis(800));

    match codex_windows_process_exists() {
        Ok(false) => return Ok(()),
        Ok(true) => {}
        Err(err) => failures.push(format!("taskkill 后检测进程失败：{err}")),
    }

    let script = r#"
$ErrorActionPreference = "Continue"
Get-Process -Name Codex -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction Continue
"#;
    match windows_powershell_status(script) {
        Ok(()) => any_success = true,
        Err(err) => {
            access_denied = access_denied || text_has_access_denied(&err);
            failures.push(format!("PowerShell 失败：{err}"));
        }
    }
    thread::sleep(Duration::from_millis(1200));

    match codex_windows_process_exists() {
        Ok(false) => Ok(()),
        Ok(true) if access_denied => Err(format!(
            "Codex app 仍在运行，Windows 拒绝当前切号器结束部分 Codex 进程。请点击“以管理员身份重启切号器”，或手动关闭 Codex app 后重试。{}",
            if any_success {
                "已成功关闭了一部分 Codex 子进程，但仍有进程需要更高权限。".to_string()
            } else {
                String::new()
            }
        )),
        Ok(true) => Err(format!(
            "已尝试通过温和关闭、taskkill 和 PowerShell 停止 Codex app，但 Codex.exe 仍在运行。请手动关闭 Codex app 后重试。{}",
            if failures.is_empty() {
                String::new()
            } else {
                format!(" 详细：{}", failures.join("；"))
            }
        )),
        Err(err) if any_success => Ok(()),
        Err(err) => Err(format!(
            "无法确认 Codex app 是否已关闭：{err}。{}",
            if failures.is_empty() {
                "请手动关闭 Codex app 后重试。".to_string()
            } else {
                format!("详细：{}", failures.join("；"))
            }
        )),
    }
}

#[cfg(target_os = "linux")]
fn quit_codex_process() -> Result<(), String> {
    let status = Command::new("sh")
        .args([
            "-lc",
            "pkill -f '(^|/)(Codex|codex)( |$)' >/dev/null 2>&1 || true",
        ])
        .status()
        .map_err(|err| format!("停止 Codex 失败：{}", err))?;
    if !status.success() {
        return Err("未能通过 pkill 停止 Codex app".to_string());
    }
    thread::sleep(Duration::from_millis(1200));
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn quit_codex_process() -> Result<(), String> {
    Err("当前系统暂不支持自动停止 Codex app".to_string())
}

#[cfg(target_os = "macos")]
fn start_codex_process() -> Result<(), String> {
    let status = Command::new("open")
        .args(["-a", "Codex"])
        .status()
        .map_err(|err| format!("启动 Codex 失败：{}", err))?;
    if status.success() {
        Ok(())
    } else {
        Err("未能通过 open -a Codex 启动 Codex app".to_string())
    }
}

#[cfg(target_os = "windows")]
fn start_codex_process() -> Result<(), String> {
    let script = r#"
$ErrorActionPreference = "Stop"
Start-Process Codex
"#;
    command_status_detail("cmd.exe", &["/C", "start", "", "Codex"])
        .map_err(|err| err.detail())
        .or_else(|cmd_err| {
            windows_powershell_status(script)
                .map_err(|ps_err| format!("cmd.exe: {cmd_err}；PowerShell: {ps_err}"))
        })
        .map_err(|err| format!("未能通过 Windows shell 启动 Codex app。详细：{err}"))
}

#[cfg(target_os = "linux")]
fn start_codex_process() -> Result<(), String> {
    let script = r#"
(gtk-launch codex.desktop >/dev/null 2>&1 || gtk-launch Codex.desktop >/dev/null 2>&1 || nohup codex >/dev/null 2>&1 &)
"#;
    let status = Command::new("sh")
        .args(["-lc", script])
        .status()
        .map_err(|err| format!("启动 Codex 失败：{}", err))?;
    if status.success() {
        Ok(())
    } else {
        Err("未能通过 Linux 桌面入口或 codex 命令启动 Codex app".to_string())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn start_codex_process() -> Result<(), String> {
    Err("当前系统暂不支持自动启动 Codex app".to_string())
}

fn restart_codex_process() -> Result<(), String> {
    quit_codex_process()?;
    thread::sleep(Duration::from_millis(900));
    start_codex_process()
}

fn client_refresh_hint(preference: &ClientPreference) -> &'static str {
    match preference {
        ClientPreference::CodexApp => "Codex App 已自动处理。",
        ClientPreference::VscodeExtension => "请在 VS Code 中执行 Reload Window，或重启 VS Code 后让 Codex 扩展重新读取 ~/.codex。",
        ClientPreference::CliOther => "请重启当前终端里的 Codex CLI/相关进程，让它重新读取 ~/.codex。",
    }
}

fn should_manage_codex_app(preference: &ClientPreference) -> bool {
    matches!(preference, ClientPreference::CodexApp)
}

#[cfg(target_os = "windows")]
fn restart_switcher_as_admin_process() -> Result<(), String> {
    let current_exe = env::current_exe().map_err(|err| format!("读取当前程序路径失败：{err}"))?;
    let exe = current_exe.to_string_lossy().to_string();
    let script = format!(
        r#"
$ErrorActionPreference = "Stop"
Start-Process -FilePath {} -Verb RunAs
"#,
        ps_single_quote(&exe)
    );
    windows_powershell_status(&script)
        .map_err(|err| format!("无法以管理员身份重启切号器：{err}"))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn restart_switcher_as_admin_process() -> Result<(), String> {
    Err("当前系统不需要 Windows 管理员重启流程".to_string())
}

fn read_file_tail(path: &Path, max_bytes: u64) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(max_bytes);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buffer = Vec::new();
    file.take(max_bytes).read_to_end(&mut buffer).ok()?;
    Some(String::from_utf8_lossy(&buffer).to_string())
}

fn latest_log_text() -> String {
    let Ok(dir) = codex_dir() else {
        return String::new();
    };
    ["logs_2.sqlite-wal", "logs_2.sqlite"]
        .iter()
        .filter_map(|name| read_file_tail(&dir.join(name), 512 * 1024))
        .collect::<Vec<_>>()
        .join("\n")
}

fn line_hint(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    if lower.contains("invalid_api_key") || lower.contains("invalid api key") {
        "API Key 被服务端拒绝；请检查当前档案里的 Key 是否属于这个 Base URL，或者重新生成 Key。"
            .to_string()
    } else if lower.contains("chatgpt.com") && lower.contains("401") {
        "ChatGPT 账号登录态可能已失效；重置账号状态后重新登录通常最快。".to_string()
    } else if lower.contains("401") || lower.contains("unauthorized") {
        "服务端返回未授权；重点检查 auth.json 与 config.toml 是否匹配当前体系。".to_string()
    } else {
        "查看相邻日志可获得更多上下文。".to_string()
    }
}

fn recent_auth_errors() -> Vec<RecentCodexError> {
    let text = latest_log_text();
    let mut errors = Vec::new();
    for line in text.lines().rev() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("sse event") || lower.contains("response.output_text") {
            continue;
        }
        if !lower.contains("401")
            && !lower.contains("unauthorized")
            && !lower.contains("invalid_api_key")
            && !lower.contains("invalid api key")
        {
            continue;
        }
        if !lower.contains("turn error")
            && !lower.contains("failed")
            && !lower.contains("invalid_api_key")
            && !lower.contains("unauthorized")
        {
            continue;
        }
        let message = line
            .split('\0')
            .filter(|part| !part.trim().is_empty())
            .last()
            .unwrap_or(line)
            .trim()
            .chars()
            .take(900)
            .collect::<String>();
        if message.is_empty() {
            continue;
        }
        errors.push(RecentCodexError {
            occurred_at: None,
            level: None,
            target: None,
            hint: line_hint(&message),
            message,
        });
        if errors.len() >= 5 {
            break;
        }
    }
    errors
}

fn diagnostic_check(level: DiagnosticLevel, title: &str, detail: String) -> DiagnosticCheck {
    DiagnosticCheck {
        level,
        title: title.to_string(),
        detail,
    }
}

#[tauri::command]
fn diagnose_codex_state() -> Result<CodexDiagnosticReport, String> {
    let (config, auth) = current_files()?;
    let current = current_state(None)?;
    let provider = config_provider(&config).unwrap_or_else(|| "未设置".to_string());
    let base_url = current
        .base_url
        .clone()
        .unwrap_or_else(|| "默认 OpenAI".to_string());
    let has_api_key = auth_has_api_key(&auth);
    let has_login_tokens = auth_has_login_tokens(&auth);
    let uses_openai_auth = config_uses_openai_auth(&config);
    let auth_keys = {
        let mut keys = json_key_set(&auth).into_iter().collect::<Vec<_>>();
        keys.sort();
        keys
    };
    let recent_errors = recent_auth_errors();
    let mut checks = Vec::new();

    checks.push(diagnostic_check(
        if current.config_exists {
            DiagnosticLevel::Ok
        } else {
            DiagnosticLevel::Warning
        },
        "config.toml",
        if current.config_exists {
            format!("已读取配置，provider 为 {provider}，Base URL 为 {base_url}。")
        } else {
            "未发现 config.toml；Codex 会回到默认配置或要求重新初始化。".to_string()
        },
    ));

    checks.push(diagnostic_check(
        if current.auth_exists {
            DiagnosticLevel::Ok
        } else {
            DiagnosticLevel::Error
        },
        "auth.json",
        if current.auth_exists {
            format!(
                "授权形态：{}；顶层字段：{}。",
                current.auth_mode,
                if auth_keys.is_empty() {
                    "无法解析".to_string()
                } else {
                    auth_keys.join(", ")
                }
            )
        } else {
            "未发现 auth.json；需要重新登录或切换到含授权的档案。".to_string()
        },
    ));

    if uses_openai_auth && has_api_key && !has_login_tokens {
        checks.push(diagnostic_check(
            DiagnosticLevel::Warning,
            "账号体系与 API Key 混用",
            "当前配置要求 OpenAI 授权，但 auth.json 只有 OPENAI_API_KEY，没有 ChatGPT 登录 tokens。若 Base URL 指向中转，这通常表示按 API Key 体系工作；如果你想用 Plus/Pro 登录，需要切到保存了 tokens 的账号档案或清除后重登。".to_string(),
        ));
    }

    if !base_url.contains("api.openai.com") && has_api_key {
        checks.push(diagnostic_check(
            DiagnosticLevel::Warning,
            "中转 Key 匹配",
            format!(
                "当前 Base URL 是 {base_url}，服务端只会接受它自己认可的 Key；最近的 INVALID_API_KEY/401 一般就是 Key 与该中转不匹配。"
            ),
        ));
    }

    if !recent_errors.is_empty() {
        checks.push(diagnostic_check(
            DiagnosticLevel::Error,
            "最近授权错误",
            format!(
                "日志尾部发现 {} 条 401/Unauthorized/Invalid API key 线索。",
                recent_errors.len()
            ),
        ));
    }

    if checks.is_empty() {
        checks.push(diagnostic_check(
            DiagnosticLevel::Ok,
            "状态",
            "没有发现明显问题。".to_string(),
        ));
    }

    let has_error = checks
        .iter()
        .any(|check| matches!(check.level, DiagnosticLevel::Error));
    let has_warning = checks
        .iter()
        .any(|check| matches!(check.level, DiagnosticLevel::Warning));
    let summary = if has_error {
        "发现需要处理的 Codex 授权问题".to_string()
    } else if has_warning {
        "发现可能导致 401 的配置风险".to_string()
    } else {
        "Codex 状态看起来正常".to_string()
    };

    Ok(CodexDiagnosticReport {
        generated_at: Utc::now(),
        summary,
        checks,
        recent_errors,
    })
}

fn best_login_profile(store: &Store) -> Option<Profile> {
    store
        .profiles
        .iter()
        .filter(|profile| profile.auth_json.is_some() && auth_has_login_tokens(&profile.auth_json))
        .max_by_key(|profile| {
            let kind_score = if profile.kind == ProfileKind::ChatGptLogin {
                1
            } else {
                0
            };
            (kind_score, profile.updated_at)
        })
        .cloned()
}

#[tauri::command]
fn restore_account_mode() -> Result<RestoreAccountModeResult, String> {
    let dir = codex_dir()?;
    fs::create_dir_all(&dir).map_err(|err| format!("创建 ~/.codex 目录失败：{}", err))?;
    let mut store = load_store()?;
    if should_manage_codex_app(&store.client_preference) {
        quit_codex_process()?;
    }
    let backup_dir = backup_current()?;
    let current_config = read_optional(&dir.join("config.toml"))?;
    let login_profile = best_login_profile(&store);
    let auth_json = login_profile
        .as_ref()
        .and_then(|profile| profile.auth_json.clone());

    fs::write(
        dir.join("config.toml"),
        account_mode_config(current_config.as_deref()),
    )
    .map_err(|err| format!("写入官方账号体系 config.toml 失败：{}", err))?;
    match &auth_json {
        Some(value) => fs::write(dir.join("auth.json"), value)
            .map_err(|err| format!("恢复账号 auth.json 失败：{}", err))?,
        None => {
            let auth_path = dir.join("auth.json");
            if auth_path.exists() {
                fs::remove_file(&auth_path)
                    .map_err(|err| format!("删除旧 auth.json 失败：{}", err))?;
            }
        }
    }

    store.active_profile_id = login_profile.as_ref().map(|profile| profile.id.clone());
    save_store(&store)?;
    update_recent_thread_providers();
    if should_manage_codex_app(&store.client_preference) {
        restart_codex_process()?;
    }
    let used_profile_name = login_profile.as_ref().map(|profile| profile.name.clone());
    let base_message = if let Some(name) = &used_profile_name {
        format!("已恢复官方账号体系并使用账号档案「{name}」，中转 Base URL/API Key 已从 live 配置移除。")
    } else {
        "已恢复官方账号体系并移除中转 Base URL/API Key；未找到保存的登录 tokens，请在 Codex 里重新登录。"
            .to_string()
    };
    let message = format!("{base_message} {}", client_refresh_hint(&store.client_preference));

    Ok(RestoreAccountModeResult {
        message,
        backup_dir,
        used_profile_name,
        app_state: get_app_state()?,
    })
}

#[tauri::command]
fn clear_codex_state() -> Result<ClearCodexStateResult, String> {
    let dir = codex_dir()?;
    fs::create_dir_all(&dir).map_err(|err| format!("创建 ~/.codex 目录失败：{}", err))?;
    let mut store = load_store()?;
    if should_manage_codex_app(&store.client_preference) {
        quit_codex_process()?;
    }
    let backup_dir = backup_current()?;
    let mut removed = Vec::new();
    for name in ["auth.json", "config.toml"] {
        let path = dir.join(name);
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|err| format!("删除 {} 失败：{}", path.to_string_lossy(), err))?;
            removed.push(name.to_string());
        }
    }
    if store.active_profile_id.is_some() {
        store.active_profile_id = None;
        save_store(&store)?;
    }
    let app_state = get_app_state()?;
    let message = if removed.is_empty() {
        "没有可重置的 auth.json 或 config.toml".to_string()
    } else if should_manage_codex_app(&store.client_preference) {
        restart_codex_process()?;
        format!("已停止 Codex App、重置 {} 并重新启动。", removed.join("、"))
    } else {
        format!(
            "已备份并重置 {}。{}",
            removed.join("、"),
            client_refresh_hint(&store.client_preference)
        )
    };
    Ok(ClearCodexStateResult {
        message,
        backup_dir,
        removed,
        app_state,
    })
}

#[tauri::command]
fn delete_codex_file(name: String) -> Result<DeleteCodexFileResult, String> {
    let name = name.trim();
    if name != "auth.json" && name != "config.toml" {
        return Err("只能删除 auth.json 或 config.toml".to_string());
    }
    let dir = codex_dir()?;
    fs::create_dir_all(&dir).map_err(|err| format!("创建 ~/.codex 目录失败：{}", err))?;
    let mut store = load_store()?;
    if should_manage_codex_app(&store.client_preference) {
        quit_codex_process()?;
    }
    let backup_dir = backup_current()?;
    let path = dir.join(name);
    let removed = if path.exists() {
        fs::remove_file(&path)
            .map_err(|err| format!("删除 {} 失败：{}", path.to_string_lossy(), err))?;
        Some(name.to_string())
    } else {
        None
    };
    if name == "auth.json" && removed.is_some() {
        if store.active_profile_id.is_some() {
            store.active_profile_id = None;
            save_store(&store)?;
        }
    }
    let message = if removed.is_some() && should_manage_codex_app(&store.client_preference) {
        format!("已停止 Codex App 并删除 {name}")
    } else if removed.is_some() {
        format!("已备份并删除 {name}。{}", client_refresh_hint(&store.client_preference))
    } else if should_manage_codex_app(&store.client_preference) {
        format!("已停止 Codex App；{name} 不存在，无需删除")
    } else {
        format!("{name} 不存在，无需删除。{}", client_refresh_hint(&store.client_preference))
    };
    Ok(DeleteCodexFileResult {
        message,
        backup_dir,
        removed,
        app_state: get_app_state()?,
    })
}

#[tauri::command]
fn get_app_state() -> Result<AppState, String> {
    let (current_config, current_auth) = current_files()?;
    let store = load_store()?;
    let active_id =
        active_profile(&store, &current_config, &current_auth).map(|profile| profile.id.clone());
    let current = current_state(active_id.clone())?;
    let client_preference = store.client_preference.clone();
    let profiles = store
        .profiles
        .iter()
        .map(|profile| {
            summarize(
                profile,
                &current_config,
                &current_auth,
                active_id.as_deref(),
            )
        })
        .collect();

    Ok(AppState {
        current,
        client_preference,
        profiles,
    })
}

#[tauri::command]
fn set_client_preference(input: ClientPreferenceInput) -> Result<ClientPreferenceResult, String> {
    let mut store = load_store()?;
    store.client_preference = input.preference;
    save_store(&store)?;
    let message = format!(
        "已设置目标客户端偏好。{}",
        client_refresh_hint(&store.client_preference)
    );
    Ok(ClientPreferenceResult {
        message,
        app_state: get_app_state()?,
    })
}

#[tauri::command]
fn import_current_profile(input: ImportInput) -> Result<AppState, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("请输入档案名称".to_string());
    }

    let (config_toml, auth_json) = current_files()?;
    if config_toml.is_none() && auth_json.is_none() {
        return Err("当前 ~/.codex 下没有可导入的 config.toml 或 auth.json".to_string());
    }

    let now = Utc::now();
    let mut store = load_store()?;
    store.profiles.push(Profile {
        id: Uuid::new_v4().to_string(),
        workspace_id: Uuid::new_v4().to_string(),
        isolate_sessions: false,
        codex_system: CodexSystem::Account,
        name: name.to_string(),
        kind: input.kind,
        notes: input.notes.unwrap_or_default(),
        created_at: now,
        updated_at: now,
        config_toml,
        auth_json,
    });
    save_store(&store)?;
    get_app_state()
}

#[tauri::command]
fn create_proxy_profile(input: ProxyProfileInput) -> Result<AppState, String> {
    let name = input.name.trim();
    let base_url = input.base_url.trim().trim_end_matches('/');
    let api_key = input.api_key.trim();
    let codex_system = input.codex_system.unwrap_or_default();
    if name.is_empty() || base_url.is_empty() {
        return Err("名称和 Base URL 不能为空".to_string());
    }
    if codex_system == CodexSystem::Api && api_key.is_empty() {
        return Err("只用 API Key 时需要填写 API Key".to_string());
    }

    let model = if input.model.trim().is_empty() {
        "gpt-5.5"
    } else {
        input.model.trim()
    };
    let review_model = if input.review_model.trim().is_empty() {
        model
    } else {
        input.review_model.trim()
    };
    let effort = if input.reasoning_effort.trim().is_empty() {
        "xhigh"
    } else {
        input.reasoning_effort.trim()
    };

    let config_toml = match codex_system {
        CodexSystem::Account => format!(
            r#"model_provider = "openai"
model = "{model}"
review_model = "{review_model}"
model_reasoning_effort = "{effort}"
disable_response_storage = true
network_access = "enabled"
windows_wsl_setup_acknowledged = true
model_context_window = 1000000
model_auto_compact_token_limit = 900000
openai_base_url = "{base_url}"
"#
        ),
        CodexSystem::Api => format!(
            r#"model_provider = "OpenAI"
model = "{model}"
review_model = "{review_model}"
model_reasoning_effort = "{effort}"
disable_response_storage = true
network_access = "enabled"
windows_wsl_setup_acknowledged = true
model_context_window = 1000000
model_auto_compact_token_limit = 900000

[model_providers.OpenAI]
name = "OpenAI"
base_url = "{base_url}"
wire_api = "responses"
requires_openai_auth = true
"#
        ),
    };
    let auth_json = match codex_system {
        CodexSystem::Account => {
            let (_, current_auth) = current_files()?;
            let current_mode = auth_mode(&current_auth);
            if current_mode == "ChatGPT 登录授权" {
                current_auth
                    .unwrap_or_else(|| serde_json::json!({ "OPENAI_API_KEY": api_key }).to_string())
            } else if api_key.is_empty() {
                return Err(
                    "当前没有 ChatGPT 登录授权；请先导入/登录账号，或填写 API Key 作为兜底"
                        .to_string(),
                );
            } else {
                serde_json::json!({ "OPENAI_API_KEY": api_key }).to_string()
            }
        }
        CodexSystem::Api => serde_json::json!({ "OPENAI_API_KEY": api_key }).to_string(),
    };
    let now = Utc::now();
    let mut store = load_store()?;
    store.profiles.push(Profile {
        id: Uuid::new_v4().to_string(),
        workspace_id: Uuid::new_v4().to_string(),
        isolate_sessions: false,
        codex_system,
        name: name.to_string(),
        kind: ProfileKind::ProxyApiKey,
        notes: input.notes.unwrap_or_default(),
        created_at: now,
        updated_at: now,
        config_toml: Some(config_toml),
        auth_json: Some(format!("{}\n", auth_json.trim_end())),
    });
    save_store(&store)?;
    get_app_state()
}

#[tauri::command]
async fn fetch_gogoais_codex_key(
    input: GogoaisCodexKeyInput,
) -> Result<GogoaisCodexKeyResult, String> {
    let username = input.username.trim();
    let password = input.password.trim();
    if username.is_empty() || password.is_empty() {
        return Err("账号和密码不能为空".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|err| format!("创建 HTTP 客户端失败：{}", err))?;

    let response = client
        .get("https://x-api.gogoais.com/api/public/codex-key")
        .query(&[("username", username), ("password", password)])
        .header("accept", "application/json")
        .header("content-type", "application/json")
        .send()
        .await
        .map_err(|err| format!("请求 gogoais Codex Key 失败：{}", err))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("读取 gogoais 响应失败：{}", err))?;
    let parsed_body = serde_json::from_str::<serde_json::Value>(&body).ok();
    if !status.is_success() {
        return Err(gogoais_error_message(status, parsed_body.as_ref()));
    }
    let value = parsed_body.ok_or_else(|| "解析 gogoais 响应失败：响应不是 JSON".to_string())?;
    if value
        .get("success")
        .and_then(|success| success.as_bool())
        .is_some_and(|success| !success)
    {
        return Err(gogoais_error_message(status, Some(&value)));
    }

    let api_key = string_at(&value, &["data", "codex", "api_key"])
        .or_else(|| string_at(&value, &["data", "codex", "sk"]))
        .ok_or_else(|| "gogoais 响应里没有 data.codex.api_key".to_string())?;
    if api_key.trim().is_empty() {
        return Err("gogoais 返回了空 API Key".to_string());
    }

    Ok(GogoaisCodexKeyResult {
        api_key,
        base_url: string_at(&value, &["data", "codex", "base_url"]),
        openai_base_url: string_at(&value, &["data", "codex", "openai_base_url"]),
        api_key_name: string_at(&value, &["data", "codex", "api_key_name"]),
        expires_at: string_at(&value, &["data", "codex", "expires_at"])
            .or_else(|| string_at(&value, &["data", "service", "expiry_time"])),
        service_status: string_at(&value, &["data", "service", "status"]),
        quota: number_at(&value, &["data", "codex", "quota"]),
    })
}

#[tauri::command]
fn switch_profile(input: SwitchInput) -> Result<AppState, String> {
    apply_profile(input.id, false)
}

fn apply_profile(id: String, manage_codex_app: bool) -> Result<AppState, String> {
    let mut store = load_store()?;
    let target_profile = store
        .profiles
        .iter()
        .find(|profile| profile.id == id)
        .cloned()
        .ok_or_else(|| "找不到指定档案".to_string())?;

    let dir = codex_dir()?;
    fs::create_dir_all(&dir).map_err(|err| format!("创建 ~/.codex 目录失败：{}", err))?;
    if manage_codex_app {
        quit_codex_process()?;
    }
    backup_current()?;
    write_optional(&dir.join("config.toml"), &target_profile.config_toml)?;
    write_optional(&dir.join("auth.json"), &target_profile.auth_json)?;
    store.active_profile_id = Some(target_profile.id);
    save_store(&store)?;
    if manage_codex_app {
        thread::sleep(Duration::from_millis(500));
        start_codex_process()?;
    }
    get_app_state()
}

#[tauri::command]
fn switch_profile_and_restart(input: SwitchInput) -> Result<SwitchProfileResult, String> {
    let store = load_store()?;
    let manage_codex_app =
        input.restart_codex_app && should_manage_codex_app(&store.client_preference);
    let preference = store.client_preference.clone();
    let app_state = apply_profile(input.id, manage_codex_app)?;
    let message = if manage_codex_app {
        "已停止 Codex App、切换档案并重新启动。".to_string()
    } else {
        format!("已切换档案。{}", client_refresh_hint(&preference))
    };
    Ok(SwitchProfileResult {
        message,
        app_state,
    })
}

#[tauri::command]
fn delete_profile(id: String) -> Result<AppState, String> {
    let mut store = load_store()?;
    let before = store.profiles.len();
    store.profiles.retain(|profile| profile.id != id);
    if store.profiles.len() == before {
        return Err("找不到指定档案".to_string());
    }
    save_store(&store)?;
    get_app_state()
}

fn open_path_with_system(path: &Path, label: &str) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("{} 不存在：{}", label, path.to_string_lossy()));
    }

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open")
            .arg(path)
            .status()
            .map_err(|err| format!("打开文件失败：{}", err))?;
        if status.success() {
            return Ok(());
        }
        return Err(format!("未能通过 open 打开 {label}"));
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("notepad.exe")
            .arg(path)
            .spawn()
            .map_err(|err| format!("通过 notepad.exe 打开 {label} 失败：{err}"))?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let status = Command::new("xdg-open")
            .arg(path)
            .status()
            .map_err(|err| format!("打开文件失败：{}", err))?;
        if status.success() {
            return Ok(());
        }
        return Err(format!("未能通过 xdg-open 打开 {label}"));
    }

    #[allow(unreachable_code)]
    Err(format!("当前系统暂不支持打开 {label}"))
}

#[tauri::command]
fn open_codex_file(name: String) -> Result<String, String> {
    if name != "config.toml" && name != "auth.json" {
        return Err("只能打开 config.toml 或 auth.json".to_string());
    }
    let path = codex_dir()?.join(&name);
    open_path_with_system(&path, &name)?;
    Ok(format!("已打开 {}", path.to_string_lossy()))
}

#[tauri::command]
fn open_codex_config() -> Result<String, String> {
    open_codex_file("config.toml".to_string())
}

const HOSTS_MARKER: &str = "codex-account-switcher";

fn hosts_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let root = env::var("SystemRoot")
            .or_else(|_| env::var("WINDIR"))
            .unwrap_or_else(|_| "C:\\Windows".to_string());
        return PathBuf::from(root)
            .join("System32")
            .join("drivers")
            .join("etc")
            .join("hosts");
    }

    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from("/etc/hosts")
    }
}

fn read_hosts_file(path: &Path) -> Result<String, String> {
    if !path.exists() {
        return Ok(String::new());
    }
    match fs::read_to_string(path) {
        Ok(value) => Ok(value),
        Err(err) if err.kind() == io::ErrorKind::InvalidData => {
            let bytes =
                fs::read(path).map_err(|read_err| format!("读取 hosts 文件失败：{read_err}"))?;
            Ok(String::from_utf8_lossy(&bytes).to_string())
        }
        Err(err) => Err(format!("读取 hosts 文件失败：{err}")),
    }
}

fn split_hosts_comment(line: &str) -> (&str, Option<&str>) {
    line.split_once('#')
        .map(|(code, comment)| (code, Some(comment)))
        .unwrap_or((line, None))
}

fn parse_hosts_entries(raw: &str) -> Vec<HostsEntry> {
    raw.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let (code, comment) = split_hosts_comment(line);
            let tokens = code.split_whitespace().collect::<Vec<_>>();
            if tokens.len() < 2 || tokens[0].parse::<IpAddr>().is_err() {
                return None;
            }
            Some(HostsEntry {
                line_number: index + 1,
                ip: tokens[0].to_string(),
                names: tokens[1..].iter().map(|name| (*name).to_string()).collect(),
                managed: comment.is_some_and(|value| value.contains(HOSTS_MARKER)),
                comment: comment.map(|value| value.trim().to_string()),
            })
        })
        .collect()
}

fn get_hosts_state_from_path(path: &Path) -> Result<HostsState, String> {
    let raw = read_hosts_file(path)?;
    let entries = parse_hosts_entries(&raw);
    let managed_entries = entries
        .iter()
        .filter(|entry| entry.managed)
        .cloned()
        .collect();
    Ok(HostsState {
        path: path.to_string_lossy().to_string(),
        exists: path.exists(),
        entries,
        managed_entries,
    })
}

#[tauri::command]
fn get_hosts_state() -> Result<HostsState, String> {
    get_hosts_state_from_path(&hosts_path())
}

fn validate_hosts_ip(value: &str) -> Result<String, String> {
    let ip = value
        .trim()
        .parse::<IpAddr>()
        .map_err(|_| "请输入有效的 IPv4 或 IPv6 地址".to_string())?;
    Ok(ip.to_string())
}

fn validate_hosts_name(value: &str) -> Result<String, String> {
    let name = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if name.is_empty() {
        return Err("域名不能为空".to_string());
    }
    if name == "*" || name.contains('*') {
        return Err("hosts 文件不支持通配符域名，请填写具体域名".to_string());
    }
    if name.len() > 253 {
        return Err("域名过长".to_string());
    }
    for label in name.split('.') {
        if label.is_empty() {
            return Err("域名格式不正确".to_string());
        }
        if label.len() > 63 {
            return Err("域名单段长度不能超过 63 个字符".to_string());
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err("域名单段不能以连字符开头或结尾".to_string());
        }
        if !label
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        {
            return Err("域名只能包含字母、数字、连字符和点".to_string());
        }
    }
    Ok(name)
}

fn collect_hosts_names(hostname: &str, aliases: Option<&str>) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    for value in std::iter::once(hostname).chain(
        aliases
            .unwrap_or_default()
            .split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';'),
    ) {
        if value.trim().is_empty() {
            continue;
        }
        let name = validate_hosts_name(value)?;
        if seen.insert(name.clone()) {
            names.push(name);
        }
    }
    if names.is_empty() {
        return Err("至少需要填写一个域名".to_string());
    }
    Ok(names)
}

fn hosts_name_key(value: &str) -> String {
    value.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn sanitize_hosts_comment(value: Option<&str>) -> String {
    value
        .unwrap_or_default()
        .replace(['\r', '\n', '#'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn hosts_mapping_line(ip: &str, names: &[String], comment: Option<&str>) -> String {
    let mut marker = HOSTS_MARKER.to_string();
    let comment = sanitize_hosts_comment(comment);
    if !comment.is_empty() {
        marker.push_str(": ");
        marker.push_str(&comment);
    }
    format!("{} {} # {}", ip, names.join(" "), marker)
}

fn rewrite_hosts_for_upsert(raw: &str, target_names: &HashSet<String>) -> (Vec<String>, usize) {
    let mut changed = 0;
    let mut lines = Vec::new();
    for line in raw.lines() {
        let (code, comment) = split_hosts_comment(line);
        let managed = comment.is_some_and(|value| value.contains(HOSTS_MARKER));
        let tokens = code.split_whitespace().collect::<Vec<_>>();
        if tokens.len() < 2 || tokens[0].parse::<IpAddr>().is_err() {
            lines.push(line.to_string());
            continue;
        }
        if !managed {
            lines.push(line.to_string());
            continue;
        }

        let remaining = tokens[1..]
            .iter()
            .copied()
            .filter(|name| !target_names.contains(&hosts_name_key(name)))
            .collect::<Vec<_>>();
        if remaining.len() == tokens.len() - 1 {
            lines.push(line.to_string());
            continue;
        }

        changed += 1;
        if remaining.is_empty() {
            continue;
        }

        let mut rewritten = format!("{} {}", tokens[0], remaining.join(" "));
        if let Some(comment) = comment {
            rewritten.push_str(" #");
            rewritten.push_str(comment);
        }
        lines.push(rewritten);
    }
    (lines, changed)
}

fn unmanaged_hosts_conflicts(raw: &str, target_names: &HashSet<String>) -> Vec<String> {
    let mut conflicts = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        let (code, comment) = split_hosts_comment(line);
        if comment.is_some_and(|value| value.contains(HOSTS_MARKER)) {
            continue;
        }
        let tokens = code.split_whitespace().collect::<Vec<_>>();
        if tokens.len() < 2 || tokens[0].parse::<IpAddr>().is_err() {
            continue;
        }
        let matched = tokens[1..]
            .iter()
            .filter(|name| target_names.contains(&hosts_name_key(name)))
            .copied()
            .collect::<Vec<_>>();
        if !matched.is_empty() {
            conflicts.push(format!(
                "第 {} 行：{} -> {}",
                index + 1,
                tokens[0],
                matched.join(", ")
            ));
        }
    }
    conflicts
}

fn rewrite_hosts_for_delete(raw: &str, target_name: &str) -> (Vec<String>, usize) {
    let target = hosts_name_key(target_name);
    let mut changed = 0;
    let mut lines = Vec::new();
    for line in raw.lines() {
        let (code, comment) = split_hosts_comment(line);
        let managed = comment.is_some_and(|value| value.contains(HOSTS_MARKER));
        let tokens = code.split_whitespace().collect::<Vec<_>>();
        if !managed
            || tokens.len() < 2
            || tokens[0].parse::<IpAddr>().is_err()
            || !tokens[1..]
                .iter()
                .any(|name| hosts_name_key(name) == target)
        {
            lines.push(line.to_string());
            continue;
        }
        changed += 1;
    }
    (lines, changed)
}

fn backup_hosts_file(path: &Path) -> Result<Option<String>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let stamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let backup_dir = app_dir()?.join("backups").join(format!("hosts-{stamp}"));
    fs::create_dir_all(&backup_dir).map_err(|err| format!("创建 hosts 备份目录失败：{err}"))?;
    fs::copy(path, backup_dir.join("hosts"))
        .map_err(|err| format!("备份 hosts 文件失败：{err}"))?;
    Ok(Some(backup_dir.to_string_lossy().to_string()))
}

fn hosts_permission_error(action: &str, err: &io::Error) -> String {
    #[cfg(target_os = "windows")]
    {
        format!(
            "Windows 拒绝{action} hosts 文件。请点击“以管理员身份重启切号器”后重试，或手动用管理员权限编辑 hosts。详细：{err}"
        )
    }

    #[cfg(target_os = "macos")]
    {
        format!("macOS 拒绝{action} hosts 文件。请确认系统授权弹窗，或手动用 sudo 编辑 /etc/hosts。详细：{err}")
    }

    #[cfg(target_os = "linux")]
    {
        format!("Linux 拒绝{action} hosts 文件。请以 root/admin 权限运行切号器，或手动用 sudo 编辑 /etc/hosts。详细：{err}")
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        format!("当前系统拒绝{action} hosts 文件：{err}")
    }
}

#[cfg(target_os = "macos")]
fn sh_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(target_os = "macos")]
fn apple_double_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(target_os = "macos")]
fn write_hosts_file_elevated(path: &Path, text: &str) -> Result<(), String> {
    let temp_dir = app_dir()?.join("hosts-write");
    fs::create_dir_all(&temp_dir).map_err(|err| format!("创建临时目录失败：{err}"))?;
    let temp_path = temp_dir.join("hosts.tmp");
    fs::write(&temp_path, text).map_err(|err| format!("写入 hosts 临时文件失败：{err}"))?;
    let shell_command = format!(
        "cp {} {} && chmod 644 {}",
        sh_single_quote(&temp_path.to_string_lossy()),
        sh_single_quote(&path.to_string_lossy()),
        sh_single_quote(&path.to_string_lossy())
    );
    let script = format!(
        "do shell script {} with administrator privileges",
        apple_double_quote(&shell_command)
    );
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|err| format!("请求 macOS 管理员权限失败：{err}"))?;
    let _ = fs::remove_file(&temp_path);
    if output.status.success() {
        Ok(())
    } else {
        let stderr = command_output_text(&output.stderr);
        Err(if stderr.is_empty() {
            "macOS 管理员写入 hosts 被取消或失败".to_string()
        } else {
            format!("macOS 管理员写入 hosts 失败：{stderr}")
        })
    }
}

fn write_hosts_file(path: &Path, text: &str) -> Result<(), String> {
    match fs::write(path, text) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            #[cfg(target_os = "macos")]
            {
                write_hosts_file_elevated(path, text).map_err(|elevated_err| {
                    format!("{}；{elevated_err}", hosts_permission_error("写入", &err))
                })
            }

            #[cfg(not(target_os = "macos"))]
            {
                Err(hosts_permission_error("写入", &err))
            }
        }
        Err(err) => Err(format!("写入 hosts 文件失败：{err}")),
    }
}

fn hosts_line_ending(raw: &str) -> &'static str {
    if raw.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn compose_hosts_text(
    mut lines: Vec<String>,
    line_ending: &str,
    new_line: Option<String>,
) -> String {
    if let Some(new_line) = new_line {
        let has_marker = lines.iter().any(|line| line.contains(HOSTS_MARKER));
        if !lines.is_empty() && lines.last().is_some_and(|line| !line.trim().is_empty()) {
            lines.push(String::new());
        }
        if !has_marker {
            lines.push(format!("# {HOSTS_MARKER} managed hosts"));
        }
        lines.push(new_line);
    }
    let mut text = lines.join(line_ending);
    if !text.ends_with(line_ending) {
        text.push_str(line_ending);
    }
    text
}

fn flush_dns_cache() -> String {
    #[cfg(target_os = "windows")]
    {
        return match command_stdout("ipconfig", &["/flushdns"]) {
            Ok(_) => "已刷新 Windows DNS 缓存".to_string(),
            Err(err) => format!("hosts 已写入，但刷新 Windows DNS 缓存失败：{err}"),
        };
    }

    #[cfg(target_os = "macos")]
    {
        let dscache_ok = Command::new("dscacheutil")
            .arg("-flushcache")
            .status()
            .is_ok_and(|status| status.success());
        let mdns_ok = Command::new("killall")
            .args(["-HUP", "mDNSResponder"])
            .status()
            .is_ok_and(|status| status.success());
        return if dscache_ok || mdns_ok {
            "已尝试刷新 macOS DNS 缓存".to_string()
        } else {
            "hosts 已写入，但刷新 macOS DNS 缓存命令未成功；重启 Codex 或浏览器后通常会生效"
                .to_string()
        };
    }

    #[cfg(target_os = "linux")]
    {
        for (program, args) in [
            ("resolvectl", vec!["flush-caches"]),
            ("systemd-resolve", vec!["--flush-caches"]),
            ("nscd", vec!["-i", "hosts"]),
        ] {
            if Command::new(program)
                .args(args)
                .status()
                .is_ok_and(|status| status.success())
            {
                return format!("已尝试通过 {program} 刷新 DNS 缓存");
            }
        }
        "hosts 已写入，但未找到可用的 Linux DNS 缓存刷新命令；重启 Codex 或相关网络服务后通常会生效"
            .to_string()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "hosts 已写入；当前系统暂不支持自动刷新 DNS 缓存".to_string()
    }
}

#[tauri::command]
fn upsert_hosts_mapping(input: HostsMappingInput) -> Result<HostsWriteResult, String> {
    let ip = validate_hosts_ip(&input.ip)?;
    let names = collect_hosts_names(&input.hostname, input.aliases.as_deref())?;
    let target_names = names.iter().map(|name| hosts_name_key(name)).collect();
    let path = hosts_path();
    let raw = read_hosts_file(&path)?;
    let line_ending = hosts_line_ending(&raw);
    let conflicts = unmanaged_hosts_conflicts(&raw, &target_names);
    if !conflicts.is_empty() {
        return Err(format!(
            "hosts 中已存在同域名的手动映射，本工具不会自动改写非托管行。请点击“打开”手动处理后再保存。冲突：{}",
            conflicts.join("；")
        ));
    }
    let (lines, touched_lines) = rewrite_hosts_for_upsert(&raw, &target_names);
    let new_line = hosts_mapping_line(&ip, &names, input.comment.as_deref());
    let new_text = compose_hosts_text(lines, line_ending, Some(new_line));
    let backup_dir = backup_hosts_file(&path)?;
    write_hosts_file(&path, &new_text)?;
    let dns_flush_message = Some(flush_dns_cache());
    let message = if touched_lines > 0 {
        format!("已更新 hosts 映射：{} -> {}", names.join(", "), ip)
    } else {
        format!("已添加 hosts 映射：{} -> {}", names.join(", "), ip)
    };
    Ok(HostsWriteResult {
        message,
        backup_dir,
        dns_flush_message,
        hosts_state: get_hosts_state_from_path(&path)?,
    })
}

#[tauri::command]
fn delete_hosts_mapping(hostname: String) -> Result<HostsWriteResult, String> {
    let hostname = validate_hosts_name(&hostname)?;
    let path = hosts_path();
    let raw = read_hosts_file(&path)?;
    let line_ending = hosts_line_ending(&raw);
    let (lines, removed_count) = rewrite_hosts_for_delete(&raw, &hostname);
    if removed_count == 0 {
        return Ok(HostsWriteResult {
            message: format!("没有找到本工具管理的 hosts 映射：{hostname}"),
            backup_dir: None,
            dns_flush_message: None,
            hosts_state: get_hosts_state_from_path(&path)?,
        });
    }
    let new_text = compose_hosts_text(lines, line_ending, None);
    let backup_dir = backup_hosts_file(&path)?;
    write_hosts_file(&path, &new_text)?;
    let dns_flush_message = Some(flush_dns_cache());
    Ok(HostsWriteResult {
        message: format!("已删除本工具管理的 hosts 映射：{hostname}"),
        backup_dir,
        dns_flush_message,
        hosts_state: get_hosts_state_from_path(&path)?,
    })
}

#[tauri::command]
fn open_hosts_file() -> Result<String, String> {
    let path = hosts_path();
    open_path_with_system(&path, "hosts")?;
    Ok(format!("已打开 {}", path.to_string_lossy()))
}

#[cfg(target_os = "macos")]
fn detect_system_proxy() -> SystemProbeCheck {
    let services = command_stdout("networksetup", &["-listallnetworkservices"]).unwrap_or_default();
    let mut enabled = Vec::new();
    let mut unavailable = Vec::new();
    for service in services
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("An asterisk"))
    {
        let web = command_stdout("networksetup", &["-getwebproxy", service]).unwrap_or_default();
        let secure =
            command_stdout("networksetup", &["-getsecurewebproxy", service]).unwrap_or_default();
        let socks = command_stdout("networksetup", &["-getsocksfirewallproxy", service])
            .unwrap_or_default();
        let combined = format!("{web}\n{secure}\n{socks}");
        if combined.contains("Enabled: Yes") {
            enabled.push(service.to_string());
        } else {
            unavailable.push(service.to_string());
        }
    }

    if enabled.is_empty() {
        system_probe_check(
            SystemProbeStatus::Warning,
            "系统代理",
            "辅助项：不是 Codex 必须条件，但能解释网络为何可用或不可用。",
            "没有发现 macOS 网络服务开启 HTTP、HTTPS 或 SOCKS 代理。若你用的是 TUN/VPN 模式，系统代理为空也可能正常。",
            "如果 Google 连接失败，请打开代理客户端的系统代理，或切到 TUN/VPN 模式后重新检测。",
        )
    } else {
        system_probe_check(
            SystemProbeStatus::Ok,
            "系统代理",
            "辅助项：不是 Codex 必须条件，但能解释网络为何可用或不可用。",
            format!("已发现这些网络服务开启代理：{}。Codex 发起 HTTPS 请求时通常可以走这些代理。", enabled.join(", ")),
            "如果后续 Codex 仍然请求失败，请确认代理规则包含 api.openai.com、chatgpt.com 或你的中转 Base URL。",
        )
    }
}

#[cfg(target_os = "windows")]
fn detect_system_proxy() -> SystemProbeCheck {
    let script = r#"
$proxy = Get-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
if ($proxy.ProxyEnable -eq 1) { "enabled $($proxy.ProxyServer)" } else { "disabled" }
"#;
    match windows_powershell_stdout(script) {
        Ok(output) if output.starts_with("enabled") => system_probe_check(
            SystemProbeStatus::Ok,
            "系统代理",
            "辅助项：不是 Codex 必须条件，但能解释网络为何可用或不可用。",
            format!(
                "Windows 系统代理已开启：{}。Codex 发起 HTTPS 请求时通常可以走系统代理。",
                output.replacen("enabled", "", 1).trim()
            ),
            "如果后续 Codex 仍然请求失败，请确认代理规则包含 api.openai.com、chatgpt.com 或你的中转 Base URL。",
        ),
        Ok(_) => system_probe_check(
            SystemProbeStatus::Warning,
            "系统代理",
            "辅助项：不是 Codex 必须条件，但能解释网络为何可用或不可用。",
            "Windows 系统代理未开启。若你用的是 TUN/VPN 模式、透明代理或直连可访问，这不一定是问题。".to_string(),
            "如果 Google 连接失败，请打开代理客户端的系统代理，或切到 TUN/VPN 模式后重新检测。",
        ),
        Err(err) => system_probe_check(
            SystemProbeStatus::Error,
            "系统代理",
            "辅助项：不是 Codex 必须条件，但能解释网络为何可用或不可用。",
            format!("无法检测 Windows 系统代理：{err}。"),
            "这项检测失败不一定影响 Codex；请优先看 Google 连接和 Codex 配置检测结果。",
        ),
    }
}

#[cfg(target_os = "linux")]
fn detect_system_proxy() -> SystemProbeCheck {
    let env_proxy = [
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "ALL_PROXY",
        "https_proxy",
        "http_proxy",
        "all_proxy",
    ]
    .iter()
    .find_map(|key| {
        env::var(key)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| (*key, value))
    });
    if let Some((key, value)) = env_proxy {
        return system_probe_check(
            SystemProbeStatus::Ok,
            "系统代理",
            "辅助项：不是 Codex 必须条件，但能解释网络为何可用或不可用。",
            format!("检测到代理环境变量 {key}={value}。CLI 或从同一环境启动的进程通常会继承它。"),
            "如果桌面版 Codex 仍不能联网，确认桌面进程是否继承了该环境变量，或改用系统代理/TUN。",
        );
    }

    let gsettings = command_stdout("gsettings", &["get", "org.gnome.system.proxy", "mode"]).ok();
    if let Some(mode) = gsettings {
        if !mode.contains("none") {
            return system_probe_check(
                SystemProbeStatus::Ok,
                "系统代理",
                "辅助项：不是 Codex 必须条件，但能解释网络为何可用或不可用。",
                format!("GNOME 系统代理模式为 {mode}。桌面应用通常可以读取系统代理。"),
                "如果 Codex 仍不能联网，请确认代理规则包含 api.openai.com、chatgpt.com 或你的中转 Base URL。",
            );
        }
    }

    system_probe_check(
        SystemProbeStatus::Warning,
        "系统代理",
        "辅助项：不是 Codex 必须条件，但能解释网络为何可用或不可用。",
        "未检测到常见代理环境变量，也未检测到 GNOME 系统代理。若你用的是 TUN/VPN 模式或直连可访问，这不一定是问题。".to_string(),
        "如果 Google 连接失败，请配置 HTTP_PROXY/HTTPS_PROXY，或打开代理客户端的系统代理/TUN 模式。",
    )
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn detect_system_proxy() -> SystemProbeCheck {
    system_probe_check(
        SystemProbeStatus::Warning,
        "系统代理",
        "辅助项：不是 Codex 必须条件，但能解释网络为何可用或不可用。",
        "当前系统暂不支持自动检测系统代理。".to_string(),
        "请以 Google 连接结果为准；如果连接失败，需要手动确认代理或网络策略。",
    )
}

#[cfg(target_os = "macos")]
fn detect_virtual_adapter() -> SystemProbeCheck {
    match command_stdout("ifconfig", &[]) {
        Ok(output) => {
            let mut names = Vec::new();
            for block in output.split("\n\n") {
                let Some((name, _)) = block.split_once(':') else {
                    continue;
                };
                let lower = block.to_ascii_lowercase();
                if lower.contains("utun")
                    || lower.contains("tun")
                    || lower.contains("tap")
                    || lower.contains("wg")
                    || lower.contains("vpn")
                {
                    names.push(name.trim().to_string());
                }
            }
            if names.is_empty() {
                system_probe_check(
                    SystemProbeStatus::Warning,
                    "虚拟网卡",
                    "辅助项：不是 Codex 必须条件；只有你依赖 TUN/VPN 模式时才关键。",
                    "没有发现常见 utun/tun/tap/wg/vpn 网卡。若你使用系统代理或直连，这不影响 Codex。".to_string(),
                    "如果你的代理客户端应处于 TUN/VPN 模式，请检查客户端里是否已启用虚拟网卡，并确认系统授权。",
                )
            } else {
                system_probe_check(
                    SystemProbeStatus::Ok,
                    "虚拟网卡",
                    "辅助项：不是 Codex 必须条件；只有你依赖 TUN/VPN 模式时才关键。",
                    format!("检测到可能的虚拟网卡：{}。这说明 TUN/VPN/类似透明代理通道可能已经启用。", names.join(", ")),
                    "如果 Google 连接正常，Codex 的基础网络条件基本满足；如果不正常，请检查该虚拟网卡的路由规则。",
                )
            }
        }
        Err(err) => system_probe_check(
            SystemProbeStatus::Error,
            "虚拟网卡",
            "辅助项：不是 Codex 必须条件；只有你依赖 TUN/VPN 模式时才关键。",
            format!("无法执行 ifconfig：{err}。"),
            "这项检测失败不一定影响 Codex；请优先看 Google 连接和代理检测结果。",
        ),
    }
}

#[cfg(target_os = "windows")]
fn detect_virtual_adapter() -> SystemProbeCheck {
    let script = r#"
Get-NetAdapter | Where-Object {
  $_.Status -eq 'Up' -and ($_.InterfaceDescription -match 'TAP|TUN|WireGuard|Wintun|VPN|Clash|Tailscale|ZeroTier')
} | Select-Object -ExpandProperty Name
"#;
    match windows_powershell_stdout(script) {
        Ok(output) if !output.trim().is_empty() => system_probe_check(
            SystemProbeStatus::Ok,
            "虚拟网卡",
            "辅助项：不是 Codex 必须条件；只有你依赖 TUN/VPN 模式时才关键。",
            format!(
                "检测到已启用的虚拟网卡：{}。这说明 TUN/VPN/类似透明代理通道可能已经启用。",
                output.lines().collect::<Vec<_>>().join(", ")
            ),
            "如果 Google 连接正常，Codex 的基础网络条件基本满足；如果不正常，请检查该虚拟网卡的路由规则。",
        ),
        Ok(_) => system_probe_check(
            SystemProbeStatus::Warning,
            "虚拟网卡",
            "辅助项：不是 Codex 必须条件；只有你依赖 TUN/VPN 模式时才关键。",
            "未检测到常见 TAP/TUN/WireGuard/VPN 虚拟网卡处于 Up 状态。若你使用系统代理或直连，这不影响 Codex。".to_string(),
            "如果你的代理客户端应处于 TUN/VPN 模式，请检查客户端里是否已启用虚拟网卡，并确认系统授权。",
        ),
        Err(err) => system_probe_check(
            SystemProbeStatus::Error,
            "虚拟网卡",
            "辅助项：不是 Codex 必须条件；只有你依赖 TUN/VPN 模式时才关键。",
            format!("无法检测 Windows 网卡：{err}。"),
            "这项检测失败不一定影响 Codex；请优先看 Google 连接和代理检测结果。",
        ),
    }
}

#[cfg(target_os = "linux")]
fn detect_virtual_adapter() -> SystemProbeCheck {
    let output = command_stdout(
        "sh",
        &[
            "-lc",
            "ip -o link show 2>/dev/null || ifconfig -a 2>/dev/null",
        ],
    );
    match output {
        Ok(value) => {
            let mut names = Vec::new();
            for line in value.lines() {
                let lower = line.to_ascii_lowercase();
                if lower.contains("tun")
                    || lower.contains("tap")
                    || lower.contains("wg")
                    || lower.contains("vpn")
                    || lower.contains("tailscale")
                    || lower.contains("zerotier")
                {
                    if let Some(name) = line.split(':').nth(1).map(str::trim) {
                        names.push(name.to_string());
                    }
                }
            }
            if names.is_empty() {
                system_probe_check(
                    SystemProbeStatus::Warning,
                    "虚拟网卡",
                    "辅助项：不是 Codex 必须条件；只有你依赖 TUN/VPN 模式时才关键。",
                    "未检测到常见 tun/tap/wg/vpn 虚拟网卡。若你使用系统代理或直连，这不影响 Codex。".to_string(),
                    "如果你的代理客户端应处于 TUN/VPN 模式，请检查客户端里是否已启用虚拟网卡，并确认系统授权。",
                )
            } else {
                system_probe_check(
                    SystemProbeStatus::Ok,
                    "虚拟网卡",
                    "辅助项：不是 Codex 必须条件；只有你依赖 TUN/VPN 模式时才关键。",
                    format!("检测到可能的虚拟网卡：{}。这说明 TUN/VPN/类似透明代理通道可能已经启用。", names.join(", ")),
                    "如果 Google 连接正常，Codex 的基础网络条件基本满足；如果不正常，请检查该虚拟网卡的路由规则。",
                )
            }
        }
        Err(err) => system_probe_check(
            SystemProbeStatus::Error,
            "虚拟网卡",
            "辅助项：不是 Codex 必须条件；只有你依赖 TUN/VPN 模式时才关键。",
            format!("无法检测网卡：{err}。"),
            "这项检测失败不一定影响 Codex；请优先看 Google 连接和代理检测结果。",
        ),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn detect_virtual_adapter() -> SystemProbeCheck {
    system_probe_check(
        SystemProbeStatus::Warning,
        "虚拟网卡",
        "辅助项：不是 Codex 必须条件；只有你依赖 TUN/VPN 模式时才关键。",
        "当前系统暂不支持自动检测虚拟网卡。".to_string(),
        "请以 Google 连接结果为准；如果连接失败，需要手动确认代理客户端或网络策略。",
    )
}

fn detect_codex_files() -> SystemProbeCheck {
    match current_files() {
        Ok((config, auth)) => {
            let model =
                extract_toml_value(&config, "model").unwrap_or_else(|| "未设置".to_string());
            let base_url = extract_base_url(&config).unwrap_or_else(|| "默认 OpenAI".to_string());
            let mode = auth_mode(&auth);
            let has_auth = auth_has_api_key(&auth) || auth_has_login_tokens(&auth);
            if config.is_some() && has_auth {
                system_probe_check(
                    SystemProbeStatus::Ok,
                    "Codex 配置",
                    "必要项：需要 config.toml 指定模型/服务，并需要 auth.json 里有登录态或 API Key。",
                    format!("已找到 config.toml 和可用授权。模型：{model}；Base URL：{base_url}；授权方式：{mode}。"),
                    "配置基础条件通过。若请求仍失败，重点检查该授权是否属于当前 Base URL，以及模型名是否被服务端支持。",
                )
            } else if config.is_some() {
                system_probe_check(
                    SystemProbeStatus::Warning,
                    "Codex 配置",
                    "必要项：需要 config.toml 指定模型/服务，并需要 auth.json 里有登录态或 API Key。",
                    format!("已找到 config.toml，但 auth.json 没有可识别的 ChatGPT 登录态或 OPENAI_API_KEY。模型：{model}；Base URL：{base_url}；授权方式：{mode}。"),
                    "请切换到含授权的档案、导入当前登录状态，或创建只用 API Key 的中转档案。",
                )
            } else if has_auth {
                system_probe_check(
                    SystemProbeStatus::Warning,
                    "Codex 配置",
                    "必要项：需要 config.toml 指定模型/服务，并需要 auth.json 里有登录态或 API Key。",
                    format!("已找到授权文件，但没有 config.toml。授权方式：{mode}。Codex 可能回到默认配置，也可能要求重新初始化。"),
                    "建议打开 config.toml 或切换到一个完整档案，确保模型、Base URL 与授权方式一致。",
                )
            } else {
                system_probe_check(
                    SystemProbeStatus::Error,
                    "Codex 配置",
                    "必要项：需要 config.toml 指定模型/服务，并需要 auth.json 里有登录态或 API Key。",
                    "没有找到可用的 Codex 配置和授权。当前环境不具备直接使用 Codex 的基本账号/API 条件。".to_string(),
                    "请先登录 Codex，或在切号器里导入/创建一个含 auth.json 与 config.toml 的档案。",
                )
            }
        }
        Err(err) => system_probe_check(
            SystemProbeStatus::Error,
            "Codex 配置",
            "必要项：需要 config.toml 指定模型/服务，并需要 auth.json 里有登录态或 API Key。",
            format!("读取 ~/.codex 失败：{err}。"),
            "请确认用户主目录可访问，并检查 ~/.codex 目录权限。",
        ),
    }
}

fn detect_google_connectivity() -> SystemProbeCheck {
    let output_arg = if cfg!(target_os = "windows") {
        "NUL"
    } else {
        "/dev/null"
    };
    let output = Command::new("curl")
        .args([
            "-L",
            "--connect-timeout",
            "8",
            "--max-time",
            "12",
            "-o",
            output_arg,
            "-s",
            "-w",
            "%{http_code} %{time_total}",
            "https://www.google.com",
        ])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            let stdout = command_output_text(&result.stdout);
            let mut parts = stdout.split_whitespace();
            let code = parts.next().unwrap_or("未知");
            let seconds = parts.next().unwrap_or("未知");
            system_probe_check(
                SystemProbeStatus::Ok,
                "Google 连接",
                "必要项：Codex 要正常请求模型，至少需要能访问外网或你的中转服务。",
                format!("curl https://www.google.com 成功，HTTP {code}，耗时 {seconds}s。说明当前网络具备访问外网的能力。"),
                "网络基础条件通过。下一步若 Codex 仍不可用，请检查 ~/.codex/config.toml 的 Base URL、auth.json 登录态或 API Key 是否匹配。",
            )
        }
        Ok(result) => {
            let stderr = command_output_text(&result.stderr);
            system_probe_check(
                SystemProbeStatus::Error,
                "Google 连接",
                "必要项：Codex 要正常请求模型，至少需要能访问外网或你的中转服务。",
                if stderr.is_empty() {
                    "curl google.com 失败，未返回错误详情。".to_string()
                } else {
                    format!("curl google.com 失败：{stderr}。")
                },
                "这通常表示当前网络、代理或 DNS 不满足 Codex 访问官方服务的条件。请先打开代理/TUN/VPN，或确认你的中转 Base URL 可以直连。",
            )
        }
        Err(err) => system_probe_check(
            SystemProbeStatus::Error,
            "Google 连接",
            "必要项：Codex 要正常请求模型，至少需要能访问外网或你的中转服务。",
            format!("无法执行 curl：{err}。"),
            "请安装 curl 或把它加入 PATH；若没有 curl，本工具无法自动验证网络，但 Codex 本身仍可能可用。",
        ),
    }
}

#[tauri::command]
fn detect_system_network() -> Result<SystemProbeReport, String> {
    let checks = vec![
        detect_codex_files(),
        detect_system_proxy(),
        detect_virtual_adapter(),
        detect_google_connectivity(),
    ];
    let error_count = checks
        .iter()
        .filter(|check| matches!(check.status, SystemProbeStatus::Error))
        .count();
    let warning_count = checks
        .iter()
        .filter(|check| matches!(check.status, SystemProbeStatus::Warning))
        .count();
    let google_ok = checks
        .iter()
        .any(|check| check.title == "Google 连接" && matches!(check.status, SystemProbeStatus::Ok));
    let codex_config_ok = checks
        .iter()
        .any(|check| check.title == "Codex 配置" && matches!(check.status, SystemProbeStatus::Ok));
    let codex_ready = google_ok && codex_config_ok;
    let codex_ready_title = if codex_ready {
        "Codex 使用环境：可用".to_string()
    } else if google_ok {
        "Codex 使用环境：网络可用，但配置/授权还需处理".to_string()
    } else {
        "Codex 使用环境：暂不可用".to_string()
    };
    let codex_ready_detail = if codex_ready {
        "当前机器可以访问外网，并且 ~/.codex 下有可识别的配置和授权。若仍失败，问题大概率在模型名、Base URL 与 API Key/登录态是否匹配。".to_string()
    } else if google_ok {
        "当前机器网络是通的，但 Codex 配置或授权不完整。先处理 Codex 配置项，再重试请求。"
            .to_string()
    } else {
        "当前机器未通过 Google 连通性检测。请先确认代理、TUN/VPN、DNS 或中转网络是否可用。"
            .to_string()
    };
    let summary = if codex_ready {
        if warning_count > 0 {
            format!("Codex 使用环境检测完成：基础环境可用；另有 {warning_count} 个非阻塞提醒。")
        } else {
            "Codex 使用环境检测完成：基础环境可用。".to_string()
        }
    } else if error_count > 0 {
        format!("Codex 使用环境检测完成：基础环境未通过；{error_count} 项失败，{warning_count} 项提醒。")
    } else {
        format!("Codex 使用环境检测完成：基础环境还需确认；{warning_count} 项提醒。")
    };

    Ok(SystemProbeReport {
        generated_at: Utc::now(),
        summary,
        codex_ready,
        codex_ready_title,
        codex_ready_detail,
        checks,
    })
}

#[tauri::command]
fn detect_codex_environment() -> Result<SystemProbeReport, String> {
    detect_system_network()
}

#[tauri::command]
fn copy_text_to_clipboard(text: String) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("没有可复制的检测结果".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        write_to_command_stdin("pbcopy", &[], &text)?;
        return Ok("检测结果已复制到剪贴板".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        write_to_command_stdin(
            "powershell.exe",
            &[
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "[Console]::InputEncoding = [System.Text.Encoding]::UTF8; Set-Clipboard -Value ([Console]::In.ReadToEnd())",
            ],
            &text,
        )
        .or_else(|_| {
            write_to_command_stdin(
                "pwsh",
                &[
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    "[Console]::InputEncoding = [System.Text.Encoding]::UTF8; Set-Clipboard -Value ([Console]::In.ReadToEnd())",
                ],
                &text,
            )
        })?;
        return Ok("检测结果已复制到剪贴板".to_string());
    }

    #[cfg(target_os = "linux")]
    {
        if write_to_command_stdin("wl-copy", &[], &text).is_ok()
            || write_to_command_stdin("xclip", &["-selection", "clipboard"], &text).is_ok()
            || write_to_command_stdin("xsel", &["--clipboard", "--input"], &text).is_ok()
        {
            return Ok("检测结果已复制到剪贴板".to_string());
        }
        return Err("无法写入剪贴板：未找到 wl-copy、xclip 或 xsel".to_string());
    }

    #[allow(unreachable_code)]
    Err("当前系统暂不支持自动复制到剪贴板".to_string())
}

#[tauri::command]
fn restart_codex_app() -> Result<String, String> {
    restart_codex_process()?;
    Ok("已尝试重启 Codex app".to_string())
}

#[tauri::command]
fn quit_codex_app() -> Result<String, String> {
    quit_codex_process()?;
    Ok("已尝试关闭 Codex app".to_string())
}

#[tauri::command]
fn restart_switcher_as_admin(app: tauri::AppHandle) -> Result<String, String> {
    restart_switcher_as_admin_process()?;
    app.exit(0);
    Ok("已请求以管理员身份重启切号器".to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            diagnose_codex_state,
            restore_account_mode,
            import_current_profile,
            create_proxy_profile,
            fetch_gogoais_codex_key,
            set_client_preference,
            switch_profile,
            switch_profile_and_restart,
            delete_profile,
            clear_codex_state,
            delete_codex_file,
            open_codex_file,
            open_codex_config,
            get_hosts_state,
            upsert_hosts_mapping,
            delete_hosts_mapping,
            open_hosts_file,
            detect_codex_environment,
            detect_system_network,
            copy_text_to_clipboard,
            quit_codex_app,
            restart_codex_app,
            restart_switcher_as_admin
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_set(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_managed_hosts_entries() {
        let raw =
            "127.0.0.1 localhost\n10.0.0.2 api.local app.local # codex-account-switcher: test\n";
        let entries = parse_hosts_entries(raw);
        assert_eq!(entries.len(), 2);
        assert!(!entries[0].managed);
        assert!(entries[1].managed);
        assert_eq!(entries[1].names, vec!["api.local", "app.local"]);
    }

    #[test]
    fn upsert_only_rewrites_managed_rows() {
        let raw = "10.0.0.1 manual.local\n10.0.0.2 old.local api.local # codex-account-switcher\n";
        let (lines, touched) = rewrite_hosts_for_upsert(raw, &name_set(&["api.local"]));
        assert_eq!(touched, 1);
        assert_eq!(lines[0], "10.0.0.1 manual.local");
        assert_eq!(lines[1], "10.0.0.2 old.local # codex-account-switcher");
    }

    #[test]
    fn detects_unmanaged_conflicts() {
        let raw = "10.0.0.1 api.local\n10.0.0.2 api.local # codex-account-switcher\n";
        let conflicts = unmanaged_hosts_conflicts(raw, &name_set(&["api.local"]));
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0].contains("第 1 行"));
    }

    #[test]
    fn delete_only_removes_managed_rows() {
        let raw = "10.0.0.1 api.local\n10.0.0.2 api.local # codex-account-switcher\n";
        let (lines, removed) = rewrite_hosts_for_delete(raw, "api.local");
        assert_eq!(removed, 1);
        assert_eq!(lines, vec!["10.0.0.1 api.local"]);
    }
}

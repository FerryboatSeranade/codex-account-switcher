# AGENTS

## Project Rules

- Keep Tauri commands responsive. Any command that can run external processes, touch the network, sleep, stop/start apps, open installers, write system files, or wait for user action must be `async` and move blocking work into `tauri::async_runtime::spawn_blocking`.
- Do not wait for GUI apps, system settings panels, installers, browsers, or control panels to exit. Use `Command::spawn()` for these and return after launch.
- Do not run long CLI flows synchronously from the UI thread. Device-code login, Codex install, winget/npm/brew commands, DNS/network diagnostics, app restart/quit, and hosts writes are blocking-risk flows.
- External network checks must have explicit timeouts. For example, `curl` checks should use both `--connect-timeout` and `--max-time`.
- Long install flows should emit progress events before and after each slow substep. Prefer parent steps plus child substeps so users can see whether a flow is detecting, installing, waiting for admin confirmation, or verifying.
- For device-code auth, let `codex login --device-auth` own the official login flow. The app should start it in the background, parse the URL/code from stdout/stderr, and return quickly. Do not reimplement OpenAI private auth endpoints unless official docs expose a stable API.
- Codex plugin enablement has two layers. This switcher owns only config-layer changes in `~/.codex/config.toml`, such as `[plugins."browser@openai-bundled"] enabled = true` and `[marketplaces.*]`. Codex++-style plugin entry unlock, marketplace unlock, and forced install require renderer injection from a Codex++ launcher and should not be implied by this app's config writer.
- When writing plugin config, use `toml_edit` and keep unrelated `config.toml` content intact. Always preserve the existing backup behavior before touching `~/.codex/config.toml`.
- After plugin config writes, give the same target-client refresh hint used for profile switching: restart Codex App, reload VS Code, or restart the CLI/other client.
- On Windows, PowerShell scripts should use process-scoped execution policy bypass: `Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force`, and command invocations should also include `-ExecutionPolicy Bypass` where applicable.
- On Windows, every non-interactive child process launched by the GUI app should use the shared hidden-command helper with `CREATE_NO_WINDOW`. This includes `powershell.exe`, `cmd.exe`, `taskkill`, `winget`, `notepad.exe`, `control.exe`, and `explorer.exe` launches that are not explicitly meant to show a terminal. UAC/admin prompts are allowed when elevation is required.
- Opening DNS/network settings is platform-specific and should be best effort:
  - Windows: `control.exe ncpa.cpl` for adapter/DNS properties, `explorer.exe ms-settings:network` for network settings.
  - macOS: `open x-apple.systempreferences:com.apple.Network-Settings.extension`, with Network.prefPane fallback.
  - Linux: try `gnome-control-center network`, `systemsettings kcm_networkmanagement`, then `nm-connection-editor`.
- Preserve user files. Before modifying `~/.codex/auth.json`, `~/.codex/config.toml`, or hosts files, keep the existing backup behavior intact.
- Prefer scoped changes that match the existing React/Tauri patterns. Avoid introducing new runtimes or background daemons for simple command launch and progress-reporting work.

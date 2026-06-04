# AGENTS

## Project Rules

- Keep Tauri commands responsive. Any command that can run external processes, touch the network, sleep, stop/start apps, open installers, write system files, or wait for user action must be `async` and move blocking work into `tauri::async_runtime::spawn_blocking`.
- Do not wait for GUI apps, system settings panels, installers, browsers, or control panels to exit. Use `Command::spawn()` for these and return after launch.
- Do not run long CLI flows synchronously from the UI thread. Device-code login, Codex install, winget/npm/brew commands, DNS/network diagnostics, app restart/quit, and hosts writes are blocking-risk flows.
- External network checks must have explicit timeouts. For example, `curl` checks should use both `--connect-timeout` and `--max-time`.
- Long install flows should emit progress events before and after each slow substep. Prefer parent steps plus child substeps so users can see whether a flow is detecting, installing, waiting for admin confirmation, or verifying.
- For device-code auth, let `codex login --device-auth` own the official login flow. The app should start it in the background, parse the URL/code from stdout/stderr, and return quickly. Do not reimplement OpenAI private auth endpoints unless official docs expose a stable API.
- On Windows, PowerShell scripts should use process-scoped execution policy bypass: `Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force`, and command invocations should also include `-ExecutionPolicy Bypass` where applicable.
- Opening DNS/network settings is platform-specific and should be best effort:
  - Windows: `control.exe ncpa.cpl` for adapter/DNS properties, `explorer.exe ms-settings:network` for network settings.
  - macOS: `open x-apple.systempreferences:com.apple.Network-Settings.extension`, with Network.prefPane fallback.
  - Linux: try `gnome-control-center network`, `systemsettings kcm_networkmanagement`, then `nm-connection-editor`.
- Preserve user files. Before modifying `~/.codex/auth.json`, `~/.codex/config.toml`, or hosts files, keep the existing backup behavior intact.
- Prefer scoped changes that match the existing React/Tauri patterns. Avoid introducing new runtimes or background daemons for simple command launch and progress-reporting work.

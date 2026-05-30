# Profile Switcher Notes

This project manages local Codex Desktop/CLI state under `~/.codex`.

Since v0.1.15 the installed app name is `Profile Switcher`, with bundle identifier `com.local.profile-switcher` and Windows executable name `profile-switcher.exe`. This avoids polluting Windows StartApps/launcher matching with another app named `Codex`. The tool still manages Codex state, and it intentionally keeps the existing local data folder named `codex-account-switcher` so old profiles/backups survive upgrades.

Windows packaging keeps the previous MSI upgrade code so upgrades replace the old install instead of creating a duplicate app. The NSIS installer also removes legacy Start menu shortcuts named `Codex Account Switcher` during install/uninstall.

## Auth Modes

Codex has two practical auth modes:

- `chatgpt`: ChatGPT account login, usually Plus/Pro/Team. The login tokens live in `~/.codex/auth.json` under `tokens`.
- `apikey`: API key auth. The API key lives in `~/.codex/auth.json` as `OPENAI_API_KEY`.

The Codex UI may show a logged-in account in the lower-left corner while the active model request still uses API key auth. The actual request path depends on both `auth.json` and `config.toml`.

## model_provider

`~/.codex/config.toml` controls which provider Codex uses for model requests.

Important cases:

- `model_provider = "openai"` means the built-in OpenAI provider.
- `model_provider = "OpenAI"` can be a custom provider if `[model_providers.OpenAI]` exists.
- A custom provider can set `base_url`, for example `https://code.gogoais.com`.

If `model_provider = "OpenAI"` and `[model_providers.OpenAI].base_url = "https://code.gogoais.com"`, Codex will send requests to that proxy even if the lower-left account menu shows a ChatGPT account. A 401 like `INVALID_API_KEY` usually means the API key used for that proxy is invalid or mismatched.

For API-key proxy profiles, the switcher writes both files together:

```toml
model_provider = "OpenAI"

[model_providers.OpenAI]
name = "OpenAI"
base_url = "https://code.gogoais.com/v1"
wire_api = "responses"
requires_openai_auth = true
```

```json
{
  "OPENAI_API_KEY": "sk-..."
}
```

The UI accepts a root Base URL such as `https://code.gogoais.com`; the saved provider URL is normalized to `/v1`.

## Key Files

- `~/.codex/config.toml`: provider, model, base URL, plugins, MCP servers, and other Codex config.
- `~/.codex/auth.json`: auth material. Either ChatGPT login tokens or `OPENAI_API_KEY`.
- `~/.codex/state_5.sqlite`: Codex Desktop thread metadata database.
- `~/.codex/sessions/`: active thread rollout JSONL files.
- `~/.codex/archived_sessions/`: archived rollout JSONL files.
- `~/.codex/session_index.jsonl`: lightweight active session index.
- `~/.codex/logs_2.sqlite`: logs, including 401/Unauthorized/INVALID_API_KEY details.
- `~/Library/Application Support/codex-account-switcher/profiles.json`: saved switcher profiles on macOS. Windows uses `%LOCALAPPDATA%\codex-account-switcher\profiles.json`; Linux uses the local data directory.
- `~/Library/Application Support/codex-account-switcher/backups/`: backups made before destructive writes on macOS. Windows/Linux use the same app-local data directory pattern.
- `/etc/hosts`: macOS/Linux local DNS override file.
- `C:\Windows\System32\drivers\etc\hosts`: Windows local DNS override file.

## Thread Database And Rollout Files

Codex stores a thread in two layers:

- `~/.codex/state_5.sqlite`: SQLite metadata database.
- `~/.codex/sessions/` and `~/.codex/archived_sessions/`: JSONL rollout files with the actual thread event stream.

`~/.codex/state_5.sqlite` contains the `threads` table. The table is the source of truth for the visible thread list in Codex Desktop. The JSONL file pointed to by each row is the source of truth for the conversation payload.

Fields that matter for switching:

- `id`: thread id.
- `title`: thread title.
- `rollout_path`: JSONL file path for the thread.
- `cwd`: workspace path.
- `archived`: active vs archived.
- `model`: selected model.
- `model_provider`: provider captured for the thread, such as `openai` or `OpenAI`.
- `updated_at`: recency.

Active threads usually have rollout files under `~/.codex/sessions/`. Archived threads usually have rollout files under `~/.codex/archived_sessions/`. `~/.codex/session_index.jsonl` is only a lightweight index; it can have fewer rows than `state_5.sqlite`, so do not use it as the total thread count.

To count local threads:

```sh
sqlite3 ~/.codex/state_5.sqlite \
  "select count(*) as total, sum(archived = 0) as active, sum(archived = 1) as archived from threads;"
```

To see which request path old threads are tied to:

```sh
sqlite3 ~/.codex/state_5.sqlite \
  "select coalesce(model_provider, '<null>') as provider, archived, count(*) from threads group by provider, archived order by provider, archived;"
```

## Account Threads vs API Threads

Account mode and API/proxy mode can share the same thread database while still having different request behavior.

- Account-mode threads normally use `model_provider = "openai"`.
- API/proxy-mode threads can use `model_provider = "OpenAI"` when `config.toml` defines `[model_providers.OpenAI]`.
- A thread can keep its captured `model_provider` even after `config.toml` changes.

This means a shared thread list can contain both account-style and API/proxy-style rows. For example, the user may see all 13 local threads in one list, while 3 active rows still say `OpenAI` and 2 active rows say `openai`. That is expected under the shared-thread policy: visibility is shared, but each thread row may still carry provider metadata from when it was created or last used.

If a shared thread opens but still routes like an API/proxy thread, check both:

- current `~/.codex/config.toml`;
- the thread row's `model_provider` in `~/.codex/state_5.sqlite`.

The switcher does not rewrite thread provider metadata during normal profile switching. It only swaps authorization/provider files.

## Shared Threads

The switcher uses one shared Codex thread store for account mode and API/proxy mode.

Switching profiles only writes the target profile's `auth.json` and `config.toml`. It does not move, replace, or isolate these live session paths:

- `sessions/`
- `archived_sessions/`
- `session_index.jsonl`
- `history.jsonl`
- `state_5.sqlite`
- `state_5.sqlite-shm`
- `state_5.sqlite-wal`
- `goals_1.sqlite`
- `goals_1.sqlite-shm`
- `goals_1.sqlite-wal`

This keeps ChatGPT account profiles and API/proxy profiles looking at the same thread list and the same rollout JSONL files. The intended behavior is:

- ChatGPT account profiles see the same account threads.
- API/proxy profiles also see those same account threads after login/switching.
- Switching mode does not hide, archive, copy, or restore threads.
- Saved switcher profiles contain only `auth.json`/`config.toml` snapshots plus labels/notes.
- The target client preference is stored in the switcher's `profiles.json` as `client_preference`. `codex_app` can close/reopen Codex App after writes. `vscode_extension` and `cli_other` only write `~/.codex` and then tell the user to reload VS Code or restart the CLI process.
- On Windows, `codex_app` detects and starts the desktop app through the Start menu AppID (`shell:AppsFolder\...`) instead of `start Codex`, because the plain command name can resolve to Codex CLI (`codex.cmd`/`codex.exe`) when CLI is on PATH. The detector prefers the official `OpenAI.Codex` package and excludes this switcher app (`Profile Switcher`, legacy `Codex Account Switcher`, and their app ids) so the switcher is not mistaken for the official Codex App.

## Reset Account State

Use `Reset Account State` when the current live Codex login/provider state should be discarded and rebuilt from scratch.

It should:

- stop Codex;
- back up `auth.json` and `config.toml`;
- delete both files;
- clear the switcher's active profile marker;
- restart Codex.

After reset, Codex will need a fresh ChatGPT login or API key configuration. Saved switcher profiles and shared threads are not deleted by this action.

## Local DNS hosts

The switcher has a `Local DNS hosts` entry for adding fixed host-to-IP mappings.

Behavior:

- macOS/Linux write `/etc/hosts`.
- Windows writes `C:\Windows\System32\drivers\etc\hosts`.
- Every write backs up the current hosts file under the switcher's `backups/hosts-<timestamp>/` directory.
- Entries written by the switcher include the `codex-account-switcher` marker comment.
- The delete button only removes entries managed by the switcher, not arbitrary manual hosts rows.
- If the same hostname already exists in a manual hosts row, the switcher refuses to overwrite it and asks the user to open hosts for manual cleanup.
- After a write, the app attempts to flush the DNS cache: `dscacheutil`/`mDNSResponder` on macOS, `ipconfig /flushdns` on Windows, and common resolver flush commands on Linux.

Permissions:

- macOS may show an administrator prompt when writing hosts.
- Windows usually requires launching the switcher as administrator before writing hosts.
- Linux usually requires root/admin permissions, or manual editing with `sudo`.

hosts is exact-name mapping only. It does not support wildcard domains such as `*.example.com`; add each specific hostname that needs the local DNS override.

## Install Codex Button

The `安装 Codex` button checks and installs the local Codex toolchain, then writes a step-by-step report into the same expandable result panel used by environment diagnostics.

Windows flow:

- If `winget` is missing, the switcher launches an elevated PowerShell repair script with:
  `Set-ExecutionPolicy -Scope Process Bypass -Force`,
  `Install-PackageProvider -Name NuGet -Force`,
  `Install-Module -Name Microsoft.WinGet.Client -Force -Repository PSGallery`,
  and `Repair-WinGetPackageManager -AllUsers`.
- If Node.js is missing, it runs `winget install --id OpenJS.NodeJS.LTS -e --accept-package-agreements --accept-source-agreements`.
- If Codex CLI is missing, it first runs `npm install -g @openai/codex`; if npm is unavailable or fails, it falls back to OpenAI's Windows install script: `irm https://chatgpt.com/codex/install.ps1 | iex`.
- If Codex App is missing, it runs the official Windows app command `winget install Codex -s msstore --accept-package-agreements --accept-source-agreements`.

macOS flow:

- If Node.js is missing and Homebrew is available, it runs `brew install node`.
- If Codex CLI is missing, it runs `npm install -g @openai/codex`, then falls back to OpenAI's install script: `curl -fsSL https://chatgpt.com/codex/install.sh | sh`.
- If Codex App is missing, it downloads the official OpenAI DMG for Apple Silicon or Intel and opens it for the user to finish installation.

Linux flow:

- If Node.js is missing, it tries common distro package managers (`apt-get`, `dnf`, `yum`, `pacman`, `zypper`) when root or passwordless sudo is available.
- If Codex CLI is missing, it runs `npm install -g @openai/codex`, then falls back to OpenAI's install script.
- Codex App is currently reported as manual/unsupported because OpenAI's Codex App download page only lists macOS and Windows installers.

Some installers update PATH only for new processes. A warning result can still mean the install succeeded but the switcher must be restarted before `node`, `npm`, or `codex` is detected.

## gogoais API Key Fetch

The `New Proxy` form can fetch a Codex proxy API key from gogoais by username and password.

Endpoint:

```text
GET https://x-api.gogoais.com/api/public/codex-key?username=<username>&password=<password>
```

Expected response fields:

- `data.codex.api_key`: key written into the form's API Key field.
- `data.codex.sk`: fallback key if `api_key` is missing.
- `data.codex.base_url`: preferred Base URL written into the form.
- `data.codex.openai_base_url`: fallback Base URL.
- `data.codex.api_key_name`, `data.codex.expires_at`, `data.service.status`: shown as feedback only.

The switcher does not store the gogoais username/password in profiles. The password is cleared from the UI after a successful fetch. Saved proxy profiles still contain only the resulting `auth.json`/`config.toml` snapshot.

After a successful gogoais fetch, the form automatically switches to `只用 API Key` so saving the proxy profile writes the custom provider block and `OPENAI_API_KEY` instead of account-mode `openai_base_url`.

## Online Updates

The switcher uses the official Tauri v2 updater path:

- frontend API: `@tauri-apps/plugin-updater`;
- restart API: `@tauri-apps/plugin-process`;
- runtime permissions: `src-tauri/capabilities/default.json`;
- release-only updater config: `src-tauri/tauri.updater.conf.json`;
- GitHub Actions release workflow: `.github/workflows/release.yml`.

The normal local build still uses:

```sh
npm run tauri:build
```

The updater build uses:

```sh
npm run tauri:build:updater
```

The updater build creates updater archives and `.sig` signatures, so it requires a Tauri updater private key.

### Generate Updater Keys

Run once on a trusted machine:

```sh
npm run tauri signer generate -- --write-keys updater.key
```

The command prints a public key and writes the private key to `updater.key`.

Current local updater public key:

```text
dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDc5QTQ2OEVFMzlCMTk5NzcKUldSM21iRTU3bWlrZVI2WGxJb2ZnZE9Ic2lmenF2L2grSnNYTC9ndlRBZ3k5ZFNTanpKd1hjM00K
```

Rules:

- put the public key in `src-tauri/tauri.conf.json` and `src-tauri/tauri.updater.conf.json`;
- put the private key in GitHub Secret `TAURI_SIGNING_PRIVATE_KEY`;
- put the key password in GitHub Secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`;
- do not commit `updater.key` or `updater.key.pub`.

### macOS Signing And Notarization

macOS may show `"Profile Switcher.app" is damaged and can't be opened` when the app is not signed with an Apple Developer ID certificate and notarized by Apple. This is Gatekeeper blocking the app, not necessarily a corrupt DMG.

Current release builds always use Tauri updater signatures. For normal macOS distribution, also configure Apple signing/notarization.

Requirements:

- Apple Developer Program membership, normally 99 USD/year.
- A `Developer ID Application` certificate.
- An Apple ID app-specific password for notarization, or equivalent App Store Connect credentials.
- GitHub Secrets configured for the release workflow.

GitHub Secrets used by `.github/workflows/release.yml`:

- `APPLE_CERTIFICATE`: base64-encoded `.p12` Developer ID Application certificate.
- `APPLE_CERTIFICATE_PASSWORD`: password for the `.p12` certificate.
- `APPLE_SIGNING_IDENTITY`: signing identity name, for example `Developer ID Application: Your Name (TEAMID)`.
- `APPLE_ID`: Apple ID email used for notarization.
- `APPLE_PASSWORD`: app-specific password for that Apple ID.
- `APPLE_TEAM_ID`: Apple Developer Team ID.

Example certificate export:

```sh
security find-identity -v -p codesigning
security export -t identities -f pkcs12 \
  -o developer-id-application.p12 \
  -k ~/Library/Keychains/login.keychain-db
base64 -i developer-id-application.p12 | pbcopy
```

Paste the base64 output into the `APPLE_CERTIFICATE` GitHub Secret. Store the `.p12` export password in `APPLE_CERTIFICATE_PASSWORD`.

Local verification after a signed release:

```sh
spctl --assess --verbose=4 --type execute "/Applications/Profile Switcher.app"
codesign -dv --verbose=4 "/Applications/Profile Switcher.app"
```

An unsigned local workaround for trusted self-use is:

```sh
xattr -dr com.apple.quarantine "/Applications/Profile Switcher.app"
open "/Applications/Profile Switcher.app"
```

Do not present the workaround as the normal install path for public users; public macOS releases should be signed and notarized.

### Configure GitHub Releases

The app is configured to read release metadata from:

```text
https://github.com/FerryboatSeranade/codex-account-switcher/releases/latest/download/latest.json
```

`src-tauri/tauri.conf.json` contains the runtime updater endpoint. `src-tauri/tauri.updater.conf.json` enables signed updater artifacts during release builds.

If the GitHub repository name changes, update both files and the release workflow target repository.

### Release Flow

1. Bump versions in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.
2. Commit the changes.
3. Create and push a tag, for example `v0.1.15`.
4. GitHub Actions builds installers, updater archives, signatures, and release metadata.
5. Review the draft release, then publish it.

The app checks for updates automatically every 24 hours after startup. It stores the last automatic check timestamp in browser `localStorage` under `codex-account-switcher:last-auto-update-check`. If no update is available, the automatic check stays quiet; if a newer version is found, it shows the update panel so the user can install it.

The app's `检查更新` button still checks the configured `latest.json` immediately. If a newer version is found, it shows release notes, downloads the signed updater package, installs it, and relaunches the switcher.

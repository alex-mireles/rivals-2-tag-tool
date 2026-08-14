# Rivals II Tag Tool

Tauri v2 desktop app (Windows/macOS) for sharing Rivals of Aether II player tags + custom controls — via a start.gg-backed cloud service, or as local `.r2tag` files and `.r2pack` archives. Vue 3 + TypeScript frontend, Rust backend.

## Commands

- `pnpm install` — install frontend deps
- `pnpm tauri dev` — run the app with hot reload
- `pnpm tauri build` — release build + installer
- `pnpm lint` — ESLint over `src/`
- `pnpm build` — type-check (`vue-tsc --noEmit`) + Vite build; run this to type-check the frontend
- `cargo test --manifest-path src-tauri/Cargo.toml` — backend tests. `uesave`'s types can't be built outside their crate, so export, renamed-save validation, and replace-custom round-trip coverage have `#[ignore]`d tests that run against a real save: add `-- --ignored` with `R2_SAVE` set to a valid Rivals II tag `.sav`.
- `pnpm --filter rivals-2-tag-tool-infra test` — cloud API tests (vitest, `infra/`)

`pnpm build` requires `VITE_CLOUD_API_BASE_URL` in `process.env` (CI supplies it; `.env.local` only reaches `import.meta.env`, so a bare local `pnpm build` fails by design).

## Viewing the UI

**Never open `localhost:1420` in a browser, and never point a browser-preview tool at it.** Port 1420 is not a browsable build of this app — it exists solely for the Tauri window to load from. `App.vue` calls Tauri APIs during setup, so outside the Tauri webview it throws before anything renders (`Unhandled error during execution of setup function at <App>`) and you get a blank page. No amount of retrying, reloading, or reconfiguring changes that; there is no browser path to this UI.

To see a change, look at the real window:

- `pnpm tauri dev` runs it, and it hot-reloads on save — frontend edits need no rebuild.
- Use computer use to screenshot and drive that window. Request access to `rivals-2-tag-tool.exe`; the grant resolves to `src-tauri/target/debug/rivals-2-tag-tool.exe`.
- The Start-menu entry "Rivals II Tag Tool" is the *installed release build*, usually an older version — opening it by name gets you the wrong app. Check the version under the title to confirm which one you're looking at.
- The card is vertically centred and animates its height, so it shifts between screens. Re-screenshot after every navigation instead of reusing coordinates.

For anything the window can't show (computed styles, overflow measurements), read the source or reason about the CSS — don't reach for the browser.

## Architecture

- Frontend: three views in `src/views/` — `HomeView` → `GetTagsView` / `ShareTagsView`. Each of the two leaf views puts the cloud path in its first, default tab and local files in the last.
  - Shared state lives in module-singleton composables in `src/composables/`: `useSaveFile` (the one save file; exposes read-only refs, mutate via actions), `useCloudAuth` (session survives navigation), `useCloudSearch` (cancellable tournament scan), `useStagedTags` (cache files pending import), `useAppUpdate` (new-release banner and install).
  - Because those composables are module-scoped they have **no component lifecycle** — `GetTagsView` must cancel the scan and clean staged files in its own `onBeforeUnmount`.
  - Components in `src/components/`, types in `src/types.ts`, cloud config in `src/cloud.ts`.
- Backend, all registered in `src-tauri/src/lib.rs`:
  - `commands/save_file.rs` — `resolve_save_file` (resolve + read tags in one call; also serves as reload) and `set_save_path`. Everyday problems come back as a `SaveStatus`, not an `Err`; `Err` is reserved for a broken environment.
  - `commands/tags.rs` — `get_tag_names`, `get_tag_previews`, `export_tags`, `import_tags`.
  - `commands/archive.rs` — `pack_tags_from_save`, `pack_tag_files`, `unpack_r2pack`.
  - `commands/cloud.rs` — start.gg auth, search, upload/download, staging cleanup.
  - `commands/update.rs` — `check_for_update`, `install_update`.
  - `settings.rs` — persisted preferences.
- Save file parsing uses the `uesave` crate. The save is `Rivals2_PlayerTagSaveSlot.sav` (Windows default under `%LOCALAPPDATA%\Rivals2\Saved\SaveGames\`), resolved and read automatically at startup.

## Comments

Most lines need no comment. Write one only where a reader would otherwise stop and wonder — an order that matters, a workaround, a choice that looks wrong until explained. If someone would read the line and simply trust it, say nothing.

- Explain **why**, not what. The code already says what it does.
- Write for someone who doesn't know these tools. Don't stack jargon — a sentence carrying three tool names and an acronym only reads back to whoever wrote it.
- Say what you are talking about. "macOS is notify-only" never says notify-only *about what*.
- One or two lines. A three-line comment above a one-line statement reads strangely. Module-level docs (`//!`) can run longer.
- Comments that prevent a bug earn their place: an ordering constraint, a trap, a name that must not be "corrected". Ones that only restate intent are the first to cut.
- Long reasoning belongs in this file, not in the source. A comment can point here instead.

This one was deleted, because nobody reading a build script stops to question that line — the comment invented a question the reader never had:

```sh
# `sed -n 1p` not `head -1`: head exits early, and the SIGPIPE that
# sends upstream is what `pipefail` turns into a failed step.
dmg=$(find src-tauri/target -name '*.dmg' | sort | sed -n '1p')
```

This one stayed, because getting it wrong silently breaks releases — and it names the trap in words that don't assume you know either tool:

```sh
# Careful: the file is named "x86_64" but the exe inside it is named
# "x64". Both are correct — the app looks for the first spelling, and
# tauri-action produces the second. Don't make them match.
```

## Invariants

- Exporting, packing, and uploading only read the save; importing rewrites it. Never write to the `.sav` outside the import path.
- `import_tags` writes a sibling temp file and renames over the original. Never truncate the user's save before a successful write.
- The game's built-in tags (Player1–Player4) are excluded from listing, export **and import**. Import needs its own guard: the conflict list is built from the custom-tag names, so without one a hand-made `.r2tag` named `Player1` reads as "New" and silently replaces a profile the UI can neither show nor restore.
- Import conflicts default to Skip; user can opt into Overwrite per tag.
- Rivals II supports 96 custom tags plus Player1–Player4. The game blocks creating the 97th custom tag but does not enforce the limit in the save format, so every import mode must reject a final set above 96 before writing.
- Replace-custom imports preserve the destination's existing Player1–Player4 structs and leave only the selected custom tags. They are all-or-nothing and require a byte-for-byte backup before the destination is replaced.
- Cross-version imports are rejected: a `.r2tag`'s root `SaveVersion` must equal the destination save's.
- Exported filenames sanitize characters invalid on Windows/macOS to `_` and drop bidi/zero-width characters; the tag name inside the `.r2tag` stays unchanged.
- A `.r2tag` contains only one tag name + its control settings — nothing else from the save or system. It is a complete save carrying exactly one tag, so it is self-describing.
- Cloud browsing and saving a `.r2pack` must work with **no save file loaded** — the machine downloading tags may not have the game installed. Only import, export, and publish are gated on `canWriteSave`.
- Every screen that can finish with nothing to show must say so. `<ImportReview>` is gated on `previews.length`, so anything it hosts (the pack banners included) disappears when a selection yields no previews — the caller has to cover that case itself.

### `.r2pack` archives

- A zip containing `manifest.json` + N `.r2tag` files, for the tournament-organiser workflow (download a bracket once, import onto every setup from a USB stick).
- `manifest.json` has an **integer** `formatVersion`; a pack declaring a higher version is refused outright. A missing or corrupt manifest falls back to reading the `.r2tag` entries directly — it must never be fatal.
- Unpacking treats every archive as hostile: on-disk names are derived solely from a per-run token, entry index, and content hash. **Never** join an archive-provided name onto a path — that is what removes zip-slip as a category.
- `entry.size()` is attacker-controlled; reads are capped independently of it. Caps: 64 MiB archive, 512 entries, 8 MiB per entry, 128 MiB total.
- A malicious `.r2tag` cannot inject properties outside its own tag struct (`import_tags` clones exactly one `StructValue`), but it can put arbitrary data *inside* it, which the game then reads. This is equally true of cloud downloads and loose `.r2tag` files; packs do not widen it.

### Cloud and settings

- `settings.json` (app config dir) holds **only** `savePath`, written atomically. Never persist the cloud session token — the app runs on shared tournament PCs where a plaintext bearer token would be readable by any process.
- Staged tag files (cloud downloads and pack extractions alike) share `app_cache_dir()/cloud-tags/` and the same cleanup commands; a 24h sweep runs at startup. Only the frontend tracks staged paths, and it never sees them when a command returns `Err` — so a command that writes staged files must clean up its own partial output before failing.
- HTTP throttling (429/503) comes back as the `RATE_LIMITED` sentinel rather than a message, because the tournament scan retries on it. `src/cloud.ts` mirrors the constant and owns the human-readable wording; every cloud error surface goes through `describeCloudError`.
- The `GET /v1/tournaments/tags` route throttle (1 rps) is calibrated to start.gg's own ~80 req/min token limit, not to the client. It is stage-wide, so concurrent scans *will* see 429s; that is handled by client backoff and by keeping partial results, not by raising the limit.
- `tauri.conf.json` sets a real CSP. `style-src` needs `'unsafe-inline'` because `oh-vue-icons` injects a `<style>` element at runtime, and `img-src` needs `data:` because Vite inlines `startgg.svg`. The CSP applies only to `frontendDist`, so `tauri dev` never exercises it — verify changes with `tauri build --debug --no-bundle` and run the resulting exe.
- A `.r2pack` is offline redistribution that outlives cloud deletion — the publish disclosure says so.

### In-app updates

- Tauri's updater plugin is **not** used. Its Windows install step only runs an NSIS or MSI installer, and this build has neither (`--no-bundle`, `bundle.active: false`). Adding an installer to satisfy it would be a product change, not a build change — the same reason `signCommand` is unusable.
- Windows replaces the running exe via `self-replace`. Windows refuses to open a running image for writing but *does* allow renaming one: the live exe is moved aside, the new one takes its path, and the old handle deletes itself at exit. Nothing is removed before the replacement is on disk and verified, so a failed update leaves a working app.
- After the swap, `current_exe()` still resolves to the original path — which now holds the new binary — so `app.restart()` relaunches the version just installed.
- macOS is **notify-only**, because the shapes differ, not for any trust reason: Windows ships one portable exe that can be swapped, while the mac build is a `.app` inside a `.dmg`. The mac banner opens the releases page.
- The security gate is TLS to a hard-coded `github.com` host. The manifest's SHA-256 is an *integrity* gate only — it travels the same channel as the binary, so it proves nothing about authorship; what it prevents is replacing the app with a truncated download or a CDN error page and restarting into nothing. `sha256` is deliberately not `#[serde(default)]`: a manifest without one must fail, not install unverified bytes.
- Verifying the exe's Authenticode signature was considered and rejected. The Azure signing credentials live in the same repository secrets that serve the release, so a compromise reaching one reaches the other; and a signer pinned at compile time would strand every installed copy if the certificate's validated identity ever changed. Releases are still signed — the app just doesn't re-check it.
- Downloads go through `reqwest`, not a browser, so the replacement carries no mark-of-the-web and starts without a SmartScreen prompt.
- `check_for_update` probes whether the install directory is writable and reports `canSelfInstall`. The app gets run from USB sticks and from locked-down `Program Files` installs; both fail to write, and the banner has to offer the releases page instead of a button that cannot work.
- Update-check failures are swallowed. A machine that is offline or behind a venue firewall gives the user nothing to act on.
- Banner dismissal is **in-memory only**, which is why `settings.json` still holds only `savePath`. On a shared tournament PC, one person clicking "Later" must not silence the notice for whoever sits down next.
- `R2_UPDATE_MANIFEST_URL` overrides the manifest URL for local testing. It is `#[cfg(debug_assertions)]`, so the `env::var` call is not compiled into a release build at all — an environment variable that can retarget the updater is a privilege-escalation primitive on a shared PC, and the point is that there is nothing there to set. Works under `tauri dev` and under `tauri build --debug`, which is the better one to exercise: real `frontendDist`, real CSP.
- It overrides the *manifest* only. `check_download_url` still demands `https://github.com/…` for the binary in debug builds too, so a fully local test is impossible by design — relaxing what a debug build will execute is the one change that would make the override genuinely dangerous. Point `url` at a real published asset, paired with that asset's real SHA-256, save as `latest-windows-x86_64.json`, and serve the directory with `python -m http.server 8787`:

  ```json
  {
    "version": "9.9.9",
    "url": "https://github.com/alex-mireles/rivals-2-tag-tool/releases/download/v2.1.1/Rivals-II-Tag-Tool_2.1.1_windows_x64.exe",
    "sha256": "c373df4b9c93db4b7c2cffda52abc6968e6adc5dc1620a686272af327f92bacc",
    "pubDate": "2026-08-11T23:46:03Z"
  }
  ```

  Then `R2_UPDATE_MANIFEST_URL=http://127.0.0.1:8787/latest-windows-x86_64.json pnpm tauri dev`. Vary one field per run: a wrong `sha256` digit exercises the mismatch banner, `version: 0.0.1` confirms an older release offers nothing, and stopping the server confirms an unreachable manifest stays silent.
- A successful test run really does replace `target/debug/rivals-2-tag-tool.exe` with whatever it was pointed at — run `cargo build` afterwards to get the dev binary back. `tauri dev` also exits when its binary is swapped out from under it; that is the expected ending, not a failure.

## Versioning / Release

- Version lives in `package.json` and `src-tauri/Cargo.toml` (tauri.conf.json reads it from package.json). Bump both together. The UI reads `APP_VERSION`, injected from package.json by `vite.config.ts` — nothing hardcodes a version string.
- Pushing a `v*` tag triggers `.github/workflows/release.yml`, which builds Windows + macOS (ARM) installers.
- `tauri-action` owns the release entirely — creation, notes, and asset names. Don't add a second thing that creates releases: GitHub permits multiple drafts sharing a tag, drafts aren't retrievable via `GET /releases/tags/{tag}`, and `tauri-action` never rewrites an existing draft's body. Every one of those is a trap that a second creator walks into.
- Asset names come from `releaseAssetNamePattern`. `[arch]` renders as `x64`/`aarch64` and `[platform]` as `windows`/`darwin`. The signing step re-uploads `Rivals-II-Tag-Tool_<version>_windows_x64.exe` with `--clobber`, so that name has to keep matching the pattern's output — a mismatch silently adds a second asset instead of replacing the unsigned one.
- Each matrix leg publishes its own updater manifest — `latest-windows-x86_64.json`, `latest-darwin-aarch64.json` — rather than sharing one `latest.json`. The legs finish in either order, so a shared file would need one to read and merge the other's upload: a race, and a second thing reaching into the release.
- **Two arch spellings are in play and are not interchangeable.** The manifest is *named* for Rust's `std::env::consts::ARCH` (`x86_64`), because that is what the app uses to build the URL it fetches; the asset named *inside* it uses tauri-action's `[arch]` (`x64`).
- The Windows manifest must be written **after** the signing step — its SHA-256 has to describe the bytes users actually download.
- The updater reads `/releases/latest/download/...`, which resolves only to the newest **published, non-prerelease** release. The draft `tauri-action` creates is therefore a natural staging gate: an update reaches users when the release is published, not when it is built. Serving from the release CDN rather than `api.github.com` also dodges the 60-requests-per-hour unauthenticated API limit, which a venue running a dozen setups behind one NAT could plausibly hit.

### Windows code signing

- The Windows exe is signed with **Azure Artifact Signing** (formerly Trusted Signing; the SDK still uses the old `codesigning` spelling in keys, endpoints, and dll names — don't "fix" those).
- Tauri's `bundle.windows.signCommand` is unusable here: signing runs inside the bundler's per-package-type loop, and the Windows build is gated out of the bundler by both `--no-bundle` and `bundle.active: false` in `tauri.windows.conf.json`. Signing is therefore its own workflow step. Re-enabling bundling to use `signCommand` would also start shipping an NSIS installer, which is a product change, not a build change.
- Signing is a `matrix.platform == 'windows-latest'` step *after* `tauri-action`, not something wired into the build: the signing Action is a composite Action, so it has to be a workflow step and can't run inside `tauri-action`. The draft therefore holds an unsigned exe for a minute before the signed one clobbers it — drafts aren't downloadable, so this is invisible to users.
- Certificates are valid ~3 days, so RFC3161 timestamping is mandatory. The job runs `signtool verify /pa` and fails the build rather than shipping an unsigned or untimestamped exe.
- CI authenticates as a service principal via `AZURE_TENANT_ID` / `AZURE_CLIENT_ID` / `AZURE_CLIENT_SECRET` repo secrets. `DefaultAzureCredential` takes `EnvironmentCredential` first, which is what makes this deterministic — a personal Microsoft account resolves to the consumer tenant and fails.
- `src-tauri/artifact-signing.json` is for **local** signing only (`signtool /dmdf`) and is gitignored, so a fresh clone has to recreate it — CI never reads it and passes the same values as Action inputs instead. Changing region, account, or profile means editing both. Shape:

  ```json
  {
    "Endpoint": "https://eus.codesigning.azure.net/",
    "CodeSigningAccountName": "hyperflame-codesign",
    "CertificateProfileName": "rivals-2-tag-tool"
  }
  ```

  Local signing authenticates through `DefaultAzureCredential`, which ignores the tenant `az login` is using. Set `AZURE_TENANT_ID` / `AZURE_CLIENT_ID` / `AZURE_CLIENT_SECRET` in the shell so `EnvironmentCredential` wins; a personal Microsoft account otherwise resolves to the consumer tenant and fails.

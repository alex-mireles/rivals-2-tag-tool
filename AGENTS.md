# Rivals II Tag Tool

Tauri v2 desktop app (Windows/macOS) for sharing Rivals of Aether II player tags + custom controls — via a start.gg-backed cloud service, or as local `.r2tag` files and `.r2pack` archives. Vue 3 + TypeScript frontend, Rust backend.

## Commands

- `pnpm install` — install frontend deps
- `pnpm tauri dev` — run the app with hot reload
- `pnpm tauri build` — release build + installer
- `pnpm lint` — ESLint over `src/`
- `pnpm build` — type-check (`vue-tsc --noEmit`) + Vite build; run this to type-check the frontend
- `cargo test --manifest-path src-tauri/Cargo.toml` — backend tests. `uesave`'s types can't be built outside their crate, so the export path has one `#[ignore]`d test that runs against a real save: add `-- --ignored` with `R2_SAVE` set to a `Rivals2_PlayerTagSaveSlot.sav`.
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
  - Shared state lives in module-singleton composables in `src/composables/`: `useSaveFile` (the one save file; exposes read-only refs, mutate via actions), `useCloudAuth` (session survives navigation), `useCloudSearch` (cancellable tournament scan), `useStagedTags` (cache files pending import).
  - Because those composables are module-scoped they have **no component lifecycle** — `GetTagsView` must cancel the scan and clean staged files in its own `onBeforeUnmount`.
  - Components in `src/components/`, types in `src/types.ts`, cloud config in `src/cloud.ts`.
- Backend, all registered in `src-tauri/src/lib.rs`:
  - `commands/save_file.rs` — `resolve_save_file` (resolve + read tags in one call; also serves as reload) and `set_save_path`. Everyday problems come back as a `SaveStatus`, not an `Err`; `Err` is reserved for a broken environment.
  - `commands/tags.rs` — `get_tag_names`, `get_tag_previews`, `export_tags`, `import_tags`.
  - `commands/archive.rs` — `pack_tags_from_save`, `pack_tag_files`, `unpack_r2pack`.
  - `commands/cloud.rs` — start.gg auth, search, upload/download, staging cleanup.
  - `settings.rs` — persisted preferences.
- Save file parsing uses the `uesave` crate. The save is `Rivals2_PlayerTagSaveSlot.sav` (Windows default under `%LOCALAPPDATA%\Rivals2\Saved\SaveGames\`), resolved and read automatically at startup.

## Invariants

- Exporting, packing, and uploading only read the save; importing rewrites it. Never write to the `.sav` outside the import path.
- `import_tags` writes a sibling temp file and renames over the original. Never truncate the user's save before a successful write.
- The game's built-in tags (Player1–Player4) are excluded from listing, export **and import**. Import needs its own guard: the conflict list is built from the custom-tag names, so without one a hand-made `.r2tag` named `Player1` reads as "New" and silently replaces a profile the UI can neither show nor restore.
- Import conflicts default to Skip; user can opt into Overwrite per tag.
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

## Versioning / Release

- Version lives in `package.json` and `src-tauri/Cargo.toml` (tauri.conf.json reads it from package.json). Bump both together. The UI reads `APP_VERSION`, injected from package.json by `vite.config.ts` — nothing hardcodes a version string.
- Pushing a `v*` tag triggers `.github/workflows/release.yml`, which builds Windows + macOS (ARM) installers.
- The workflow is three jobs, not one matrix. `create-release` makes the draft up front so both platform jobs upload into a release that already exists; previously each matrix leg created it after building, which only avoided a 422 collision because build times differed by minutes. Keep that ordering if you touch the workflow.
- Asset names come from `releaseAssetNamePattern`. `[arch]` renders as `x64`/`aarch64` and `[platform]` as `windows`/`darwin` — the Windows job builds its own name by hand, so it has to match: `Rivals-II-Tag-Tool_<version>_windows_x64.exe`.

### Windows code signing

- The Windows exe is signed with **Azure Artifact Signing** (formerly Trusted Signing; the SDK still uses the old `codesigning` spelling in keys, endpoints, and dll names — don't "fix" those).
- Tauri's `bundle.windows.signCommand` is unusable here: signing runs inside the bundler's per-package-type loop, and the Windows build is gated out of the bundler by both `--no-bundle` and `bundle.active: false` in `tauri.windows.conf.json`. Signing is therefore its own workflow step. Re-enabling bundling to use `signCommand` would also start shipping an NSIS installer, which is a product change, not a build change.
- Windows is a separate job from macOS because the signing step is a composite Action and must be a workflow step — it can't be invoked from inside `tauri-action`. Both platform jobs depend only on `create-release`, so a macOS failure can't skip the Windows build.
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
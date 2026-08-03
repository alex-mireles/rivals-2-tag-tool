# Rivals II Tag Tool

Tauri v2 desktop app (Windows/macOS) for sharing Rivals of Aether II player tags + custom controls — via a start.gg-backed cloud service, or as local `.r2tag` files and `.r2pack` archives. Vue 3 + TypeScript frontend, Rust backend.

## Commands

- `pnpm install` — install frontend deps
- `pnpm tauri dev` — run the app with hot reload
- `pnpm tauri build` — release build + installer
- `pnpm lint` — ESLint over `src/`
- `pnpm build` — type-check (`vue-tsc --noEmit`) + Vite build; run this to type-check the frontend
- `cargo test --manifest-path src-tauri/Cargo.toml` — backend tests
- `pnpm --filter rivals-2-tag-tool-infra test` — cloud API tests (vitest, `infra/`)

`pnpm build` requires `VITE_CLOUD_API_BASE_URL` in `process.env` (CI supplies it; `.env.local` only reaches `import.meta.env`, so a bare local `pnpm build` fails by design).

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
- The game's built-in tags (Player1–Player4) are excluded from listing/export.
- Import conflicts default to Skip; user can opt into Overwrite per tag.
- Cross-version imports are rejected: a `.r2tag`'s root `SaveVersion` must equal the destination save's.
- Exported filenames sanitize characters invalid on Windows/macOS to `_` and drop bidi/zero-width characters; the tag name inside the `.r2tag` stays unchanged.
- A `.r2tag` contains only one tag name + its control settings — nothing else from the save or system. It is a complete save carrying exactly one tag, so it is self-describing.
- Cloud browsing and saving a `.r2pack` must work with **no save file loaded** — the machine downloading tags may not have the game installed. Only import, export, and publish are gated on `canWriteSave`.

### `.r2pack` archives

- A zip containing `manifest.json` + N `.r2tag` files, for the tournament-organiser workflow (download a bracket once, import onto every setup from a USB stick).
- `manifest.json` has an **integer** `formatVersion`; a pack declaring a higher version is refused outright. A missing or corrupt manifest falls back to reading the `.r2tag` entries directly — it must never be fatal.
- Unpacking treats every archive as hostile: on-disk names are derived solely from a per-run token, entry index, and content hash. **Never** join an archive-provided name onto a path — that is what removes zip-slip as a category.
- `entry.size()` is attacker-controlled; reads are capped independently of it. Caps: 64 MiB archive, 512 entries, 8 MiB per entry, 128 MiB total.
- A malicious `.r2tag` cannot inject properties outside its own tag struct (`import_tags` clones exactly one `StructValue`), but it can put arbitrary data *inside* it, which the game then reads. This is equally true of cloud downloads and loose `.r2tag` files; packs do not widen it.

### Cloud and settings

- `settings.json` (app config dir) holds **only** `savePath`, written atomically. Never persist the cloud session token — the app runs on shared tournament PCs where a plaintext bearer token would be readable by any process.
- Staged tag files (cloud downloads and pack extractions alike) share `app_cache_dir()/cloud-tags/` and the same cleanup commands; a 24h sweep runs at startup.
- A `.r2pack` is offline redistribution that outlives cloud deletion — the publish disclosure says so.

## Versioning / Release

- Version lives in `package.json` and `src-tauri/Cargo.toml` (tauri.conf.json reads it from package.json). Bump both together.
- Pushing a `v*` tag triggers `.github/workflows/release.yml`, which builds Windows + macOS (ARM) installers.
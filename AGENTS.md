# Rivals II Tag Tool

Tauri v2 desktop app (Windows/macOS) for exporting and importing Rivals of Aether II player tags + custom controls as `.r2tag` files. Vue 3 + TypeScript frontend, Rust backend.

## Commands

- `pnpm install` — install frontend deps
- `pnpm tauri dev` — run the app with hot reload
- `pnpm tauri build` — release build + installer
- `pnpm lint` — ESLint over `src/`
- `pnpm build` — type-check (`vue-tsc --noEmit`) + Vite build; run this to type-check the frontend
- `cargo test --manifest-path src-tauri/Cargo.toml` — backend tests

## Architecture

- Frontend: three views in `src/views/` (LoadView → ExportView / ImportView), shared components in `src/components/`, types in `src/types.ts`.
- Backend: Tauri commands in `src-tauri/src/commands/tags.rs` (`get_tag_names`, `get_tag_previews`, `export_tags`, `import_tags`) and `src-tauri/src/commands.rs` (`get_default_save_path`), registered in `src-tauri/src/lib.rs`.
- Save file parsing uses the `uesave` crate. The save is `Rivals2_PlayerTagSaveSlot.sav` (Windows default under `%LOCALAPPDATA%\Rivals2\Saved\SaveGames\`).

## Invariants

- Exporting only reads the save; importing rewrites it. Never write to the `.sav` outside the import path.
- The game's built-in tags (Player1–Player4) are excluded from listing/export.
- Import conflicts default to Skip; user can opt into Overwrite per tag.
- Exported filenames sanitize characters invalid on Windows/macOS to `_`; the tag name inside the `.r2tag` stays unchanged.
- A `.r2tag` contains only one tag name + its control settings — nothing else from the save or system.

## Versioning / Release

- Version lives in `package.json` and `src-tauri/Cargo.toml` (tauri.conf.json reads it from package.json). Bump both together.
- Pushing a `v*` tag triggers `.github/workflows/release.yml`, which builds Windows + macOS (ARM) installers.
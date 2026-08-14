# Rivals II Tag Tool

Sitting down at a new tournament setup, creating a tag, and setting your custom controls can be tedious.

Export, share, and import Rivals of Aether II tags & custom controls. Stop re-entering your tag and custom controls at every tournament setup!

Runs on **Windows** and **macOS**.

## Download

Each GitHub release includes a portable Windows `.exe` that runs without
installation and a macOS disk image. There is no Windows installer; run the
downloaded `.exe` directly.

The portable build uses the Microsoft WebView2 Runtime included with Windows 10
and Windows 11. If it is missing from your system, install the
[WebView2 Evergreen Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/consumer/).

## Usage

Your save file is found and loaded automatically on startup. If your save lives somewhere unusual or has been renamed, point the app at any valid Rivals II tag `.sav` once and it remembers.

There are two screens.

**Get Tags** — bring tags onto this PC:

- **Find Player** — look up an exact username or start.gg profile.
- **Find Tournament** — scan a bracket and grab every entrant who has uploaded a tag.
- **From Files** — import `.r2tag` files or a `.r2pack` archive. Works with no internet.

**Share Tags** — send your tags out:

- **Publish to Cloud** — sign in with start.gg and publish one canonical tag. This is the easiest way to have your tag waiting for you at every setup.
- **Export to Files** — write `.r2tag` files to a folder, or bundle them into a single `.r2pack`.

**Tournament Organizers:** scan your bracket under **Find Tournament**, hit **Save as .r2pack**, and put the file on a USB stick. On each setup: **Get Tags → From Files → pick the pack → Import**. You only need internet on the machine that builds the pack, so the setups themselves can stay offline.

Cloud files are gzip-compressed before upload and are checked against their hash locally before the game save is modified.

## Contributing / Development Setup

The app is built with [Tauri v2](https://v2.tauri.app/), using a Vue 3 + TypeScript frontend and a Rust backend.

### Prerequisites

- [Node.js](https://nodejs.org/) and [pnpm](https://pnpm.io/)
- [Rust](https://rustup.rs/)
- Platform build tools, per the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/):
  - **Windows:** Visual Studio Build Tools (C++ workload) and the WebView2 runtime (preinstalled on Windows 11)
  - **macOS:** Xcode Command Line Tools (`xcode-select --install`)

### Commands

```sh
pnpm install        # install frontend dependencies
pnpm tauri dev      # run the app with hot reload
pnpm tauri build    # produce a portable Windows build or macOS disk image
pnpm lint           # lint the frontend
pnpm --filter rivals-2-tag-tool-infra test         # run cloud service tests
cargo test --manifest-path src-tauri/Cargo.toml   # run backend tests
```

Production builds require `VITE_CLOUD_API_BASE_URL`. The AWS SAM service and deployment procedure are documented in [`infra/README.md`](infra/README.md).

Save file parsing is handled by the [uesave](https://crates.io/crates/uesave) crate. Shoutouts to the [uesave source code repo](https://github.com/trumank/uesave).

## FAQ

**Where is the default save file located?**

On Windows:

```
%LOCALAPPDATA%\Rivals2\Saved\SaveGames\Rivals2_PlayerTagSaveSlot.sav
```

The app looks there automatically at startup. If it isn't found — or your save lives elsewhere or uses another name — use the **⋯** button on the path bar to browse to any valid Rivals II tag `.sav`. Your choice is remembered between launches; the **⟳** button next to it re-reads the file.

**The Windows SmartScreen warning came up with "Windows protected your PC." Is that bad?**

Currently the builds aren't code-signed, so Windows doesn't recognize me as the publisher. Click **More info → Run anyway** to run the portable build.

**macOS says the app "is damaged and can't be opened." What gives?**

The builds aren't code-signed or notarized with Apple, so macOS quarantines downloaded copies. The app isn't actually damaged. Remove the quarantine flag by running this command in your terminal and it will open properly:

```sh
xattr -d com.apple.quarantine "/Applications/Rivals II Tag Tool.app"
```

**Does the tool work on macOS even though Rivals II isn't on Mac?**

Yes. Rivals of Aether II does not natively support macOS, so there's no default save location to look for, but the tool itself runs natively on Mac. As long as you have a valid Rivals II tag `.sav` to point it at, you can import and export tags as usual; the file does not need to use the game's default name.

**Can I use the tool on a PC that doesn't have Rivals II installed?**

Partly, and deliberately so. With no save file present you can still search the tag tool database and **Save as .r2pack** — that's the tournament organizer workflow, where the laptop building the pack isn't one of the setups. Importing, exporting, and publishing need a save file, and the app tells you which ones are unavailable.

**Does this modify my save file?**

Only when importing. Exporting, packing, and publishing just read your save. Importing writes the updated save to a temporary file and then swaps it in, so an interrupted import (full disk, antivirus, the game holding the file) leaves your original untouched rather than truncated. **Replace existing custom tags** also creates a byte-for-byte backup before removing anything.

**Can Rivals II stay open while importing/exporting tags?**

Yes... but you should almost certainly close it. Rivals II does not reread from the `.sav` file until the game is relaunched, and may overwrite any changes you make when the game is closed.

**What exactly is in a `.r2tag` file?**

It's a custom file containing a single player tag: the tag name and its custom control settings. Nothing else from your save (or your system) is included, so they're safe to share.

**What's a `.r2pack`?**

A bundle of many `.r2tag` files in one file, plus a small manifest recording where the pack came from and which save version it targets. It's a zip underneath, so you can rename it to `.zip` and look inside. Packs are meant for carrying a whole bracket's tags between setups on a USB stick.

Note that a `.r2pack` is an offline copy: if someone later deletes their published cloud tag, packs already saved elsewhere still contain it.

**What happens if an imported tag already exists on the setup?**

The import screen flags it as a conflict. Conflicts default to **Skip**; toggle individual tags to **Overwrite** if you want to replace the existing version.

Rivals II supports 96 custom tags in addition to Player1–Player4. The import screen shows how many slots the selected tags would use and prevents writing a save above that limit. Tournament organizers can instead choose **Replace existing custom tags** to remove the setup's custom tags and leave only the selected imports; Player1–Player4 are preserved and a backup is created first.

**What becomes public when I publish a cloud tag?**

Your public start.gg username and profile slug, the in-game tag name, save-format version, and compressed controls file become searchable/downloadable. The service does not store your email or start.gg OAuth tokens.

**Why don't I see Player1–Player4 in the tag list?**

Those are the game's built-in default tags. The tool only lists (and exports) custom tags.

**My tag name has characters like `/` or `:` in it. Will exporting work?**

Yes. Characters that aren't allowed in Windows or macOS filenames are replaced with `_` for the exported file's name (e.g. a tag named `test/lower` exports as `test_lower.r2tag`). The tag itself is stored unchanged inside the file, so it imports with its original name.

# Rivals II Tag Tool

Sitting down at a new tournament setup, creating a tag, and setting your custom controls can be tedious.

Export, share, and import Rivals of Aether II tags & custom controls. Stop re-entering your tag and custom controls at every tournament setup!

Runs on **Windows** and **macOS**.

## Usage

**Players (at home):**
Export your custom player tag and controls to a `.r2tag` file, then send it to your tournament organizers.

**Tournament Organizers:**
Gather all of your entrants' `.r2tag` files, and import them directly onto your setups!

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
pnpm tauri build    # produce a release build + installer
pnpm lint           # lint the frontend
cargo test --manifest-path src-tauri/Cargo.toml   # run backend tests
```

Save file parsing is handled by the [uesave](https://crates.io/crates/uesave) crate. Shoutouts to the [uesave source code repo](https://github.com/trumank/uesave).

## FAQ

**Where is the default save file located?**

On Windows:

```
%LOCALAPPDATA%\Rivals2\Saved\SaveGames\Rivals2_PlayerTagSaveSlot.sav
```

Use **Choose a Save File** to browse to it.

**Does the tool work on macOS even though Rivals II isn't on Mac?**

Yes. Rivals of Aether II does not natively support macOS, so there's no default save location to look for, but the tool itself runs natively on Mac. As long as you have a valid `Rivals2_PlayerTagSaveSlot.sav` to point it at, you can import and export tags as usual via **Choose a Save File**.

**Does this modify my save file?**

Only when importing. Exporting just reads your save and writes standalone `.r2tag` files. Importing rewrites the save file to add (or overwrite) tags, so consider making a copy of the `.sav` first if you want a backup.

**Can Rivals II stay open while importing/exporting tags?**

Yes... but you should almost certainly close it. Rivals II does not reread from the `.sav` file until the game is relaunched, and may overwrite any changes you make when the game is closed.

**What exactly is in a `.r2tag` file?**

It's a custom file containing a single player tag the tag name and its custom control settings. Nothing else from your save (or your system) is included, so they're safe to share.

**What happens if an imported tag already exists on the setup?**

The import screen flags it as a conflict. Conflicts default to **Skip**; toggle individual tags to **Overwrite** if you want to replace the existing version.

**Why don't I see Player1–Player4 in the tag list?**

Those are the game's built-in default tags. The tool only lists (and exports) custom tags.

**My tag name has characters like `/` or `:` in it. Will exporting work?**

Yes. Characters that aren't allowed in Windows or macOS filenames are replaced with `_` for the exported file's name (e.g. a tag named `test/lower` exports as `test_lower.r2tag`). The tag itself is stored unchanged inside the file, so it imports with its original name.

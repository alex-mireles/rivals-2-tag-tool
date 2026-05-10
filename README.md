# Rivals of Aether II Tag Tool

Sitting down at a new tournament setup, creating a tag, and setting your custom controls can be tedious.

Export, share, and import Rivals of Aether II tags & custom controls. Stop re-entering your tag and custom controls at every tournament setup!

## Usage

Open `Rivals2TagTool.exe` on a Windows machine with Rivals of Aether II installed.

**Players (at home):**
Export your custom player tag and controls to a `.r2tag` file, then send it to your tournament organizers.

**Tournament Organizers:**
Gather all of your entrants' `.r2tag` files, and import them directly into your setups with a simple `.exe` on a USB stick.

## Requirements for building from source

- Windows
- Python 3.12+
- `uesave.exe`, placed in the same folder as the script

### Getting uesave

Download the latest Windows release from: https://github.com/trumank/uesave-rs/releases

Grab `uesave-x86_64-pc-windows-msvc.zip`, extract `uesave.exe`, and place
it in the same directory as `rivals_tag_tool.py`.

## Building from source

1. Install PyInstaller with pip:
   ```
   pip install pyinstaller
   ```

2. Place `uesave.exe` in the same directory as `rivals_tag_tool.spec` and `rivals_tag_tool.py`.

3. Build the `.exe` with the following command:
   ```
   pyinstaller rivals_tag_tool.spec
   ```

4. Find the output at `dist/Rivals2TagTool.exe`

Copy `Rivals2TagTool.exe` to a USB or drop it on any venue PC.

## FAQ

**Where is the default save location for tags and custom controls?**

`%LOCALAPPDATA%\Rivals2\Saved\SaveGames\Rivals2_PlayerTagSaveSlot.sav`
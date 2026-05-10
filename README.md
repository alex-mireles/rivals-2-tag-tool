# Rivals of Aether II Tag Tool

Sitting down at a new tournament setup, creating a tag, and setting your custom controls can be tedious.

Export, share, and import Rivals of Aether II tags & custom controls. Stop re-entering your tag and custom controls at every tournament setup!

## Usage

Open `Rivals2TagTool.exe` on a Windows machine with Rivals of Aether II installed.

**Players (at home):**
Export your custom player tag and controls to a `.r2tag` file, then send it to your tournament organizers.

**Tournament Organizers:**
Gather all of your entrants' `.r2tag` files, and import them directly into your setups with a simple `.exe` on a USB stick.

## Requirements

This section is a WIP.

Requirements:
- uesave

### Getting uesave

Download the latest Windows release from: https://github.com/trumank/uesave-rs/releases

Grab `uesave-x86_64-pc-windows-msvc.zip`, extract `uesave.exe`, and place it in the project directory.

## FAQ

**Where is the default save location for tags and custom controls?**

`%LOCALAPPDATA%\Rivals2\Saved\SaveGames\Rivals2_PlayerTagSaveSlot.sav`
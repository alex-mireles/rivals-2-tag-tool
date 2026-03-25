# Author: Alex Mireles (HyperFlame)
# Version 0.1.0

# Mostly vibecoded this with Claude. 
# It's unlikely that I'll be using tkinter in future iterations. 
# Might turn this into a webapp that can run locally.

import sys
import os
import json
import copy
import shutil
import tempfile
import subprocess
from pathlib import Path
import tkinter as tk
from tkinter import filedialog, messagebox

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

APP_TITLE  = "Rivals 2 Tag Tool"
APP_VER    = "0.1.0"
SAV_PATH   = (
    Path(os.environ.get("LOCALAPPDATA", ""))
    / "Rivals2" / "Saved" / "SaveGames"
    / "Rivals2_PlayerTagSaveSlot.sav"
)
TAGS_KEY       = "SavedPlayerTags_0"
TAG_NAME_KEY   = "TagName_0"
IS_DEFAULT_KEY = "IsDefaultTag_0"
TAG_EXT        = ".r2tag"
UESAVE_EXE     = "uesave.exe"
SAFE_TAG_LIMIT = 90       # soft cap – warn TOs above this

# ---------------------------------------------------------------------------
# Colours / fonts
# ---------------------------------------------------------------------------

BG       = "#111119"
BG_ALT   = "#191924"
CARD     = "#1c1c2e"
CARD_HI  = "#24243a"
ACCENT   = "#e94560"
ACCENT2  = "#c73a52"
TEAL     = "#2dd4bf"
TEXT     = "#eaeaea"
SUBTEXT  = "#8888a0"
OK       = "#4caf50"
WARN     = "#ff9800"
ERR      = "#f44336"
INPUT_BG = "#13132a"
BORDER   = "#2a2a44"

FONT        = ("Segoe UI", 10)
FONT_BOLD   = ("Segoe UI", 10, "bold")
FONT_HEAD   = ("Segoe UI", 14, "bold")
FONT_TITLE  = ("Segoe UI", 18, "bold")
FONT_SMALL  = ("Segoe UI", 9)
FONT_TINY   = ("Segoe UI", 8)
FONT_MONO   = ("Consolas", 9)

# ---------------------------------------------------------------------------
# Suppress console windows from subprocess on Windows
# ---------------------------------------------------------------------------

_SUBPROCESS_FLAGS = {}
if sys.platform == "win32":
    _SUBPROCESS_FLAGS["creationflags"] = (
        subprocess.CREATE_NO_WINDOW
    )

# ---------------------------------------------------------------------------
# uesave helpers
# ---------------------------------------------------------------------------

def _uesave() -> Path:
    for candidate in [
        getattr(sys, "_MEIPASS", None) and Path(sys._MEIPASS) / UESAVE_EXE,
        Path(__file__).parent / UESAVE_EXE,
    ]:
        if candidate and Path(candidate).exists():
            return Path(candidate)
    raise FileNotFoundError(
        f"{UESAVE_EXE} not found. Place it next to this script."
    )


def sav_to_dict(sav: Path) -> dict:
    with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as tf:
        tmp = Path(tf.name)
    try:
        r = subprocess.run(
            [str(_uesave()), "to-json", "--input", str(sav), "--output", str(tmp)],
            capture_output=True, text=True, **_SUBPROCESS_FLAGS
        )
        if r.returncode != 0:
            raise RuntimeError(f"uesave to-json failed:\n{r.stderr.strip()}")
        with open(tmp, encoding="utf-8") as f:
            return json.load(f)
    finally:
        tmp.unlink(missing_ok=True)


def dict_to_sav(data: dict, sav: Path):
    with tempfile.NamedTemporaryFile(
        suffix=".json", delete=False, mode="w", encoding="utf-8"
    ) as tf:
        json.dump(data, tf, indent=2)
        tmp = Path(tf.name)
    try:
        backup = sav.with_suffix(".sav.bak")
        shutil.copy2(sav, backup)
        r = subprocess.run(
            [str(_uesave()), "from-json", "--input", str(tmp), "--output", str(sav)],
            capture_output=True, text=True, **_SUBPROCESS_FLAGS
        )
        if r.returncode != 0:
            shutil.copy2(backup, sav)
            raise RuntimeError(f"uesave from-json failed:\n{r.stderr.strip()}")
    finally:
        tmp.unlink(missing_ok=True)


# ---------------------------------------------------------------------------
# Tag helpers
# ---------------------------------------------------------------------------

def get_tags(data: dict) -> list:
    return data["root"]["properties"].get(TAGS_KEY, [])


def get_custom_tags(data: dict) -> list:
    return [t for t in get_tags(data) if not t.get(IS_DEFAULT_KEY, False)]


def get_default_tags(data: dict) -> list:
    return [t for t in get_tags(data) if t.get(IS_DEFAULT_KEY, False)]


def tag_name(t: dict) -> str:
    return t.get(TAG_NAME_KEY, "")


# ---------------------------------------------------------------------------
# Schema validation
# ---------------------------------------------------------------------------

def _deep_validate(incoming, reference, path=""):
    warnings  = []
    unknowns  = []
    patched   = copy.deepcopy(incoming)

    for k, ref_val in reference.items():
        full_key = f"{path}.{k}" if path else k
        if k not in patched:
            patched[k] = copy.deepcopy(ref_val)
            warnings.append(full_key)
        else:
            inc_val = patched[k]
            if isinstance(ref_val, dict) and isinstance(inc_val, dict):
                patched[k], w, u = _deep_validate(inc_val, ref_val, full_key)
                warnings  += w
                unknowns  += u
            elif isinstance(ref_val, list) and isinstance(inc_val, list):
                if ref_val and inc_val and isinstance(ref_val[0], dict):
                    new_list = []
                    for i, item in enumerate(inc_val):
                        if isinstance(item, dict):
                            p, w, u = _deep_validate(item, ref_val[0], f"{full_key}[{i}]")
                            new_list.append(p)
                            warnings += w
                            unknowns += u
                        else:
                            new_list.append(item)
                    patched[k] = new_list

    for k in incoming:
        full_key = f"{path}.{k}" if path else k
        if k not in reference:
            unknowns.append(full_key)

    return patched, warnings, unknowns


def validate_tag(incoming, reference):
    return _deep_validate(incoming, reference, "")


# ---------------------------------------------------------------------------
# Merge logic
# ---------------------------------------------------------------------------

def inject_or_replace(tags, new_tag):
    name = tag_name(new_tag).upper()
    for i, t in enumerate(tags):
        if tag_name(t).upper() == name:
            tags[i] = new_tag
            return "replaced"
    tags.append(new_tag)
    return "added"


# ---------------------------------------------------------------------------
# Styled widgets
# ---------------------------------------------------------------------------

def _btn(parent, text, command, color=ACCENT, fg=TEXT, width=18, font=FONT_BOLD):
    b = tk.Button(
        parent, text=text, command=command,
        bg=color, fg=fg, activebackground=color, activeforeground=fg,
        font=font, relief="flat", cursor="hand2",
        padx=12, pady=6, width=width, bd=0
    )
    darker = _darken(color)
    b.bind("<Enter>", lambda _: b.config(bg=darker))
    b.bind("<Leave>", lambda _: b.config(bg=color))
    return b


def _darken(hex_color):
    r, g, b = int(hex_color[1:3], 16), int(hex_color[3:5], 16), int(hex_color[5:7], 16)
    f = 0.80
    return f"#{int(r*f):02x}{int(g*f):02x}{int(b*f):02x}"


def _label(parent, text, font=FONT, fg=TEXT, bg=None, **kw):
    return tk.Label(parent, text=text, font=font, fg=fg, bg=bg or parent["bg"], **kw)


def _card(parent, **kw):
    return tk.Frame(parent, bg=CARD, highlightbackground=BORDER,
                    highlightthickness=1, **kw)


def _log_line(widget, text, color=TEXT):
    widget.config(state="normal")
    tag = f"col_{color.replace('#','')}"
    widget.tag_configure(tag, foreground=color)
    widget.insert("end", text + "\n", tag)
    widget.see("end")
    widget.config(state="disabled")


# ---------------------------------------------------------------------------
# Application  (single window, swappable views)
# ---------------------------------------------------------------------------

class App(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title(APP_TITLE)
        self.configure(bg=BG)
        self.resizable(False, False)

        self._frames = {}
        self._container = tk.Frame(self, bg=BG)
        self._container.pack(fill="both", expand=True)

        self._build_home()
        self._build_export()
        self._build_import()
        self._build_deploy()

        # Show home and center on first launch only
        self._frames["home"].pack(fill="both", expand=True)
        self._center(440, 380)

    def _center(self, w, h):
        self.update_idletasks()
        sw, sh = self.winfo_screenwidth(), self.winfo_screenheight()
        self.geometry(f"{w}x{h}+{(sw-w)//2}+{(sh-h)//2}")

    def _resize(self, w, h):
        """Resize window from current top-left position (no re-centering)."""
        self.update_idletasks()
        x = self.winfo_x()
        y = self.winfo_y()
        self.geometry(f"{w}x{h}+{x}+{y}")

    def _show(self, name):
        for f in self._frames.values():
            f.pack_forget()
        self._frames[name].pack(fill="both", expand=True)
        sizes = {"home": (440, 380), "export": (500, 500), "import": (580, 700),
                 "deploy": (560, 660)}
        w, h = sizes.get(name, (500, 500))
        self._resize(w, h)

    # =====================================================================
    # HOME
    # =====================================================================

    def _build_home(self):
        frame = tk.Frame(self._container, bg=BG)
        self._frames["home"] = frame

        tk.Frame(frame, bg=ACCENT, height=4).pack(fill="x")

        body = tk.Frame(frame, bg=BG)
        body.pack(fill="both", expand=True, padx=36, pady=24)

        _label(body, "Rivals2 Tag Tool", font=FONT_TITLE).pack()
        _label(body, f"v{APP_VER}  \u2022  Control profiles for tournaments",
               fg=SUBTEXT, font=FONT_SMALL).pack(pady=(2, 20))

        # Player card
        pc = _card(body)
        pc.pack(fill="x", pady=6, ipady=2)
        pc_head = tk.Frame(pc, bg=CARD)
        pc_head.pack(anchor="w", padx=16, pady=(10, 2))
        _label(pc_head, "Player", font=FONT_BOLD).pack(side="left")
        _label(pc_head, "  \u2014 for players at home", font=FONT_SMALL,
               fg=SUBTEXT).pack(side="left")
        _label(pc, "Export your controls to a file and send it to your TO.",
               fg=SUBTEXT, font=FONT_SMALL).pack(anchor="w", padx=16, pady=(0, 4))
        _btn(pc, "Export My Controls \u2192", lambda: self._show("export"),
             width=24).pack(anchor="w", padx=16, pady=(4, 12))

        # TO card
        tc = _card(body)
        tc.pack(fill="x", pady=6, ipady=2)
        tc_head = tk.Frame(tc, bg=CARD)
        tc_head.pack(anchor="w", padx=16, pady=(10, 2))
        _label(tc_head, "Tournament Organizer", font=FONT_BOLD).pack(side="left")
        _label(tc_head, "  \u2014 for TOs at the venue", font=FONT_SMALL,
               fg=SUBTEXT).pack(side="left")
        _label(tc, "Import player .r2tag files, or deploy a master save to setups.",
               fg=SUBTEXT, font=FONT_SMALL).pack(anchor="w", padx=16, pady=(0, 4))

        to_btn_row = tk.Frame(tc, bg=CARD)
        to_btn_row.pack(anchor="w", padx=16, pady=(4, 12))
        _btn(to_btn_row, "Import Controls \u2192", lambda: self._show("import"),
             width=18, color=TEAL, fg="#111").pack(side="left", padx=(0, 8))
        _btn(to_btn_row, "Deploy to Station \u2192", lambda: self._show("deploy"),
             width=20, color=TEAL, fg="#111").pack(side="left")

        _label(body, f"Save location:  {SAV_PATH}",
               fg="#555566", font=("Consolas", 7), wraplength=380).pack(pady=(20, 0))

    # =====================================================================
    # EXPORT
    # =====================================================================

    def _build_export(self):
        frame = tk.Frame(self._container, bg=BG)
        self._frames["export"] = frame

        self._exp_data  = None
        self._exp_tags  = []

        tk.Frame(frame, bg=ACCENT, height=4).pack(fill="x")

        top_bar = tk.Frame(frame, bg=BG)
        top_bar.pack(fill="x", padx=16, pady=(10, 0))
        _btn(top_bar, "\u2190 Back", lambda: self._show("home"),
             color=CARD_HI, width=7, font=FONT_SMALL).pack(anchor="w")

        _label(frame, "Export Your Controls", font=FONT_HEAD).pack(pady=(6, 2))
        _label(frame, "Save your control profile as a .r2tag file, then send it to your TO.",
               fg=SUBTEXT, font=FONT_SMALL).pack(pady=(0, 10))

        path_card = _card(frame)
        path_card.pack(fill="x", padx=20, pady=6)
        _label(path_card, "Your save file", font=FONT_BOLD).pack(
            anchor="w", padx=12, pady=(10, 4))
        path_row = tk.Frame(path_card, bg=CARD)
        path_row.pack(fill="x", padx=12, pady=(0, 10))
        self._exp_path_var = tk.StringVar(value=str(SAV_PATH))
        tk.Entry(path_row, textvariable=self._exp_path_var,
                 bg=INPUT_BG, fg=TEXT, insertbackground=TEXT,
                 font=FONT_SMALL, relief="flat", width=34).pack(side="left", ipady=3)
        _btn(path_row, "Browse", self._exp_browse, width=7, font=FONT_SMALL,
             color=CARD_HI).pack(side="left", padx=(6, 0))

        _btn(frame, "Load Tags from Save", self._exp_load, width=24).pack(pady=8)

        list_card = _card(frame)
        list_card.pack(fill="both", expand=True, padx=20, pady=6)
        _label(list_card, "Your custom tags:", font=FONT_BOLD).pack(
            anchor="w", padx=12, pady=(10, 4))
        self._exp_listbox = tk.Listbox(
            list_card, bg=INPUT_BG, fg=TEXT, selectbackground=ACCENT,
            font=FONT, relief="flat", height=5, activestyle="none",
            selectforeground=TEXT, highlightthickness=0)
        self._exp_listbox.pack(fill="both", expand=True, padx=12, pady=(0, 10))

        _btn(frame, "Export Selected Tag", self._exp_export, width=24).pack(pady=(6, 4))
        self._exp_status = _label(frame, "", fg=SUBTEXT, font=FONT_SMALL)
        self._exp_status.pack(pady=(0, 12))

    def _exp_browse(self):
        f = filedialog.askopenfilename(
            title="Select your Rivals 2 save file",
            filetypes=[("SAV files", "*.sav"), ("All files", "*.*")],
            initialdir=SAV_PATH.parent if SAV_PATH.parent.exists() else Path.home())
        if f:
            self._exp_path_var.set(f)

    def _exp_load(self):
        sav = Path(self._exp_path_var.get())
        if not sav.exists():
            messagebox.showerror("Not found", f"Save file not found:\n{sav}")
            return
        self._exp_status.config(text="Loading\u2026", fg=SUBTEXT)
        self.update()
        try:
            self._exp_data = sav_to_dict(sav)
            self._exp_tags = get_custom_tags(self._exp_data)
        except Exception as e:
            messagebox.showerror("Error", str(e))
            self._exp_status.config(text="")
            return

        self._exp_listbox.delete(0, "end")
        if not self._exp_tags:
            self._exp_status.config(text="No custom tags found in this save.", fg=WARN)
            return
        for t in self._exp_tags:
            self._exp_listbox.insert("end", f"  {tag_name(t)}")
        self._exp_listbox.selection_set(0)
        self._exp_status.config(text=f"{len(self._exp_tags)} custom tag(s) found.", fg=OK)

    def _exp_export(self):
        if not self._exp_tags:
            messagebox.showwarning("No tags", "Load your save file first.")
            return
        sel = self._exp_listbox.curselection()
        if not sel:
            messagebox.showwarning("Nothing selected", "Click a tag to select it.")
            return
        chosen = self._exp_tags[sel[0]]
        name   = tag_name(chosen)
        dest = filedialog.asksaveasfilename(
            title="Save .r2tag file",
            defaultextension=TAG_EXT,
            initialfile=f"{name}{TAG_EXT}",
            filetypes=[("Rivals2 Tag", f"*{TAG_EXT}"), ("All files", "*.*")])
        if not dest:
            return
        with open(dest, "w", encoding="utf-8") as f:
            json.dump(chosen, f, indent=2)
        self._exp_status.config(text=f"Exported: {Path(dest).name}", fg=OK)
        messagebox.showinfo(
            "Done!",
            f"Tag \"{name}\" saved to:\n{dest}\n\nSend this file to your TO.")

    # =====================================================================
    # IMPORT  (TO Mode)
    # =====================================================================

    def _build_import(self):
        frame = tk.Frame(self._container, bg=BG)
        self._frames["import"] = frame

        self._imp_sav_path = None
        self._imp_sav_data = None
        self._imp_ref_tag  = None
        self._imp_queue    = []

        tk.Frame(frame, bg=TEAL, height=4).pack(fill="x")

        top_bar = tk.Frame(frame, bg=BG)
        top_bar.pack(fill="x", padx=16, pady=(10, 0))
        _btn(top_bar, "\u2190 Back", lambda: self._show("home"),
             color=CARD_HI, width=7, font=FONT_SMALL).pack(anchor="w")

        _label(frame, "Import Player Controls", font=FONT_HEAD).pack(pady=(6, 2))
        _label(frame, "Load this setup's save, add player .r2tag files, then write them in.",
               fg=SUBTEXT, font=FONT_SMALL).pack(pady=(0, 10))

        pad = dict(padx=20, pady=5)

        # Step 1
        s1 = _card(frame)
        s1.pack(fill="x", **pad)
        s1_head = tk.Frame(s1, bg=CARD)
        s1_head.pack(fill="x", padx=12, pady=(10, 4))
        _label(s1_head, "\u2460", font=FONT_HEAD, fg=TEAL).pack(side="left")
        _label(s1_head, "  Load the setup's save file", font=FONT_BOLD).pack(
            side="left", pady=2)

        row = tk.Frame(s1, bg=CARD)
        row.pack(fill="x", padx=12, pady=(0, 10))
        self._imp_sav_var = tk.StringVar(value=str(SAV_PATH))
        tk.Entry(row, textvariable=self._imp_sav_var,
                 bg=INPUT_BG, fg=TEXT, insertbackground=TEXT,
                 font=FONT_SMALL, relief="flat", width=34).pack(side="left", ipady=3)
        _btn(row, "Browse", self._imp_browse_sav, width=7, font=FONT_SMALL,
             color=CARD_HI).pack(side="left", padx=6)
        _btn(row, "Load", self._imp_load_sav, width=6, font=FONT_SMALL,
             color=TEAL, fg="#111").pack(side="left")

        self._imp_sav_status = _label(s1, "", fg=SUBTEXT, font=FONT_SMALL)
        self._imp_sav_status.pack(anchor="w", padx=14, pady=(0, 8))

        # Step 2
        s2 = _card(frame)
        s2.pack(fill="x", **pad)
        s2_head = tk.Frame(s2, bg=CARD)
        s2_head.pack(fill="x", padx=12, pady=(10, 4))
        _label(s2_head, "\u2461", font=FONT_HEAD, fg=TEAL).pack(side="left")
        _label(s2_head, "  Add player .r2tag files", font=FONT_BOLD).pack(
            side="left", pady=2)

        btn_row = tk.Frame(s2, bg=CARD)
        btn_row.pack(fill="x", padx=12, pady=(0, 6))
        _btn(btn_row, "Browse for .r2tag Files\u2026", self._imp_add_tags,
             width=24, color=TEAL, fg="#111").pack(side="left")

        _label(s2, "Queued for import:", font=FONT_SMALL, fg=SUBTEXT).pack(
            anchor="w", padx=14, pady=(4, 2))
        self._imp_queue_lb = tk.Listbox(
            s2, bg=INPUT_BG, fg=TEXT, selectbackground=ACCENT,
            font=FONT, relief="flat", height=4, activestyle="none",
            selectforeground=TEXT, highlightthickness=0)
        self._imp_queue_lb.pack(fill="x", padx=12, pady=(0, 4))

        rm_row = tk.Frame(s2, bg=CARD)
        rm_row.pack(fill="x", padx=12, pady=(0, 10))
        _btn(rm_row, "Remove Selected", self._imp_remove_queued,
             color="#3a3a50", width=16, font=FONT_SMALL).pack(side="left")
        self._imp_queue_count = _label(rm_row, "0 files queued",
                                       font=FONT_SMALL, fg=SUBTEXT)
        self._imp_queue_count.pack(side="right", padx=6)

        # Step 3
        s3 = tk.Frame(frame, bg=BG)
        s3.pack(fill="x", padx=20, pady=8)
        _btn(s3, "\u2462  Write All to Save File", self._imp_merge,
             width=32, color=ACCENT).pack()

        # Log
        log_card = _card(frame)
        log_card.pack(fill="both", expand=True, padx=20, pady=(4, 14))
        _label(log_card, "Log", font=FONT_BOLD).pack(
            anchor="w", padx=12, pady=(8, 2))
        self._imp_log_widget = tk.Text(
            log_card, bg="#0a0a18", fg=TEXT, font=FONT_MONO,
            relief="flat", height=7, state="disabled", wrap="word",
            highlightthickness=0)
        self._imp_log_widget.pack(fill="both", expand=True, padx=12, pady=(0, 10))

    def _imp_browse_sav(self):
        f = filedialog.askopenfilename(
            title="Select this setup's save file",
            filetypes=[("SAV files", "*.sav"), ("All files", "*.*")],
            initialdir=SAV_PATH.parent if SAV_PATH.parent.exists() else Path.home())
        if f:
            self._imp_sav_var.set(f)

    def _imp_load_sav(self):
        sav = Path(self._imp_sav_var.get())
        if not sav.exists():
            messagebox.showerror("Not found", f"File not found:\n{sav}")
            return
        self._imp_log_clear()
        self._imp_log("Loading save file\u2026", SUBTEXT)
        self.update()
        try:
            self._imp_sav_data = sav_to_dict(sav)
            self._imp_sav_path = sav
        except Exception as e:
            self._imp_log(f"ERROR: {e}", ERR)
            self._imp_sav_status.config(text="Failed to load.", fg=ERR)
            return

        tags   = get_tags(self._imp_sav_data)
        custom = get_custom_tags(self._imp_sav_data)
        self._imp_ref_tag = custom[0] if custom else (tags[0] if tags else None)

        ref_note = (
            f"Schema reference: \"{tag_name(self._imp_ref_tag)}\""
            if self._imp_ref_tag else "Note: no existing tags found (empty save)")
        self._imp_log(f"Loaded OK \u2014 {len(tags)} tag(s) present  |  {ref_note}", OK)
        self._imp_sav_status.config(
            text=f"Loaded: {len(tags)} tag(s) total, {len(custom)} custom", fg=OK)

        if len(custom) >= SAFE_TAG_LIMIT:
            self._imp_log(
                f"\u26a0 Warning: {len(custom)}/{SAFE_TAG_LIMIT} custom tags \u2014 "
                f"at or above safe limit", WARN)

    def _imp_add_tags(self):
        if self._imp_sav_data is None:
            messagebox.showwarning(
                "Load save first",
                "Load the setup's save file (Step \u2460) before adding player files.")
            return
        files = filedialog.askopenfilenames(
            title="Select .r2tag files from players",
            filetypes=[("Rivals2 Tag", f"*{TAG_EXT}"), ("All files", "*.*")])
        for fp in files:
            try:
                with open(fp, encoding="utf-8") as f:
                    tag = json.load(f)
                if TAG_NAME_KEY not in tag:
                    self._imp_log(f"SKIP {Path(fp).name}: not a valid .r2tag file", WARN)
                    continue
                self._imp_queue.append(tag)
                self._imp_queue_lb.insert("end", f"  {tag_name(tag)}  ({Path(fp).name})")
                self._imp_log(f"Queued: {tag_name(tag)}", TEXT)
            except Exception as e:
                self._imp_log(f"ERROR reading {Path(fp).name}: {e}", ERR)
        self._imp_update_queue_count()

    def _imp_remove_queued(self):
        sel = self._imp_queue_lb.curselection()
        if not sel:
            return
        idx  = sel[0]
        name = tag_name(self._imp_queue[idx])
        del self._imp_queue[idx]
        self._imp_queue_lb.delete(idx)
        self._imp_log(f"Removed from queue: {name}", SUBTEXT)
        self._imp_update_queue_count()

    def _imp_update_queue_count(self):
        n = len(self._imp_queue)
        self._imp_queue_count.config(
            text=f"{n} file{'s' if n != 1 else ''} queued",
            fg=OK if n > 0 else SUBTEXT)

    def _imp_merge(self):
        if self._imp_sav_data is None:
            messagebox.showwarning("No save loaded",
                                   "Load the setup's save file first (Step \u2460).")
            return
        if not self._imp_queue:
            messagebox.showwarning("No player files",
                                   "Add at least one .r2tag file (Step \u2461).")
            return

        self._imp_log("\u2500\u2500\u2500 Starting import \u2500\u2500\u2500", SUBTEXT)
        data        = copy.deepcopy(self._imp_sav_data)
        tags        = get_tags(data)
        ref         = self._imp_ref_tag
        conflicts   = 0
        added       = 0
        replaced    = 0
        schema_warn = 0
        skipped_cap = 0

        for incoming in self._imp_queue:
            name = tag_name(incoming)

            # ── Tag limit check ──────────────────────────────────────
            current_custom = len([t for t in tags if not t.get(IS_DEFAULT_KEY, False)])
            is_replacement = name.upper() in [
                tag_name(t).upper() for t in tags if not t.get(IS_DEFAULT_KEY)
            ]
            if not is_replacement and current_custom >= SAFE_TAG_LIMIT:
                answer = messagebox.askyesno(
                    "Tag limit warning",
                    f"Adding \"{name}\" would bring custom tags to "
                    f"{current_custom + 1}/{SAFE_TAG_LIMIT}.\n\n"
                    f"This exceeds the safe limit of {SAFE_TAG_LIMIT}.\n\n"
                    f"YES \u2192 Add anyway (may cause issues in-game)\n"
                    f"NO  \u2192 Skip this tag")
                if not answer:
                    self._imp_log(
                        f"SKIP \"{name}\": exceeds safe limit "
                        f"({current_custom}/{SAFE_TAG_LIMIT})", WARN)
                    skipped_cap += 1
                    continue

            existing_names = [tag_name(t).upper() for t in tags
                              if not t.get(IS_DEFAULT_KEY)]
            if name.upper() in existing_names:
                answer = messagebox.askyesnocancel(
                    "Name conflict",
                    f"A tag named \"{name}\" already exists in the save.\n\n"
                    f"YES  \u2192  Replace the existing tag with this player's version\n"
                    f"NO   \u2192  Skip this tag (ask the player to rename)\n"
                    f"CANCEL  \u2192  Abort the entire import")
                if answer is None:
                    self._imp_log("Import aborted.", ERR)
                    return
                elif answer is False:
                    self._imp_log(f"SKIP \"{name}\": ask player to rename and resubmit", WARN)
                    conflicts += 1
                    continue

            if ref is not None:
                patched, warnings, unknowns = validate_tag(incoming, ref)
                if warnings:
                    schema_warn += len(warnings)
                    self._imp_log(
                        f"  \"{name}\": {len(warnings)} missing field(s) filled "
                        f"with defaults \u2014 may be from an older game version", WARN)
                    for w in warnings[:5]:
                        self._imp_log(f"    + defaulted: {w}", WARN)
                    if len(warnings) > 5:
                        self._imp_log(f"    \u2026 and {len(warnings)-5} more", WARN)
                if unknowns:
                    self._imp_log(
                        f"  \"{name}\": {len(unknowns)} unknown field(s) kept as-is "
                        f"(possibly from a newer game version)", SUBTEXT)
                incoming = patched

            action = inject_or_replace(tags, incoming)
            data["root"]["properties"][TAGS_KEY] = tags
            if action == "replaced":
                self._imp_log(f"  REPLACED \"{name}\"", WARN)
                replaced += 1
            else:
                self._imp_log(f"  ADDED    \"{name}\"", OK)
                added += 1

        # Write
        self._imp_log("Writing save file\u2026", SUBTEXT)
        self.update()
        try:
            dict_to_sav(data, self._imp_sav_path)
        except Exception as e:
            self._imp_log(f"ERROR writing save: {e}", ERR)
            return

        # Reload internal state
        try:
            self._imp_sav_data = sav_to_dict(self._imp_sav_path)
            self._imp_ref_tag = (get_custom_tags(self._imp_sav_data)
                                 or get_tags(self._imp_sav_data) or [None])[0]
        except Exception:
            pass

        # Clear queue
        self._imp_queue.clear()
        self._imp_queue_lb.delete(0, "end")
        self._imp_update_queue_count()

        summary = (
            f"Done: {added} added, {replaced} replaced, "
            f"{conflicts} skipped"
        )
        if skipped_cap:
            summary += f", {skipped_cap} over limit"
        summary += f", {schema_warn} schema warning(s)"
        has_issues = conflicts or schema_warn or skipped_cap
        self._imp_log(f"\u2500\u2500\u2500 {summary} \u2500\u2500\u2500",
                      OK if not has_issues else WARN)

        # Restart reminder
        self._imp_log("", TEXT)
        self._imp_log("\u26a0  Restart Rivals of Aether II for changes to take effect.", WARN)

        if schema_warn:
            messagebox.showwarning(
                "Schema warnings",
                f"{schema_warn} field(s) were missing from imported tags and were "
                f"filled with defaults.\n\n"
                f"These tags may be from an older game version. Ask those players "
                f"to re-export after updating.\n\n"
                f"Restart Rivals of Aether II for changes to take effect.")
        else:
            messagebox.showinfo(
                "Import complete",
                f"{summary}\n\n"
                f"Restart Rivals of Aether II for changes to take effect.")

    def _imp_log(self, text, color=TEXT):
        _log_line(self._imp_log_widget, text, color)

    def _imp_log_clear(self):
        self._imp_log_widget.config(state="normal")
        self._imp_log_widget.delete("1.0", "end")
        self._imp_log_widget.config(state="disabled")

    # =====================================================================
    # DEPLOY  (TO Station Mode)
    # =====================================================================

    def _build_deploy(self):
        frame = tk.Frame(self._container, bg=BG)
        self._frames["deploy"] = frame

        self._dep_master_path  = None
        self._dep_master_data  = None
        self._dep_master_custom_count = 0
        self._dep_master_tag_names = []
        self._dep_mode = tk.StringVar(value="overwrite")

        tk.Frame(frame, bg=TEAL, height=4).pack(fill="x")

        top_bar = tk.Frame(frame, bg=BG)
        top_bar.pack(fill="x", padx=16, pady=(10, 0))
        _btn(top_bar, "\u2190 Back", lambda: self._show("home"),
             color=CARD_HI, width=7, font=FONT_SMALL).pack(anchor="w")

        _label(frame, "Deploy to Station", font=FONT_HEAD).pack(pady=(6, 2))
        _label(frame, "Push a master save to this setup's local save path.",
               fg=SUBTEXT, font=FONT_SMALL).pack(pady=(0, 10))

        pad = dict(padx=20, pady=5)

        # ── Master save card ─────────────────────────────────────────
        s1 = _card(frame)
        s1.pack(fill="x", **pad)
        s1_head = tk.Frame(s1, bg=CARD)
        s1_head.pack(fill="x", padx=12, pady=(10, 4))
        _label(s1_head, "Master save", font=FONT_BOLD).pack(side="left")

        row1 = tk.Frame(s1, bg=CARD)
        row1.pack(fill="x", padx=12, pady=(0, 4))
        self._dep_master_var = tk.StringVar()
        tk.Entry(row1, textvariable=self._dep_master_var,
                 bg=INPUT_BG, fg=TEXT, insertbackground=TEXT,
                 font=FONT_SMALL, relief="flat", width=34).pack(side="left", ipady=3)
        _btn(row1, "Browse", self._dep_browse_master, width=7, font=FONT_SMALL,
             color=CARD_HI).pack(side="left", padx=6)
        _btn(row1, "Load", self._dep_load_master, width=6, font=FONT_SMALL,
             color=TEAL, fg="#111").pack(side="left")

        self._dep_master_status = _label(s1, "", fg=SUBTEXT, font=FONT_SMALL)
        self._dep_master_status.pack(anchor="w", padx=14, pady=(0, 8))

        # ── Station save path card ───────────────────────────────────
        s2 = _card(frame)
        s2.pack(fill="x", **pad)
        s2_head = tk.Frame(s2, bg=CARD)
        s2_head.pack(fill="x", padx=12, pady=(10, 4))
        _label(s2_head, "Station save path", font=FONT_BOLD).pack(side="left")

        row2 = tk.Frame(s2, bg=CARD)
        row2.pack(fill="x", padx=12, pady=(0, 4))
        self._dep_station_var = tk.StringVar(value=str(SAV_PATH))
        tk.Entry(row2, textvariable=self._dep_station_var,
                 bg=INPUT_BG, fg=TEXT, insertbackground=TEXT,
                 font=FONT_SMALL, relief="flat", width=34).pack(side="left", ipady=3)
        _btn(row2, "Browse", self._dep_browse_station, width=7, font=FONT_SMALL,
             color=CARD_HI).pack(side="left", padx=6)

        self._dep_station_status = _label(s2, "", fg=SUBTEXT, font=FONT_SMALL)
        self._dep_station_status.pack(anchor="w", padx=14, pady=(0, 8))
        self._dep_check_station()

        # ── Deploy mode card ─────────────────────────────────────────
        s3 = _card(frame)
        s3.pack(fill="x", **pad)
        s3_head = tk.Frame(s3, bg=CARD)
        s3_head.pack(fill="x", padx=12, pady=(10, 4))
        _label(s3_head, "Deploy mode", font=FONT_BOLD).pack(side="left")

        mode_row = tk.Frame(s3, bg=CARD)
        mode_row.pack(fill="x", padx=12, pady=(0, 4))

        self._dep_ow_frame = self._dep_mode_btn(
            mode_row, "Overwrite",
            "Replace station save entirely with master",
            "overwrite")
        self._dep_ow_frame.pack(side="left", fill="x", expand=True, padx=(0, 4))

        self._dep_im_frame = self._dep_mode_btn(
            mode_row, "Import",
            "Merge master tags into existing save",
            "import")
        self._dep_im_frame.pack(side="left", fill="x", expand=True, padx=(4, 0))

        self._dep_mode_note = _label(s3, "", fg=WARN, font=FONT_SMALL,
                                     wraplength=460, justify="left")
        self._dep_mode_note.pack(anchor="w", padx=14, pady=(4, 8))

        # ── Deploy button (created before _dep_update_mode_ui) ───────
        s4 = tk.Frame(frame, bg=BG)
        s4.pack(fill="x", padx=20, pady=8)
        self._dep_deploy_btn = _btn(
            s4, "Overwrite Station Save", self._dep_confirm,
            width=32, color=ACCENT)
        self._dep_deploy_btn.pack()

        self._dep_update_mode_ui()

        # ── Log ──────────────────────────────────────────────────────
        log_card = _card(frame)
        log_card.pack(fill="both", expand=True, padx=20, pady=(4, 14))
        _label(log_card, "Log", font=FONT_BOLD).pack(
            anchor="w", padx=12, pady=(8, 2))
        self._dep_log_widget = tk.Text(
            log_card, bg="#0a0a18", fg=TEXT, font=FONT_MONO,
            relief="flat", height=7, state="disabled", wrap="word",
            highlightthickness=0)
        self._dep_log_widget.pack(fill="both", expand=True, padx=12, pady=(0, 10))

    # ── deploy mode toggle ───────────────────────────────────────────

    def _dep_mode_btn(self, parent, label, desc, value):
        frame = tk.Frame(parent, bg=INPUT_BG, bd=0, highlightthickness=2,
                         highlightbackground=INPUT_BG, highlightcolor=INPUT_BG)
        lbl = tk.Label(frame, text=label, font=FONT_BOLD, fg=TEXT, bg=INPUT_BG,
                       cursor="hand2")
        lbl.pack(anchor="w", padx=10, pady=(8, 0))
        desc_lbl = tk.Label(frame, text=desc, font=FONT_SMALL, fg=SUBTEXT,
                            bg=INPUT_BG, cursor="hand2", wraplength=200,
                            justify="left")
        desc_lbl.pack(anchor="w", padx=10, pady=(0, 8))

        def _select(_=None):
            self._dep_mode.set(value)
            self._dep_update_mode_ui()

        frame.bind("<Button-1>", _select)
        lbl.bind("<Button-1>", _select)
        desc_lbl.bind("<Button-1>", _select)

        frame._label = lbl
        frame._desc  = desc_lbl
        return frame

    def _dep_update_mode_ui(self):
        mode = self._dep_mode.get()
        if mode == "overwrite":
            self._dep_ow_frame.config(highlightbackground=ACCENT,
                                       highlightcolor=ACCENT)
            self._dep_ow_frame._label.config(fg=ACCENT)
            self._dep_im_frame.config(highlightbackground=INPUT_BG,
                                       highlightcolor=INPUT_BG)
            self._dep_im_frame._label.config(fg=TEXT)
            self._dep_mode_note.config(
                text="\u26a0 This will replace all tags on this station. "
                     "A backup (.sav.bak) is created first.",
                fg=WARN)
            self._dep_deploy_btn.config(text="Overwrite Station Save")
        else:
            self._dep_ow_frame.config(highlightbackground=INPUT_BG,
                                       highlightcolor=INPUT_BG)
            self._dep_ow_frame._label.config(fg=TEXT)
            self._dep_im_frame.config(highlightbackground=TEAL,
                                       highlightcolor=TEAL)
            self._dep_im_frame._label.config(fg=TEAL)
            self._dep_mode_note.config(
                text=f"Adds missing tags from the master. Existing tags with "
                     f"the same name are skipped. Limit: {SAFE_TAG_LIMIT} "
                     f"custom tags.",
                fg=SUBTEXT)
            self._dep_deploy_btn.config(text="Import Tags to Station")

    # ── deploy browse / load ─────────────────────────────────────────

    def _dep_browse_master(self):
        f = filedialog.askopenfilename(
            title="Select master save file",
            filetypes=[("SAV files", "*.sav"), ("All files", "*.*")],
            initialdir=Path.home())
        if f:
            self._dep_master_var.set(f)

    def _dep_load_master(self):
        path = Path(self._dep_master_var.get())
        if not path.exists():
            messagebox.showerror("Not found", f"File not found:\n{path}")
            return
        self._dep_log("Loading master save\u2026", SUBTEXT)
        self.update()
        try:
            self._dep_master_data = sav_to_dict(path)
            self._dep_master_path = path
        except Exception as e:
            self._dep_log(f"ERROR: {e}", ERR)
            self._dep_master_status.config(text="Failed to load.", fg=ERR)
            return

        custom = get_custom_tags(self._dep_master_data)
        self._dep_master_custom_count = len(custom)
        self._dep_master_tag_names = [tag_name(t) for t in custom]

        preview = ", ".join(self._dep_master_tag_names[:5])
        if len(self._dep_master_tag_names) > 5:
            preview += f", +{len(self._dep_master_tag_names) - 5} more"

        status_color = OK if self._dep_master_custom_count <= SAFE_TAG_LIMIT else WARN
        self._dep_master_status.config(
            text=f"{self._dep_master_custom_count} custom tag(s): {preview}",
            fg=status_color)
        self._dep_log(
            f"Master loaded: {self._dep_master_custom_count} custom tag(s) "
            f"from {path.name}", OK)

        if self._dep_master_custom_count > SAFE_TAG_LIMIT:
            self._dep_log(
                f"\u26a0 Warning: master has {self._dep_master_custom_count} "
                f"custom tags, exceeding the safe limit of {SAFE_TAG_LIMIT}",
                WARN)

    def _dep_browse_station(self):
        f = filedialog.askopenfilename(
            title="Select station save file",
            filetypes=[("SAV files", "*.sav"), ("All files", "*.*")],
            initialdir=SAV_PATH.parent if SAV_PATH.parent.exists() else Path.home())
        if f:
            self._dep_station_var.set(f)
            self._dep_check_station()

    def _dep_check_station(self):
        path = Path(self._dep_station_var.get())
        if path.exists():
            self._dep_station_status.config(text="Existing save found.", fg=OK)
        elif path.parent.exists():
            self._dep_station_status.config(
                text="No save at this path (will be created).", fg=SUBTEXT)
        else:
            self._dep_station_status.config(
                text="Directory does not exist \u2014 check path.", fg=ERR)

    # ── deploy action ────────────────────────────────────────────────

    def _dep_confirm(self):
        if self._dep_master_data is None:
            messagebox.showwarning("No master", "Load a master save file first.")
            return

        mode    = self._dep_mode.get()
        station = Path(self._dep_station_var.get())

        if mode == "overwrite":
            msg = (
                f"This will OVERWRITE the station save at:\n"
                f"{station}\n\n"
                f"The master contains {self._dep_master_custom_count} "
                f"custom tag(s).\n"
                f"A backup (.sav.bak) will be created first.\n\nContinue?")
        else:
            msg = (
                f"This will IMPORT {self._dep_master_custom_count} tag(s) "
                f"from the master into the station save at:\n"
                f"{station}\n\n"
                f"Existing tags with the same name will be skipped.\n"
                f"A backup (.sav.bak) will be created first.\n\nContinue?")

        if not messagebox.askyesno("Confirm deploy", msg):
            return

        if mode == "overwrite":
            self._dep_overwrite(station)
        else:
            self._dep_import(station)

    def _dep_overwrite(self, station):
        self._dep_log("\u2500\u2500\u2500 Deploying (OVERWRITE) \u2500\u2500\u2500", SUBTEXT)

        if station.exists():
            backup = station.with_suffix(".sav.bak")
            try:
                shutil.copy2(station, backup)
                self._dep_log(f"Backup created: {backup.name}", SUBTEXT)
            except Exception as e:
                self._dep_log(f"ERROR creating backup: {e}", ERR)
                return

        self._dep_log(
            f"Copying master save ({self._dep_master_custom_count} custom "
            f"tags)\u2026", SUBTEXT)
        self.update()
        try:
            shutil.copy2(self._dep_master_path, station)
        except Exception as e:
            self._dep_log(f"ERROR copying file: {e}", ERR)
            return

        for name in self._dep_master_tag_names:
            self._dep_log(f"  \u2713 {name}", OK)

        self._dep_log(
            f"\u2500\u2500\u2500 Deploy complete: "
            f"{self._dep_master_custom_count} tags written \u2500\u2500\u2500", OK)
        self._dep_log("", TEXT)
        self._dep_log(
            "\u26a0  Restart Rivals of Aether II for changes to take effect.",
            WARN)
        self._dep_check_station()
        messagebox.showinfo(
            "Deploy complete",
            f"Station save overwritten with "
            f"{self._dep_master_custom_count} custom tag(s).\n\n"
            f"Restart Rivals of Aether II for changes to take effect.")

    def _dep_import(self, station):
        self._dep_log("\u2500\u2500\u2500 Deploying (IMPORT) \u2500\u2500\u2500", SUBTEXT)

        if not station.exists():
            messagebox.showerror(
                "No station save",
                "Import mode requires an existing station save to merge "
                "into.\n\nUse Overwrite mode to create a new save, or "
                "ensure the path is correct.")
            self._dep_log("ERROR: no station save found for import", ERR)
            return

        self._dep_log("Loading station save\u2026", SUBTEXT)
        self.update()
        try:
            station_data = sav_to_dict(station)
        except Exception as e:
            self._dep_log(f"ERROR loading station save: {e}", ERR)
            return

        station_tags   = get_tags(station_data)
        station_custom = get_custom_tags(station_data)
        self._dep_log(
            f"Station has {len(station_custom)} existing custom tag(s)",
            SUBTEXT)

        ref_tag = (station_custom[0] if station_custom
                   else (station_tags[0] if station_tags else None))

        master_custom = get_custom_tags(self._dep_master_data)
        added    = 0
        skipped  = 0
        schema_w = 0

        existing_names = {tag_name(t).upper() for t in station_tags
                          if not t.get(IS_DEFAULT_KEY, False)}

        for incoming in master_custom:
            name = tag_name(incoming)

            if name.upper() in existing_names:
                self._dep_log(f"  SKIP \"{name}\" (already on station)", SUBTEXT)
                skipped += 1
                continue

            current_count = len([t for t in station_tags
                                 if not t.get(IS_DEFAULT_KEY, False)])
            if current_count >= SAFE_TAG_LIMIT:
                self._dep_log(
                    f"  SKIP \"{name}\": at safe limit "
                    f"({current_count}/{SAFE_TAG_LIMIT})", WARN)
                skipped += 1
                continue

            if ref_tag is not None:
                patched, warnings, unknowns = validate_tag(incoming, ref_tag)
                if warnings:
                    schema_w += len(warnings)
                    self._dep_log(
                        f"  \"{name}\": {len(warnings)} field(s) defaulted",
                        WARN)
                incoming = patched

            station_tags.append(incoming)
            existing_names.add(name.upper())
            station_data["root"]["properties"][TAGS_KEY] = station_tags
            self._dep_log(f"  + ADDED \"{name}\"", OK)
            added += 1

        if added > 0:
            self._dep_log("Writing station save\u2026", SUBTEXT)
            self.update()
            try:
                dict_to_sav(station_data, station)
            except Exception as e:
                self._dep_log(f"ERROR writing save: {e}", ERR)
                return

        final_custom = len([t for t in station_tags
                            if not t.get(IS_DEFAULT_KEY, False)])
        summary = (
            f"Import complete: {added} added, {skipped} skipped "
            f"({final_custom}/{SAFE_TAG_LIMIT} custom tags)")
        if schema_w:
            summary += f", {schema_w} schema warning(s)"
        self._dep_log(
            f"\u2500\u2500\u2500 {summary} \u2500\u2500\u2500",
            OK if not schema_w else WARN)
        self._dep_log("", TEXT)
        self._dep_log(
            "\u26a0  Restart Rivals of Aether II for changes to take effect.",
            WARN)
        self._dep_check_station()
        messagebox.showinfo(
            "Import complete",
            f"{summary}\n\n"
            f"Restart Rivals of Aether II for changes to take effect.")

    # ── deploy log helpers ───────────────────────────────────────────

    def _dep_log(self, text, color=TEXT):
        _log_line(self._dep_log_widget, text, color)

    def _dep_log_clear(self):
        self._dep_log_widget.config(state="normal")
        self._dep_log_widget.delete("1.0", "end")
        self._dep_log_widget.config(state="disabled")


# ---------------------------------------------------------------------------
# Entry
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    app = App()
    app.mainloop()

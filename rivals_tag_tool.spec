from PyInstaller.building.build_main import Analysis, PYZ, EXE

a = Analysis(
    ['rivals_tag_tool.py'],
    pathex=[],
    binaries=[
        ('uesave.exe', '.'),
    ],
    datas=[],
    hiddenimports=[],
    hookspath=[],
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
)

pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='Rivals2TagTool',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    console=False, # hides the terminal window from popping up
    disable_windowed_traceback=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)

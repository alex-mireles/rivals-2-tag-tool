// Lets the UI run in a plain browser (`npm run dev`, then open the page) for
// layout work, without building the desktop app.
//
// Inert inside the real app: install() returns immediately when Tauri's
// internals are already present, so this can only take effect in a browser tab.
// It shims the same `window.__TAURI_INTERNALS__` object @tauri-apps/api talks
// to, so no component needs to know it exists — `invoke(...)` resolves to
// fixtures instead of reaching Rust.
//
// The fixtures are shaped just well enough to exercise the layout: a save with
// several tags, a populated shared database, and a tag whose controls differ
// from the defaults so the diff has something to render.

const TAGS = ['KIM', 'KEYB', 'ANI', 'DINPUT', 'SWITCH', 'GC', 'JUGZ!', 'BRUJITA'];

const SHARED = [
  ['RVB', 'The RVB Doc'],
  ['BRUJITA', 'Brujita'],
  ['LOOM', 'loom'],
  ['ALTN', 'AltonP'],
  ['JOULE', 'Joule Thief'],
  ['JUGZ!', 'jugeeya'],
  ['KIM', 'Kimchi'],
  ['HYPER', 'HyperFlame'],
  ['ANI', 'Ani'],
].map(([name, tag]) => ({
  name,
  author: tag,
  file: `${name.toLowerCase()}-0000.r2tag.zip`,
  startggSlug: `user/${tag.toLowerCase()}`,
  startggTag: tag,
}));

/** A parsed tag whose controls differ from the bundled defaults, so the diff
 *  shows something. Mirrors the uesave JSON shape (names carry _<index>). */
function tagTree(name: string) {
  return {
    save_game_type: '/Script/Rivals2.RivalsSaveGame',
    properties: {
      SavedPlayerTags_0: [
        {
          TagName_0: name,
          ControlSettings_0: {
            bTapJumpEnabled_0: true,
            RightStickSetting_0: 'ERivalsRightStick::Attack',
            AirGrabSetting_0: 'ERivalsAirGrab::Nair',
            ActionMappings_0: {
              CustomActionMappings_0: [
                {
                  ActionName_0: 'Jump',
                  GamepadType_0: 'EGamepadType::Standard',
                  Key_0: { KeyName_0: 'SDL_GAMEPAD_BUTTON_NORTH' },
                },
              ],
            },
          },
        },
      ],
    },
  };
}

const FIXTURES: Record<string, (args: Record<string, unknown>) => unknown> = {
  get_default_save_path: () =>
    'C:\\Users\\Josh\\AppData\\Local\\Rivals2\\Saved\\SaveGames\\Rivals2_PlayerTagSaveSlot.sav',
  get_tag_names: () => TAGS,
  fetch_shared_tags: () => SHARED,
  read_tag_json_from_save: a => tagTree(String(a.tagName ?? 'TAG')),
  read_tag_json: a => tagTree(String(a.path ?? 'TAG').split(/[\\/]/).pop() ?? 'TAG'),
  download_tags: a => (a.files as string[]).map(f => `C:\\tmp\\${f.replace('.zip', '')}`),
  get_tag_previews: a => ({
    save_version: 1,
    previews: (a.r2tagPaths as string[]).map(p => ({
      path: p,
      tag_name: (p.split(/[\\/]/).pop() ?? '').replace('.r2tag', '').split('-')[0].toUpperCase(),
      version: 1,
      compatible: true,
    })),
  }),
  import_tags: a => ({
    imported: (a.instructions as { tag_name: string }[]).map(i => i.tag_name),
    skipped: [],
    incompatible: [],
  }),
  export_tags: a => (a.tagNames as string[]).map(n => `C:\\tmp\\${n}.r2tag`),
  share_tags_to_site: () => ({ pr: 'https://example.invalid/pr/1', number: 1 }),
  startgg_event: () => ({
    event: 'Demo Event',
    entrants: [
      { entrant: 'jugeeya', gamerTag: 'jugeeya', slug: 'user/jugeeya' },
      { entrant: 'Kimchi', gamerTag: 'Kimchi', slug: 'user/kimchi' },
      // One entrant with no published tag, so the "N without a tag" misses
      // disclosure has something to render in browser dev mode.
      { entrant: 'CalVal', gamerTag: 'CalVal', slug: 'user/calval' },
    ],
  }),
  startgg_search: () => [
    { gamerTag: 'jugeeya', prefix: '', slug: 'user/jugeeya' },
    { gamerTag: 'Kimchi', prefix: '', slug: 'user/kimchi' },
  ],
  startgg_user: () => ({ slug: 'user/jugeeya', gamerTag: 'jugeeya', prefix: '' }),
};

export function install() {
  const w = window as unknown as Record<string, unknown>;
  if (w.__TAURI_INTERNALS__) return; // real app — never interfere

  w.__TAURI_INTERNALS__ = {
    metadata: {
      currentWindow: { label: 'main' },
      currentWebview: { windowLabel: 'main', label: 'main' },
    },
    transformCallback: (cb: unknown) => cb,
    convertFileSrc: (p: string) => p,
    invoke: (cmd: string, args: Record<string, unknown> = {}) => {
      // Plugins (dialog, opener, window controls) are no-ops in the browser.
      if (cmd.startsWith('plugin:dialog')) return Promise.resolve('C:\\tmp\\picked.r2tag');
      if (cmd.startsWith('plugin:')) return Promise.resolve(null);
      const fn = FIXTURES[cmd];
      if (!fn) return Promise.reject(new Error(`no browser fixture for "${cmd}"`));
      return Promise.resolve(fn(args));
    },
  };
  console.info('[dev] Tauri not detected — running with browser fixtures.');
}

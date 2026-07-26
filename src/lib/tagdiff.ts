// "What did this tag change from default?" — extracts a compact digest of a
// tag's control settings/bindings from a parsed save and diffs it against the
// bundled default baseline.
//
// Ported from the tag-sharing website (tags/tagdiff.js). Both sides read the
// save through uesave, so the JSON tree is identical and this logic is a
// straight port — keep the two in sync if either changes.
//
// Deliberately free of imports: the baseline is passed in (see tagDefaults.ts,
// which binds the bundled asset). That keeps this file pure, so a parity test
// can run it under plain Node against the website's original.

const ENUM_SETTINGS = new Set([
  'RollSetting', 'RightStickSetting', 'AirParrySetting', 'AirGrabSetting', 'ItemTossSetting',
]);
const NUM_SETTINGS = new Set(['AirdodgeCardinalRoundingAngle']);

export interface DiffItem { label: string; from: string; to: string }
export interface DiffGroup { scope: string; items: DiffItem[] }
export interface TagDiff { count: number; groups: DiffGroup[] }

interface ControllerDigest {
  actions: Record<string, string[]>;
  axes: Record<string, string[]>;
  sensitivity: { max: number; values: number[] } | null;
}
export interface Digest {
  settings: Record<string, string | number | boolean>;
  controllers: Record<string, ControllerDigest>;
}

// The parsed save is an arbitrary uesave JSON tree — deeply nested, with
// property names carrying a `_<index>` suffix. There's no useful static type
// for it, and navigation is dynamic by nature, so this one alias is `any` and
// everything else stays typed.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Any = any;

// uesave suffixes repeated property names with _<index>; compare on the bare name.
const strip = (k: string) => (k && /_\d+$/.test(k) ? k.replace(/_\d+$/, '') : k);

function first(obj: Any, prefix: string): Any {
  if (obj && typeof obj === 'object' && !Array.isArray(obj)) {
    for (const k of Object.keys(obj)) if (k.startsWith(prefix)) return obj[k];
  }
  return undefined;
}

const enumShort = (v: Any) =>
  typeof v === 'string' && v.includes('::') ? v.split('::').pop()! : v;
const round = (v: Any, n: number) => {
  const m = 10 ** n;
  return Math.round(Number(v) * m) / m;
};

/** Parsed save root (`{ save_game_type, properties }`) -> settings/bindings digest. */
export function extractDigest(root: Any): Digest {
  const tag = root.properties.SavedPlayerTags_0[0];
  const cs = first(tag, 'ControlSettings');
  const digest: Digest = { settings: {}, controllers: {} };

  // Global gameplay settings + toggles (scalar children of ControlSettings).
  for (const [k, v] of Object.entries(cs || {})) {
    if (v && typeof v === 'object') continue;
    const base = strip(k);
    if (ENUM_SETTINGS.has(base)) digest.settings[base] = enumShort(v);
    else if (NUM_SETTINGS.has(base)) digest.settings[base] = round(v, 4);
    else if (base.startsWith('b')) digest.settings[base] = !!v;
  }

  // Bindings: collect every action/axis mapping, bucket by input type.
  const actions: Any[] = [];
  const axes: Any[] = [];
  (function walk(o: Any) {
    if (o && typeof o === 'object' && !Array.isArray(o)) {
      if (Object.keys(o).some((k) => k.startsWith('ActionName'))) actions.push(o);
      if (Object.keys(o).some((k) => k.startsWith('AxisName'))) axes.push(o);
      for (const v of Object.values(o)) walk(v);
    } else if (Array.isArray(o)) {
      for (const v of o) walk(v);
    }
  })(tag);

  const bucketOf = (e: Any) =>
    first(e, 'bKeyboardKey') ? 'Keyboard' : enumShort(first(e, 'GamepadType')) || 'Unknown';
  const keynameOf = (e: Any) => first(first(e, 'Key') || {}, 'KeyName');
  const ctrl = (t: string): ControllerDigest =>
    digest.controllers[t] || (digest.controllers[t] = { actions: {}, axes: {}, sensitivity: null });

  for (const e of actions) {
    const c = ctrl(bucketOf(e));
    const name = first(e, 'ActionName');
    const mods = ['bShift', 'bCtrl', 'bAlt', 'bCmd'].filter((m) => first(e, m)).map((m) => m.slice(1));
    const label = keynameOf(e) + (mods.length ? ' +' + mods.join(',') : '');
    if (!c.actions[name]) c.actions[name] = [];
    if (!c.actions[name].includes(label)) c.actions[name].push(label);
  }
  for (const e of axes) {
    const c = ctrl(bucketOf(e));
    const name = first(e, 'AxisName');
    const label = `${keynameOf(e)}(x${round(first(e, 'Scale') || 0, 3)})`;
    if (!c.axes[name]) c.axes[name] = [];
    if (!c.axes[name].includes(label)) c.axes[name].push(label);
  }

  // Per-controller sensitivity (max per-axis value flags a change).
  const cset = first(cs, 'ControllerSettings');
  const blocks = (first(cset, 'ControllerSettings') || []).filter(
    (b: Any) => b && typeof b === 'object',
  );
  const pairs: [string, Any][] = blocks.map((b: Any) => [enumShort(b.key), b.value]);
  const kb = first(cset, 'KeyboardSettings');
  if (kb) pairs.push(['Keyboard', kb]);
  for (const [type, val] of pairs) {
    const sens: number[] = [];
    (function collect(o: Any) {
      if (o && typeof o === 'object' && !Array.isArray(o)) {
        for (const [k, v] of Object.entries(o)) {
          if (strip(k) === 'Sensitivity' && typeof v === 'number') sens.push(round(v, 3));
          else collect(v);
        }
      } else if (Array.isArray(o)) {
        for (const v of o) collect(v);
      }
    })(first(val, 'AxisProperties'));
    if (sens.length) ctrl(type).sensitivity = { max: Math.max(...sens), values: sens };
  }

  // Stable ordering so digests compare cleanly.
  for (const c of Object.values(digest.controllers)) {
    for (const m of ['actions', 'axes'] as const) {
      for (const n of Object.keys(c[m])) c[m][n].sort();
    }
  }
  return digest;
}

// ---- diff ------------------------------------------------------------------

const TYPE_ORDER = [
  'Keyboard', 'Standard', 'Xbox360', 'XboxOne', 'GameCube', 'NintendoSwitchPro', 'PS4', 'PS5',
];

export function diffDigests(tag: Digest, def: Any): TagDiff {
  const groups: DiffGroup[] = [];

  const sItems: DiffItem[] = [];
  for (const [k, v] of Object.entries(tag.settings || {})) {
    const bv = (def.settings || {})[k];
    if (v !== bv) sItems.push({ label: settingLabel(k), from: enumLabel(bv), to: enumLabel(v) });
  }
  sItems.sort((a, b) => a.label.localeCompare(b.label));
  if (sItems.length) groups.push({ scope: 'Gameplay settings', items: sItems });

  const types = Object.keys(tag.controllers || {}).sort(
    (a, b) => (TYPE_ORDER.indexOf(a) + 1 || 99) - (TYPE_ORDER.indexOf(b) + 1 || 99),
  );
  for (const t of types) {
    const c: Any = tag.controllers[t] || {};
    const bc: Any = (def.controllers || {})[t] || {};
    const items: DiffItem[] = [];
    if (c.sensitivity && bc.sensitivity && c.sensitivity.max !== bc.sensitivity.max) {
      items.push({
        label: 'Sensitivity',
        from: String(bc.sensitivity.max),
        to: String(c.sensitivity.max),
      });
    }
    for (const m of ['actions', 'axes'] as const) {
      for (const [name, keys] of Object.entries<Any>(c[m] || {})) {
        const bk = (bc[m] || {})[name];
        if (bk && JSON.stringify(keys) !== JSON.stringify(bk)) {
          items.push({ label: camel(name), from: keyList(bk), to: keyList(keys) });
        }
      }
    }
    if (items.length) {
      groups.push({ scope: t === 'Keyboard' ? 'Keyboard' : `Controller · ${camel(t)}`, items });
    }
  }
  return { count: groups.reduce((n, g) => n + g.items.length, 0), groups };
}

/** Diff a tag (given its parsed save root) against a default baseline. */
export function diffTagRoot(root: Any, def: Any): TagDiff {
  return diffDigests(extractDigest(root), def);
}

// ---- friendly labels -------------------------------------------------------

function camel(s: Any) {
  return String(s).replace(/([a-z0-9])([A-Z])/g, '$1 $2').replace(/_/g, ' ').trim();
}

const SETTING_LABEL: Record<string, string> = {
  RollSetting: 'Roll',
  RightStickSetting: 'Right stick',
  AirParrySetting: 'Air parry',
  AirGrabSetting: 'Air grab',
  ItemTossSetting: 'Item toss',
  AirdodgeCardinalRoundingAngle: 'Airdodge cardinal angle',
};
function settingLabel(k: string) {
  return SETTING_LABEL[k] || camel(k.replace(/^b/, '').replace(/Enabled$/, ''));
}

const ENUM_LABEL: Record<string, string> = { Nair: 'N-air', Nspecial: 'N-special', None: 'Off' };
function enumLabel(v: Any) {
  if (typeof v === 'boolean') return v ? 'On' : 'Off';
  if (v == null) return '—';
  return ENUM_LABEL[v] || camel(v);
}

const KEY_LABEL: Record<string, string> = {
  SDL_GAMEPAD_BUTTON_SOUTH: 'South (A)',
  SDL_GAMEPAD_BUTTON_EAST: 'East (B)',
  SDL_GAMEPAD_BUTTON_WEST: 'West (X)',
  SDL_GAMEPAD_BUTTON_NORTH: 'North (Y)',
  SDL_GAMEPAD_BUTTON_LEFT_SHOULDER: 'L bumper',
  SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER: 'R bumper',
  SDL_GAMEPAD_BUTTON_BACK: 'Back',
  SDL_GAMEPAD_BUTTON_START: 'Start',
  SDL_GAMEPAD_BUTTON_DPAD_UP: 'D-pad ↑',
  SDL_GAMEPAD_BUTTON_DPAD_DOWN: 'D-pad ↓',
  SDL_GAMEPAD_BUTTON_DPAD_LEFT: 'D-pad ←',
  SDL_GAMEPAD_BUTTON_DPAD_RIGHT: 'D-pad →',
  SDL_GAMEPAD_AXIS_LEFTX: 'L-stick X',
  SDL_GAMEPAD_AXIS_LEFTY: 'L-stick Y',
  SDL_GAMEPAD_AXIS_RIGHTX: 'R-stick X',
  SDL_GAMEPAD_AXIS_RIGHTY: 'R-stick Y',
  SDL_GAMEPAD_AXIS_LEFT_TRIGGER: 'L trigger',
  SDL_GAMEPAD_AXIS_RIGHT_TRIGGER: 'R trigger',
  SpaceBar: 'Space',
};
function keyLabel(raw: Any) {
  const m = String(raw).match(/^(.*?)(\s*\(x[^)]+\))?$/);
  const bare = (m ? m[1] : raw).trim();
  const scale = m && m[2] ? m[2].trim() : '';
  let lab = KEY_LABEL[bare];
  if (!lab) {
    lab = bare.startsWith('RivalsVirtualKey_')
      ? camel(bare.slice('RivalsVirtualKey_'.length))
      : bare;
  }
  return scale ? `${lab} ${scale}` : lab;
}
const keyList = (arr: Any) => (arr || []).map(keyLabel).join(', ');

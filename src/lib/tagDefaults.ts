// Binds the bundled default-controls baseline to the pure diff logic.
//
// Split out so tagdiff.ts stays import-free and can be run under plain Node by
// the parity test. The baseline ships with the app, so the control diff works
// offline; regenerate src/assets/control-defaults.json if the game's defaults
// ever change.

import defaults from '../assets/control-defaults.json';
import { diffTagRoot, type TagDiff } from './tagdiff';

export type { TagDiff, DiffGroup, DiffItem } from './tagdiff';

/** Diff a parsed tag against the bundled defaults. */
export function diffTag(root: unknown): TagDiff {
  return diffTagRoot(root, defaults);
}

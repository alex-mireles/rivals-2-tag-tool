import { computed, readonly, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { SaveFileInfo, SaveStatus } from '../types';

/**
 * The one save file the app is pointed at.
 *
 * Module-scoped rather than provided/injected: there is exactly one save file
 * and one window, and six consumers across two view levels. State is exposed
 * read-only so a view can never keep a private copy that drifts out of sync
 * after another view writes to the save.
 */

export const EXPECTED_SAVE_FILE_NAME = 'Rivals2_PlayerTagSaveSlot.sav';

const status = ref<SaveStatus>('resolving');
const path = ref('');
const source = ref<SaveFileInfo['source']>('none');
const defaultDir = ref('');
const tagNames = ref<string[]>([]);
const saveVersion = ref<number | null>(null);
const errorMsg = ref('');

function apply(info: SaveFileInfo) {
  status.value = info.status;
  path.value = info.path;
  source.value = info.source;
  defaultDir.value = info.defaultDir;
  tagNames.value = info.tagNames;
  saveVersion.value = info.saveVersion;
  errorMsg.value = info.error ?? '';
}

async function run(command: 'resolve_save_file' | 'set_save_path', args?: Record<string, unknown>) {
  status.value = 'resolving';
  try {
    apply(await invoke<SaveFileInfo>(command, args));
  } catch (error) {
    // Only a broken environment reaches here — everyday problems (missing,
    // unreadable, wrong file) come back as a status, not a rejection.
    status.value = 'unreadable';
    errorMsg.value = String(error);
  }
}

/** Resolve the save file and read its tags. Also serves as reload. */
const reload = () => run('resolve_save_file');

/** Let the user pick a save file; the choice persists across launches. */
async function choose() {
  const picked = await open({
    multiple: false,
    title: 'Choose a Save File',
    filters: [{ name: '.sav file', extensions: ['sav'] }],
    ...(defaultDir.value ? { defaultPath: defaultDir.value } : {}),
  });
  if (typeof picked !== 'string') return;
  await run('set_save_path', { path: picked });
}

/** Forget a hand-picked path and fall back to the default location. */
const resetToDefault = () => run('set_save_path', { path: '' });

/** The save can be read and written: gates import, export, and publishing. */
const canWriteSave = computed(() => status.value === 'ready');
const hasTags = computed(() => tagNames.value.length > 0);

export function useSaveFile() {
  return {
    status: readonly(status),
    path: readonly(path),
    source: readonly(source),
    defaultDir: readonly(defaultDir),
    tagNames: readonly(tagNames),
    saveVersion: readonly(saveVersion),
    errorMsg: readonly(errorMsg),
    canWriteSave,
    hasTags,
    reload,
    choose,
    resetToDefault,
  };
}

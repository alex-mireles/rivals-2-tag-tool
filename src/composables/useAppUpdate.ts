import { computed, readonly, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { openUrl } from '@tauri-apps/plugin-opener';

/**
 * Availability and installation of a newer release.
 *
 * Dismissal is deliberately not persisted: on a shared tournament PC, one
 * person clicking "Later" shouldn't silence the notice for the next.
 */

export interface UpdateInfo {
  version: string;
  notes: string | null;
  pubDate: string | null;
  /** False on macOS, and on Windows when the install directory isn't writable. */
  canSelfInstall: boolean;
  releasePageUrl: string;
}

interface DownloadProgress {
  downloaded: number;
  total: number | null;
}

const PROGRESS_EVENT = 'update://download-progress';

type Phase = 'idle' | 'available' | 'downloading' | 'installing' | 'failed';

const info = ref<UpdateInfo | null>(null);
const phase = ref<Phase>('idle');
const dismissed = ref(false);
const error = ref('');
/** 0–1, or null when the server didn't tell us the total size. */
const fraction = ref<number | null>(null);

const isBusy = computed(() => phase.value === 'downloading' || phase.value === 'installing');

/** Dismissal hides the banner, but never mid-install. */
const showNotice = computed(() => !!info.value && (!dismissed.value || isBusy.value));

/**
 * Look for a newer release. Errors are ignored on purpose — if the PC is
 * offline there's nothing the user could do about it anyway.
 */
async function check() {
  if (info.value || isBusy.value) return;
  try {
    const found = await invoke<UpdateInfo | null>('check_for_update');
    if (!found) return;
    info.value = found;
    phase.value = 'available';
  } catch {
    // Deliberately swallowed — see above.
  }
}

/**
 * Download, check, replace, and restart. On success this never finishes — the
 * app restarts — so anything that comes back is a failure.
 */
async function install() {
  if (!info.value?.canSelfInstall || isBusy.value) return;

  error.value = '';
  fraction.value = null;
  phase.value = 'downloading';

  const unlisten = await listen<DownloadProgress>(PROGRESS_EVENT, (event) => {
    const { downloaded, total } = event.payload;
    fraction.value = total ? Math.min(downloaded / total, 1) : null;
    if (total && downloaded >= total) phase.value = 'installing';
  });

  try {
    await invoke('install_update');
  } catch (cause) {
    error.value = String(cause);
    phase.value = 'failed';
  } finally {
    unlisten();
  }
}

function dismiss() {
  dismissed.value = true;
}

async function openReleasePage() {
  if (info.value) await openUrl(info.value.releasePageUrl);
}

export function useAppUpdate() {
  return {
    info: readonly(info),
    phase: readonly(phase),
    error: readonly(error),
    fraction: readonly(fraction),
    isBusy,
    showNotice,
    check,
    install,
    dismiss,
    openReleasePage,
  };
}

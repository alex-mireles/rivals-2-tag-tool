import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

/**
 * Tag files staged in the app cache awaiting review — cloud downloads and
 * `.r2pack` extractions alike.
 *
 * Every producer must funnel through here: a second list of paths somewhere
 * else is a guaranteed leak, since only what's tracked gets cleaned up.
 */

const paths = ref<string[]>([]);

function add(newPaths: string[]) {
  paths.value = [...paths.value, ...newPaths];
}

async function cleanup() {
  if (!paths.value.length) return;
  const staged = paths.value;
  paths.value = [];
  await invoke('cleanup_cloud_files', { paths: staged }).catch(() => undefined);
}

export function useStagedTags() {
  return { paths, add, cleanup };
}

<script setup lang="ts">
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const appVersion = APP_VERSION; // Retrieved from package.json using vite.config.ts
const errorMsg = ref('');

const tagNames = ref<string[]>([]);
const hasLoaded = ref(false);

const TEST_SAVE_PATH =
  '/home/hyper/code/projects/rivals-2-tag-tool/test-data/Rivals2_PlayerTagSaveSlot.sav';

async function loadTagNames() {
  errorMsg.value = '';
  tagNames.value = [];
  hasLoaded.value = false;

  try {
    tagNames.value = await invoke<string[]>('get_tag_names', {
      savePath: TEST_SAVE_PATH,
    });
    hasLoaded.value = true;
  } catch (error) {
    errorMsg.value = String(error);
  }
}
</script>

<template>
  <div class="min-h-screen flex items-center justify-center font-sans bg-base-800">

    <div class="w-[520px] bg-surface border border-line rounded-card px-7 py-8 text-text-primary">

      <div class="text-center mb-5">
        <h1 class="text-[26px] font-bold tracking-tight text-text-primary mb-0.5">
          Rivals 2 Tag Tool
        </h1>
        <span class="text-[11px] tracking-[1.5px] text-text-muted">
          v{{ appVersion }}
        </span>
      </div>

      <div class="flex items-center gap-2.5 bg-surface-inset rounded-button px-3.5 py-2.5 mb-5 border border-line-subtle">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" class="shrink-0">
          <path d="M2 2h5l1.5 2H14v10H2V2z" stroke="rgba(200,180,230,0.35)" stroke-width="1.2" />
        </svg>
        <span class="font-mono text-[11.5px] truncate text-text-muted">
          No save file loaded
        </span>
      </div>

      <button class="w-full cursor-pointer text-sm font-semibold tracking-wide transition-colors duration-150 bg-accent hover:bg-accent-hover text-white rounded-button py-2.5 mb-5 border-none">
        Load tags from save file
      </button>

      <div class="bg-surface-inset rounded-panel px-4 pt-4 pb-1 border border-line-subtle">

        <div class="flex justify-between items-baseline mb-3">
          <span class="text-[11px] uppercase tracking-[1.5px] font-medium text-text-muted">
            Player tags
          </span>
        </div>

        <div class="text-center border-t border-line-divider py-6">
          <span class="text-sm text-text-secondary">
            Load a save file to see tags
          </span>
        </div>
      </div>

      <div class="flex gap-2.5 mt-5">
        <button class="flex-1 text-[13px] font-medium cursor-pointer transition-colors duration-150 bg-surface hover:bg-surface-hover text-text-secondary border border-line rounded-button py-2">
          Export selected
        </button>
        <button class="flex-1 text-[13px] font-medium cursor-pointer transition-colors duration-150 bg-surface hover:bg-surface-hover text-text-secondary border border-line rounded-button py-2">
          Import .r2tag
        </button>
      </div>

    </div>
  </div>
</template>

<style>
*,
*::before,
*::after {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body {
  height: 100%;
  overflow: hidden;
  background-color: #120a1f;
}

button:focus-visible {
  outline: 2px solid rgba(99, 102, 241, 0.6);
  outline-offset: 2px;
}
</style>
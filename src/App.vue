<script setup lang="ts">
import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

const appVersion = APP_VERSION;
const EXPECTED_SAVE_FILE_NAME = 'Rivals2_PlayerTagSaveSlot.sav';

const errorMsg = ref('');
const tagNames = ref<string[]>([]);
const hasLoaded = ref(false);

const savePath = ref('');
const savePathError = ref(false);

async function chooseSaveFile() {
  const defaultPath = await invoke<string>('get_default_save_path');
  
  const filePath = await open({
    multiple: false,
    title: 'Choose a Save File',
    filters: [{ name: '.sav file', extensions: ['sav'] }],
    defaultPath: defaultPath,
  });

  if (!filePath) return;

  // Extract just the filename from the full path
  const fileName = filePath.split('\\').pop() ?? '';

  if (fileName !== EXPECTED_SAVE_FILE_NAME) {
    savePath.value = filePath;
    savePathError.value = true;
    return;
  }

  savePathError.value = false;
  savePath.value = filePath;
}

const savePathDisplay = computed(() => {
  if (!savePath.value) {
    return `${EXPECTED_SAVE_FILE_NAME} not selected`;
  }
  if (savePathError.value) {
    return `Expected ${EXPECTED_SAVE_FILE_NAME}`;
  }
  return savePath.value;
});

async function loadTagNames() {
  errorMsg.value = '';
  tagNames.value = [];
  hasLoaded.value = false;

  try {
    tagNames.value = await invoke<string[]>('get_tag_names', {
      savePath: savePath,
    });
    hasLoaded.value = true;
  } catch (error) {
    errorMsg.value = String(error);
  }
}
</script>

<template>
  <div class="viewport">
    <div class="card">

      <div class="card-header">
        <h1 class="app-title">Rivals of Aether II <br> Player Tag Tool</h1>
        <span class="app-version">v{{ appVersion }}</span>
      </div>

      <div class="save-path-row">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" class="save-path-icon">
          <path d="M2 2h5l1.5 2H14v10H2V2z" stroke="rgba(200,180,230,0.35)" stroke-width="1.5" />
        </svg>
        <div class="save-path-label" :class="{ 'save-path-label-error': savePathError, 'save-path-label-success': savePath }" >
          {{ savePathDisplay }}
        </div>
      </div>

      <button class="btn btn-primary" :class="{ 'btn-primary-save-selected': savePath && !savePathError }" @click="chooseSaveFile">
        Choose a Save File
      </button>

      <Transition name="fade">
        <div v-if="savePath && !savePathError" class="load-tags-wrapper">
          <button class="btn btn-primary" @click="loadTagNames">
            Load Tags
          </button>
        </div>
      </Transition>

      <div class="tag-panel">
        <div class="tag-panel-header">
          <span class="tag-panel-label">Player tags</span>
        </div>

        <div class="tag-panel-empty">
          <span class="tag-panel-empty-message">Load a save file to see tags</span>
        </div>
      </div>

      <div class="action-row">
        <button class="btn btn-ghost">Export Tags</button>
        <button class="btn btn-ghost">Import or Overwrite</button>
      </div>

    </div>
  </div>
</template>

<style scoped lang="scss">
.viewport {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
}

.card {
  width: 500px;
  display: flex;
  flex-direction: column;
  align-items: center;
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: var(--radius-card);
  padding: 2rem 1.75rem;
  color: var(--text-primary);

  &-header {
    text-align: center;
  }

}

.card > * + * {
  margin-top: 1rem;
}

.app-title {
  font-size: 2em;
  font-weight: 700;
}

.app-version {
  font-size: 0.75em;
  letter-spacing: 0.1em;
  color: var(--text-muted);
}

.save-path-row {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.625rem;
  background: var(--surface-inset);
  border: 1px solid var(--line-subtle);
  border-radius: var(--radius-button);
  padding: 0.625rem 0.875rem;
}

.save-path-icon {
  flex-shrink: 0;
}

.save-path-label {
  font-family: monospace;
  font-size: 11px;
  min-width: 0;
  color: var(--text-muted);
  word-break: break-all;
  
  &::-webkit-scrollbar {
    display: none;
  }

  &-success {
    color: var(--text-success);
  }

  &-error {
    color: var(--text-failure);
  }
}

.btn {
  width: 100%;
  cursor: pointer;
  font-weight: 600;
  border-radius: var(--radius-button);
  padding: 0.625rem 0;
  border: none;
  transition: background 500ms, filter 500ms, transform 500ms;

  &:hover {
    transform: translateY(-0.2em);
  }

  &-primary {
    background: var(--accent);
    color: white;
    font-size: 0.875rem;
    filter: brightness(1.1);

    &-save-selected {
      background: var(--accent-completed);
      
      &:hover {
        background: var(--accent);
      }
    }
  }

  &-ghost {
    background: var(--surface);
    color: var(--text-secondary);
    border: 1px solid var(--line);
    font-size: 13px;
    font-weight: 500;

    &:hover {
      background: var(--surface-hover);
    }
  }
}

.tag-panel {
  width: 100%;
  background: var(--surface-inset);
  border: 1px solid var(--line-subtle);
  border-radius: var(--radius-panel);
  padding: 1rem 1rem 0.25rem;

  &-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  &-label {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    font-weight: 500;
    color: var(--text-muted);
  }

  &-empty {
    text-align: center;
    border-top: 1px solid var(--line-divider);
    padding: 1.5rem 0;

    &-message {
      font-size: 0.875rem;
      color: var(--text-secondary);
    }
  }
}

.action-row {
  width: 100%;
  display: flex;
  gap: 0.625rem;

  .btn {
    flex: 1;
  }
}

.load-tags-wrapper {
  overflow: hidden;
  width: 100%;
  padding-top: 0.2rem; // Need the padding + margin to make sure the Load Tags button 
  margin-top: 0.8rem; // doesn't get cut off by its wrapped div
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.5s ease, max-height 0.5s ease, margin-top 0.5s ease, padding-top 0.5s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
  max-height: 0;
  margin-top: 0rem;
  padding-top: 0rem;
}

.fade-enter-to,
.fade-leave-from {
  opacity: 1;
  max-height: 3rem;
}
</style>
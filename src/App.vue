<script setup lang="ts">
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import './styles/global.scss';

const appVersion = APP_VERSION;
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
  <div class="viewport">
    <div class="card">

      <div class="card-header">
        <h1 class="app-title">Rivals of Aether II Tag Tool</h1>
        <span class="app-version">v{{ appVersion }}</span>
      </div>

      <div class="save-path-row">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" class="save-path-icon">
          <path d="M2 2h5l1.5 2H14v10H2V2z" stroke="rgba(200,180,230,0.35)" stroke-width="1.2" />
        </svg>
        <span class="save-path-label">No save file loaded</span>
      </div>

      <button class="btn btn-primary" @click="loadTagNames">
        Choose a Save File
      </button>

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
  width: 520px;
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: var(--radius-card);
  padding: 2rem 1.75rem;
  color: var(--text-primary);

  &-header {
    text-align: center;
    margin-bottom: 1.25rem;
  }
}

.app-title {
  font-size: 2em;
  font-weight: 700;
  margin-bottom: 0.125rem;
}

.app-version {
  font-size: 0.75em;
  letter-spacing: 0.1em;
  color: var(--text-muted);
}

.save-path-row {
  display: flex;
  align-items: center;
  gap: 0.625rem;
  background: var(--surface-inset);
  border: 1px solid var(--line-subtle);
  border-radius: var(--radius-button);
  padding: 0.625rem 0.875rem;
  margin-bottom: 1.25rem;
}

.save-path-icon {
  flex-shrink: 0;
}

.save-path-label {
  font-family: monospace;
  font-size: 11.5px;
  color: var(--text-muted);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.btn {
  width: 100%;
  cursor: pointer;
  font-weight: 600;
  letter-spacing: 0.025em;
  border-radius: var(--radius-button);
  padding: 0.625rem 0;
  border: none;
  transition: background-color 250ms, box-shadow 250ms, transform 250ms;

  &-primary {
    background: var(--accent);
    color: white;
    font-size: 0.875rem;
    margin-bottom: 1.25rem;

    &:hover {
      transform: translateY(-0.2em);
      box-shadow: inset 0 0 0 1.5em var(--hover-glow);
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
  background: var(--surface-inset);
  border: 1px solid var(--line-subtle);
  border-radius: var(--radius-panel);
  padding: 1rem 1rem 0.25rem;

  &-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 0.75rem;
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
  display: flex;
  gap: 0.625rem;
  margin-top: 1.25rem;

  .btn {
    flex: 1;
  }
}
</style>
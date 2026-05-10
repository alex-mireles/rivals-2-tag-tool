<script setup lang="ts">
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

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
        <h1 class="app-title">Rivals 2 Tag Tool</h1>
        <span class="app-version">v{{ appVersion }}</span>
      </div>

      <div class="save-path-row">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" class="save-path-icon">
          <path d="M2 2h5l1.5 2H14v10H2V2z" stroke="rgba(200,180,230,0.35)" stroke-width="1.2" />
        </svg>
        <span class="save-path-label">No save file loaded</span>
      </div>

      <button class="btn btn-primary" @click="loadTagNames">
        Load Tags from Save File
      </button>

      <div class="tag-panel">
        <div class="tag-panel-header">
          <span class="section-label">Player tags</span>
        </div>

        <div class="tag-panel-empty">
          <span class="empty-message">Load a save file to see tags</span>
        </div>
      </div>

      <div class="action-row">
        <button class="btn btn-ghost">Export selected</button>
        <button class="btn btn-ghost">Import .r2tag</button>
      </div>

    </div>
  </div>
</template>

<style>
/* Global resets and base styles */
*,
*::before,
*::after {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body, #app {
  height: 100%;
  overflow: hidden;
}

body {
  background-color: #120a1f;
  font-family: sans-serif;
  color: #f0ecf8;
}

button:focus-visible {
  outline: 2px solid rgba(99, 102, 241, 0.6);
  outline-offset: 2px;
}

/* Design tokens — must be global for rgba compositing to work against the page background */
:root {
  --surface:        rgba(255, 255, 255, 0.05);
  --surface-hover:  rgba(255, 255, 255, 0.08);
  --surface-inset:  rgba(0, 0, 0, 0.28);

  --accent:         rgba(99, 102, 241, 0.80);
  --accent-hover:   rgba(99, 102, 241, 0.90);

  --text-primary:   #f0ecf8;
  --text-secondary: #c8b8e8;
  --text-muted:     rgba(200, 180, 230, 0.60);

  --line:           rgba(255, 255, 255, 0.08);
  --line-subtle:    rgba(255, 255, 255, 0.04);
  --line-divider:   rgba(255, 255, 255, 0.05);

  --radius-card:    20px;
  --radius-panel:   12px;
  --radius-button:  10px;
}
</style>

<style scoped>
/* Layout */
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
}

/* Header */
.card-header {
  text-align: center;
  margin-bottom: 1.25rem;
}

.app-title {
  font-size: 26px;
  font-weight: 700;
  letter-spacing: -0.02em;
  margin-bottom: 0.125rem;
}

.app-version {
  font-size: 11px;
  letter-spacing: 0.1em;
  color: var(--text-muted);
}

/* Save path row */
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

/* Buttons */
.btn {
  width: 100%;
  cursor: pointer;
  font-size: 0.875rem;
  font-weight: 600;
  letter-spacing: 0.025em;
  border-radius: var(--radius-button);
  padding: 0.625rem 0;
  border: none;
  transition: background-color 150ms;
}

.btn-primary {
  background: var(--accent);
  color: white;
  margin-bottom: 1.25rem;
}

.btn-primary:hover {
  background: var(--accent-hover);
}

.btn-ghost {
  background: var(--surface);
  color: var(--text-secondary);
  border: 1px solid var(--line);
  font-size: 13px;
  font-weight: 500;
}

.btn-ghost:hover {
  background: var(--surface-hover);
}

/* Tag panel */
.tag-panel {
  background: var(--surface-inset);
  border: 1px solid var(--line-subtle);
  border-radius: var(--radius-panel);
  padding: 1rem 1rem 0.25rem;
}

.tag-panel-header {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: 0.75rem;
}

.section-label {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.1em;
  font-weight: 500;
  color: var(--text-muted);
}

.tag-panel-empty {
  text-align: center;
  border-top: 1px solid var(--line-divider);
  padding: 1.5rem 0;
}

.empty-message {
  font-size: 0.875rem;
  color: var(--text-secondary);
}

/* Action row */
.action-row {
  display: flex;
  gap: 0.625rem;
  margin-top: 1.25rem;
}

.action-row .btn {
  flex: 1;
}
</style>
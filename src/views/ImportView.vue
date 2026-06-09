<script setup lang="ts">
import { ref, computed, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import SavePathBar from '../components/SavePathBar.vue';
import ViewHeader from '../components/ViewHeader.vue';

interface TagPreview {
  path: string;
  tag_name: string;
}

interface ImportResult {
  imported: string[];
  skipped: string[];
}

const props = defineProps<{
  savePath: string;
  tagNames: string[];
}>();

const emit = defineEmits<{
  'go-back': [];
}>();

const previews = ref<TagPreview[]>([]);
const overwriteSet = ref<Set<string>>(new Set());
const isLoading = ref(false);
const isImporting = ref(false);
const result = ref<ImportResult | null>(null);
const errorMsg = ref('');

const conflictNames = computed(() =>
  new Set(previews.value.map(p => p.tag_name).filter(n => props.tagNames.includes(n)))
);

const hasConflicts = computed(() => conflictNames.value.size > 0);

async function chooseFiles() {
  errorMsg.value = '';
  previews.value = [];
  overwriteSet.value = new Set();
  result.value = null;
  isLoading.value = true;

  try {
    const paths = await open({
      multiple: true,
      title: 'Choose .r2tag Files',
      filters: [{ name: 'Tag file', extensions: ['r2tag'] }],
    });

    if (!paths || (Array.isArray(paths) && paths.length === 0)) return;

    const pathList = Array.isArray(paths) ? paths : [paths];
    previews.value = await invoke<TagPreview[]>('get_tag_previews', { r2tagPaths: pathList });
  } catch (err) {
    errorMsg.value = String(err);
  } finally {
    isLoading.value = false;
  }
}

function toggleOverwrite(name: string) {
  if (overwriteSet.value.has(name)) {
    overwriteSet.value.delete(name);
  } else {
    overwriteSet.value.add(name);
  }
  overwriteSet.value = new Set(overwriteSet.value);
}

async function doImport() {
  errorMsg.value = '';
  isImporting.value = true;
  await nextTick();

  try {
    const instructions = previews.value.map(p => ({
      path: p.path,
      tag_name: p.tag_name,
      overwrite: !conflictNames.value.has(p.tag_name) || overwriteSet.value.has(p.tag_name),
    }));

    result.value = await invoke<ImportResult>('import_tags', {
      savePath: props.savePath,
      instructions,
    });
  } catch (err) {
    errorMsg.value = String(err);
  } finally {
    isImporting.value = false;
  }
}

function reset() {
  previews.value = [];
  overwriteSet.value = new Set();
  result.value = null;
  errorMsg.value = '';
}
</script>

<template>
  <div class="card">
    <ViewHeader title="Import Tags" @go-back="emit('go-back')" />

    <SavePathBar :label="savePath" />

    <!-- Result panel -->
    <template v-if="result">
      <div class="result-panel">
        <div v-if="result.imported.length > 0" class="result-section result-section--success">
          <span class="result-section-label">Imported ({{ result.imported.length }})</span>
          <ul class="result-list">
            <li v-for="name in result.imported" :key="name" class="result-list-item result-list-item--imported">
              {{ name }}
            </li>
          </ul>
        </div>
        <div v-if="result.skipped.length > 0" class="result-section">
          <span class="result-section-label">Skipped ({{ result.skipped.length }})</span>
          <ul class="result-list">
            <li v-for="name in result.skipped" :key="name" class="result-list-item result-list-item--skipped">
              {{ name }}
            </li>
          </ul>
        </div>
      </div>
      <button class="btn btn-primary" @click="reset">Import More</button>
    </template>

    <!-- Loading: reading .r2tag previews -->
    <template v-else-if="isLoading">
      <div class="loading-panel">Reading tag files...</div>
    </template>

    <!-- Loading: writing to save -->
    <template v-else-if="isImporting">
      <div class="loading-panel">Writing to save file...</div>
    </template>

    <!-- Import UI -->
    <template v-else>
      <button class="btn btn-primary" @click="chooseFiles">
        Choose .r2tag Files
      </button>

      <template v-if="previews.length > 0">
        <div class="tag-panel">
          <div class="tag-panel-header">
            <span class="tag-panel-label">Tags to Import</span>
            <span v-if="hasConflicts" class="conflict-badge">
              {{ conflictNames.size }} conflict{{ conflictNames.size === 1 ? '' : 's' }}
            </span>
          </div>

          <ul class="tag-list">
            <li v-for="preview in previews" :key="preview.path" class="tag-row">
              <div class="tag-info">
                <span class="tag-name">{{ preview.tag_name }}</span>
                <span class="tag-source">{{ preview.path.split(/[\\/]/).pop() }}</span>
              </div>

              <div v-if="conflictNames.has(preview.tag_name)" class="conflict-toggle">
                <button
                  class="toggle-btn"
                  :class="{ 'toggle-btn--overwrite': overwriteSet.has(preview.tag_name) }"
                  @click="toggleOverwrite(preview.tag_name)"
                >
                  {{ overwriteSet.has(preview.tag_name) ? 'Overwrite' : 'Skip' }}
                </button>
              </div>
              <div v-else class="no-conflict-badge">New</div>
            </li>
          </ul>
        </div>

        <div v-if="hasConflicts" class="conflict-hint">
          Conflicting tags default to <strong>Skip</strong>. Toggle each one to overwrite instead.
        </div>
      </template>

      <div v-else-if="!errorMsg" class="empty-hint">
        Choose one or more <code>.r2tag</code> files to import into your save.
      </div>

      <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>

      <button v-if="previews.length > 0" class="btn btn-primary" @click="doImport">Import</button>
    </template>
  </div>
</template>

<style scoped lang="scss">
.conflict-badge {
  font-size: 0.85em;
  font-weight: 600;
  color: var(--text-warning);
  background: rgba(250, 204, 21, 0.1);
  border: 1px solid rgba(250, 204, 21, 0.25);
  border-radius: 0.5em;
  padding: 0.25em 0.6em;
}

.tag-row {
  justify-content: space-between;
}

.tag-info {
  display: flex;
  flex-direction: column;
  gap: 0.1em;
  min-width: 0;
}

.tag-name {
  font-size: 1.4em;
}

.tag-source {
  font-family: 'Ubuntu Sans Mono Variable', monospace;
  font-size: 0.85em;
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.conflict-toggle {
  flex-shrink: 0;
  margin-left: 0.5em;
}

.toggle-btn {
  padding: 0.3em 0.7em;
  border-radius: var(--radius-button);
  font-size: 0.85em;
  font-weight: 600;
  cursor: pointer;
  border: 1.5px solid rgba(250, 204, 21, 0.4);
  background: transparent;
  color: var(--text-warning);
  transition: background 0.15s, color 0.15s, border-color 0.15s;

  &--overwrite {
    background: rgba(248, 113, 113, 0.15);
    border-color: rgba(248, 113, 113, 0.5);
    color: var(--text-failure);
  }
}

.no-conflict-badge {
  flex-shrink: 0;
  font-size: 0.8em;
  font-weight: 600;
  color: var(--text-success);
  background: rgba(0, 255, 170, 0.08);
  border: 1px solid rgba(0, 255, 170, 0.2);
  border-radius: 0.4em;
  padding: 0.2em 0.5em;
}

.conflict-hint {
  width: 100%;
  font-size: 0.78em;
  color: var(--text-muted);
  padding: 0 0.25em;
}

.empty-hint {
  font-size: 0.85em;
  color: var(--text-muted);
  text-align: center;
  padding: 1em;

  code {
    font-family: 'Ubuntu Sans Mono Variable', monospace;
    background: var(--surface-inset);
    padding: 0.1em 0.3em;
    border-radius: 3px;
  }
}

.result-panel {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 0.75em;
}

.result-section {
  padding: 0.75em;
  border-radius: var(--radius-panel);
  background: var(--surface-inset);
  border: 1px solid var(--line-subtle);
  display: flex;
  flex-direction: column;
  gap: 0.4em;

  &--success {
    background: rgba(0, 255, 170, 0.05);
    border-color: rgba(0, 255, 170, 0.15);
  }

  &-label {
    font-size: 0.75em;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-muted);
    font-weight: 600;
  }
}

.result-list-item {
  font-size: 0.9em;
  font-weight: 600;

  &--imported {
    color: var(--text-success);

    &::before {
      content: '✓ ';
    }
  }

  &--skipped {
    color: var(--text-muted);

    &::before {
      content: '– ';
    }
  }
}
</style>

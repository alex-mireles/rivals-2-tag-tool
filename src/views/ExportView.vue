<script setup lang="ts">
import { ref, computed, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import SavePathBar from '../components/SavePathBar.vue';

const props = defineProps<{
  savePath: string;
  tagNames: string[];
}>();

const emit = defineEmits<{
  'go-back': [];
}>();

const selected = ref<Set<string>>(new Set());
const isExporting = ref(false);
const result = ref<{ exported: string[]; outputDir: string } | null>(null);
const errorMsg = ref('');

const allSelected = computed(() => selected.value.size === props.tagNames.length);

function toggleTag(name: string) {
  if (selected.value.has(name)) {
    selected.value.delete(name);
  } else {
    selected.value.add(name);
  }
  // Trigger reactivity for Set mutation
  selected.value = new Set(selected.value);
}

function toggleAll() {
  if (allSelected.value) {
    selected.value = new Set();
  } else {
    selected.value = new Set(props.tagNames);
  }
}

async function exportSelected() {
  errorMsg.value = '';
  result.value = null;

  const outputDir = await open({ directory: true, title: 'Choose Export Folder' });
  if (!outputDir) return;

  isExporting.value = true;
  await nextTick();
  try {
    const exported = await invoke<string[]>('export_tags', {
      savePath: props.savePath,
      tagNames: [...selected.value],
      outputDir,
    });
    result.value = { exported, outputDir: outputDir as string };
  } catch (err) {
    errorMsg.value = String(err);
  } finally {
    isExporting.value = false;
  }
}

function reset() {
  result.value = null;
  errorMsg.value = '';
  selected.value = new Set();
}
</script>

<template>
  <div class="card">
    <div class="card-view-header">
      <button class="back-btn" @click="emit('go-back')" title="Back">
        <!-- back arrow -->
        <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24">
          <path fill="currentColor" d="M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z"/>
        </svg>
      </button>
      <span class="card-view-header-title">Export Tags</span>
    </div>

    <SavePathBar :save-path="savePath" />

    <!-- Success result panel -->
    <template v-if="result">
      <div class="result-panel result-panel--success">
        <span class="result-panel-msg">
          Exported {{ result.exported.length }} tag{{ result.exported.length === 1 ? '' : 's' }} to
          <span class="result-panel-path">{{ result.outputDir }}</span>
        </span>
        <ul class="result-list">
          <li v-for="path in result.exported" :key="path" class="result-list-item">
            {{ path.split('\\').pop() }}
          </li>
        </ul>
      </div>
      <button class="btn btn-primary" @click="reset">Export More</button>
    </template>

    <!-- Loading state -->
    <template v-else-if="isExporting">
      <div class="loading-panel">Writing .r2tag files...</div>
    </template>

    <!-- Selection UI -->
    <template v-else>
      <div class="tag-panel">
        <div class="tag-panel-header">
          <span class="tag-panel-label">Select Tags to Export</span>
          <button class="select-all-btn" @click="toggleAll">
            {{ allSelected ? 'Deselect All' : 'Select All' }}
          </button>
        </div>
        <ul class="tag-list">
          <li
            v-for="name in tagNames"
            :key="name"
            class="tag-row tag-row--selectable"
            @click="toggleTag(name)"
          >
            <div class="tag-checkbox" :class="{ 'tag-checkbox--checked': selected.has(name) }">
              <svg v-if="selected.has(name)" xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24">
                <path fill="currentColor" d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
              </svg>
            </div>
            <span class="tag-name">{{ name }}</span>
          </li>
        </ul>
      </div>

      <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>

      <button
        class="btn btn-primary"
        :disabled="selected.size === 0"
        @click="exportSelected"
      >
        Export {{ selected.size > 0 ? selected.size : '' }} Selected Tag{{ selected.size === 1 ? '' : 's' }}
      </button>
    </template>
  </div>
</template>

<style scoped lang="scss">
.tag-panel {
  display: flex;
  flex-direction: column;
  width: 100%;
  font-size: 0.75rem;
  padding: 0.75rem 1rem 0.25rem;
  background: var(--surface-inset);
  border: 1px solid var(--line-subtle);
  border-radius: var(--radius-panel);

  &-header {
    flex-shrink: 0;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--line-divider);
    padding-bottom: 0.4em;
    margin-bottom: 0.25em;
    min-height: 2.5em;
  }

  &-label {
    font-size: 1em;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    font-weight: 400;
    color: var(--text-muted);
  }
}

.select-all-btn {
  background: none;
  border: 1px solid var(--line-subtle);
  color: var(--text-muted);
  font-size: 0.85em;
  padding: 0.25em 0.6em;
  border-radius: var(--radius-button);
  cursor: pointer;
  transition: color 0.15s, border-color 0.15s;

  &:hover {
    color: var(--text-primary);
    border-color: var(--accent);
  }
}

.tag-list {
  list-style: none;
  max-height: 14rem;
  overflow-y: auto;
}

.tag-row {
  display: flex;
  align-items: center;
  padding: 0.5em 0.25em;
  border-bottom: 1px solid var(--line-divider);
  gap: 0.75em;

  &:last-child {
    border-bottom: none;
  }

  &--selectable {
    cursor: pointer;

    &:hover {
      background: var(--surface-hover);
    }
  }
}

.tag-checkbox {
  width: 1.1em;
  height: 1.1em;
  flex-shrink: 0;
  border: 1.5px solid rgba(255, 255, 255, 0.25);
  border-radius: 3px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s, border-color 0.15s;

  &--checked {
    background: var(--accent);
    border-color: var(--accent);
  }
}

.tag-name {
  font-family: 'Consolas';
  font-size: 1.5em;
  font-weight: 700;
  color: var(--text-secondary);
}

.error-msg {
  width: 100%;
  padding: 0.6em 0.8em;
  background: rgba(248, 113, 113, 0.1);
  border: 1px solid rgba(248, 113, 113, 0.3);
  border-radius: var(--radius-button);
  font-size: 0.8em;
  color: var(--text-failure);
}

.result-panel {
  width: 100%;
  padding: 1em;
  border-radius: var(--radius-panel);
  display: flex;
  flex-direction: column;
  gap: 0.5em;

  &--success {
    background: rgba(0, 255, 170, 0.06);
    border: 1px solid rgba(0, 255, 170, 0.2);
  }

  &-msg {
    font-size: 0.85em;
    color: var(--text-success);
  }

  &-path {
    font-family: monospace;
    word-break: break-all;
  }
}

.result-list {
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 0.2em;

  &-item {
    font-family: monospace;
    font-size: 0.8em;
    color: var(--text-muted);
    padding-left: 0.5em;

    &::before {
      content: '✓ ';
      color: var(--text-success);
    }
  }
}
</style>

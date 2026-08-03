<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { ImportResult, TagPreview } from '../types';

const props = defineProps<{
  savePath: string;
  tagNames: string[];
  previews: TagPreview[];
  saveVersion: number | null;
  resetLabel?: string;
}>();

const emit = defineEmits<{
  reset: [];
  finished: [result: ImportResult];
}>();

const overwriteSet = ref<Set<string>>(new Set());
const isImporting = ref(false);
const result = ref<ImportResult | null>(null);
const errorMsg = ref('');

const compatiblePreviews = computed(() => props.previews.filter((preview) => preview.compatible));
const incompatibleCount = computed(() => props.previews.length - compatiblePreviews.value.length);
const conflictNames = computed(() => new Set(
  compatiblePreviews.value.map((preview) => preview.tag_name).filter((name) => props.tagNames.includes(name)),
));
const allOverwrite = computed(() => conflictNames.value.size > 0 && [...conflictNames.value].every((name) => overwriteSet.value.has(name)));

function toggleOverwrite(name: string) {
  const next = new Set(overwriteSet.value);
  if (next.has(name)) next.delete(name); else next.add(name);
  overwriteSet.value = next;
}

function toggleAllConflicts() {
  overwriteSet.value = allOverwrite.value ? new Set() : new Set(conflictNames.value);
}

async function doImport() {
  errorMsg.value = '';
  isImporting.value = true;
  try {
    const instructions = compatiblePreviews.value.map((preview) => ({
      path: preview.path,
      tag_name: preview.tag_name,
      overwrite: !conflictNames.value.has(preview.tag_name) || overwriteSet.value.has(preview.tag_name),
    }));
    result.value = await invoke<ImportResult>('import_tags', { savePath: props.savePath, instructions });
    emit('finished', result.value);
  } catch (error) {
    errorMsg.value = String(error);
  } finally {
    isImporting.value = false;
  }
}
</script>

<template>
  <div v-if="result" class="view-stack">
    <div class="result-panel">
      <div v-if="result.imported.length" class="result-section result-section--success">
        <span>Imported ({{ result.imported.length }})</span>
        <ul class="result-list"><li v-for="name in result.imported" :key="name">✓ {{ name }}</li></ul>
      </div>
      <div v-if="result.skipped.length" class="result-section">
        <span>Skipped ({{ result.skipped.length }})</span>
        <ul class="result-list"><li v-for="name in result.skipped" :key="name">– {{ name }}</li></ul>
      </div>
      <div v-if="result.incompatible.length" class="result-section">
        <span>Incompatible ({{ result.incompatible.length }})</span>
        <ul class="result-list"><li v-for="name in result.incompatible" :key="name">✕ {{ name }}</li></ul>
      </div>
    </div>
    <button class="btn btn-primary" @click="emit('reset')">{{ resetLabel ?? 'Import More' }}</button>
  </div>

  <div v-else-if="isImporting" class="loading-panel">Writing to save file...</div>

  <div v-else class="view-stack">
    <div class="tag-panel">
      <div class="tag-panel-header">
        <span class="tag-panel-label">Tags to Import <small v-if="saveVersion !== null">Save v{{ saveVersion }}</small></span>
        <button v-if="conflictNames.size" class="small-btn" @click="toggleAllConflicts">{{ allOverwrite ? 'Skip All' : 'Overwrite All' }}</button>
      </div>
      <ul class="tag-list">
        <li v-for="preview in previews" :key="preview.path" class="tag-row review-row" :class="{ incompatible: !preview.compatible }">
          <div><strong>{{ preview.tag_name }}</strong><small>{{ preview.path.split(/[\\/]/).pop() }}</small></div>
          <span v-if="!preview.compatible" class="badge badge--error">{{ preview.version === null ? 'Unknown version' : `v${preview.version}` }}</span>
          <button v-else-if="conflictNames.has(preview.tag_name)" class="small-btn" @click="toggleOverwrite(preview.tag_name)">
            {{ overwriteSet.has(preview.tag_name) ? 'Overwrite' : 'Skip' }}
          </button>
          <span v-else class="badge">New</span>
        </li>
      </ul>
    </div>
    <p v-if="conflictNames.size" class="hint">Conflicts default to <strong>Skip</strong>.</p>
    <p v-if="incompatibleCount" class="hint"><strong>{{ incompatibleCount }}</strong> tag(s) were saved under a different game version and cannot be imported.</p>
    <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>
    <button class="btn btn-primary" :disabled="compatiblePreviews.length === 0" @click="doImport">
      Import {{ compatiblePreviews.length }} Compatible Tag{{ compatiblePreviews.length === 1 ? '' : 's' }}
    </button>
  </div>
</template>

<style scoped lang="scss">
.review-row { justify-content: space-between; gap: 0.75rem; }
.review-row > div { min-width: 0; display: flex; flex-direction: column; }
.review-row small { color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.incompatible { opacity: 0.55; }
.small-btn { border: 1px solid var(--line); background: var(--surface-hover); color: var(--text-primary); border-radius: 0.4rem; padding: 0.25rem 0.5rem; cursor: pointer; }
.badge { color: var(--text-success); font-weight: 700; &--error { color: var(--text-failure); } }
.hint { width: 100%; color: var(--text-muted); font-size: 0.78rem; }
.result-panel { width: 100%; display: flex; flex-direction: column; gap: 0.6rem; }
.result-section { padding: 0.75rem; border: 1px solid var(--line); border-radius: var(--radius-panel); background: var(--surface-inset); font-size: 0.8rem; &--success { border-color: rgba(0,255,170,.2); } }
.result-section > span { color: var(--text-muted); text-transform: uppercase; letter-spacing: .08em; }
.result-list { margin-top: .4rem; }
</style>

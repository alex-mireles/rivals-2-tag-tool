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

// A conflict is anything whose import replaces a tag that would otherwise
// survive — including a name claimed by an earlier row of this same batch. Two
// files carrying the same tag name (two "Zetter"s from different players) would
// otherwise both read as "New" and the second would silently overwrite the
// first, while the result panel claimed two imports.
const conflictNames = computed(() => {
  const seen = new Set<string>();
  const conflicts = new Set<string>();
  for (const { tag_name: name } of compatiblePreviews.value) {
    if (props.tagNames.includes(name) || seen.has(name)) conflicts.add(name);
    seen.add(name);
  }
  return conflicts;
});
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
    <p v-if="result.imported.length" class="hint">
      Restart Rivals 2 if it is open — it rewrites this file on exit and would discard these tags.
    </p>
    <button class="btn btn-primary" @click="emit('reset')">
      <v-icon name="md-refresh-round" scale="0.85" />
      {{ resetLabel ?? 'Import More' }}
    </button>
  </div>

  <div v-else-if="isImporting" class="loading-panel">Writing to save file...</div>

  <div v-else class="view-stack">
    <!-- Provenance / compatibility context from the caller (e.g. a .r2pack). -->
    <slot name="banner" />
    <div class="tag-panel">
      <div class="tag-panel-header">
        <span class="tag-panel-label">Tags to Import</span>
        <!-- The version rule only matters when a tag breaks it, and a broken
             tag already says so on its own row. So it sits here as a bare
             fact, with the rule itself one hover away. -->
        <span
          v-if="saveVersion !== null"
          class="save-version"
          :data-tooltip="`Your save file is version ${saveVersion}. A tag can only be imported if it was saved under the same version.`"
        >
          Save v{{ saveVersion }}
        </span>
        <button v-if="conflictNames.size" class="panel-btn" @click="toggleAllConflicts">
          <v-icon name="md-doneall-round" scale="0.7" />
          {{ allOverwrite ? 'Skip All' : 'Overwrite All' }}
        </button>
      </div>
      <ul class="tag-list">
        <li v-for="preview in previews" :key="preview.path" class="tag-row review-row" :class="{ incompatible: !preview.compatible }">
          <div><strong>{{ preview.tag_name }}</strong><small>{{ preview.error ?? preview.path.split(/[\\/]/).pop() }}</small></div>
          <!-- Generic on purpose: an error row is now either an unreadable file
               or a tag this app refuses to import. The reason sits under the
               name, so the badge only has to say the row is out. -->
          <span v-if="preview.error" class="badge badge--error">Can’t import</span>
          <span v-else-if="!preview.compatible" class="badge badge--error">{{ preview.version === null ? 'Unknown version' : `v${preview.version}` }}</span>
          <!-- Colour carries the outcome: yellow keeps the tag you already
               have, red replaces it. Both are one click from the other, so the
               label alone is easy to skim past. -->
          <button
            v-else-if="conflictNames.has(preview.tag_name)"
            class="panel-btn conflict-btn"
            :class="overwriteSet.has(preview.tag_name) ? 'conflict-btn--overwrite' : 'conflict-btn--skip'"
            @click="toggleOverwrite(preview.tag_name)"
          >
            <v-icon name="md-swaphoriz-round" scale="0.7" />
            {{ overwriteSet.has(preview.tag_name) ? 'Overwrite' : 'Skip' }}
          </button>
          <span v-else class="badge">New</span>
        </li>
      </ul>
    </div>
    <!-- Sits directly under the Skip/Overwrite column it explains, close enough
         to the panel to read as its caption rather than as another footnote. -->
    <p v-if="conflictNames.size" class="hint hint--caption">
      Conflicts default to <strong>Skip</strong>.
    </p>

    <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>
    <button class="btn btn-primary" :disabled="compatiblePreviews.length === 0" @click="doImport">
      <v-icon name="md-download-round" scale="0.85" />
      Import {{ compatiblePreviews.length }} Compatible Tag{{ compatiblePreviews.length === 1 ? '' : 's' }}
    </button>
  </div>
</template>

<style scoped lang="scss">
.review-row { justify-content: space-between; gap: 0.75rem; }
.review-row > div { min-width: 0; display: flex; flex-direction: column; }
.review-row small { color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.incompatible { opacity: 0.55; }
// Shape comes from .panel-btn so it lines up with every other panel-header
// control; only the colours are its own.
.conflict-btn {
  font-weight: 600;
  transition: background 500ms, color 500ms, border-color 500ms;

  &--skip {
    border-color: rgba(250, 204, 21, 0.4);
    color: var(--text-warning);

    &:hover { border-color: rgba(250, 204, 21, 0.85); color: var(--text-warning); }
  }

  &--overwrite {
    background: rgba(248, 113, 113, 0.15);
    border-color: rgba(248, 113, 113, 0.5);
    color: var(--text-failure);

    &:hover { border-color: rgba(248, 113, 113, 0.9); color: var(--text-failure); }
  }
}
.badge { color: var(--text-success); font-weight: 700; &--error { color: var(--text-failure); } }
.hint {
  width: 100%;
  color: var(--text-muted);
  font-size: 0.78rem;
  line-height: 1.5;

  // Caption for the panel above rather than a footnote of its own: pulled up
  // against the panel and aligned to the column it describes.
  &--caption {
    margin-top: -0.7rem;
    padding-right: 0.15rem;
    text-align: right;
  }
}
// The header holds three things now, so it needs a gap and the version pushed
// up against the title instead of drifting into the middle.
.tag-panel-header { gap: 0.6rem; }
.save-version {
  margin-right: auto;
  color: var(--text-muted);
  font-size: 0.72rem;
  letter-spacing: 0.04em;
  white-space: nowrap;
  border-bottom: 1px dotted var(--line);
  cursor: help;

  &:hover { color: var(--text-primary); }
}
.result-panel { width: 100%; display: flex; flex-direction: column; gap: 0.6rem; }
.result-section { padding: 0.75rem; border: 1px solid var(--line); border-radius: var(--radius-panel); background: var(--surface-inset); font-size: 0.8rem; &--success { border-color: rgba(0,255,170,.2); } }
.result-section > span { color: var(--text-muted); text-transform: uppercase; letter-spacing: .08em; }
.result-list { margin-top: .4rem; }
</style>

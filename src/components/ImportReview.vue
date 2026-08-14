<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { ImportResult, TagPreview } from '../types';

const CUSTOM_TAG_LIMIT = 96;
type ImportMode = 'merge' | 'replace-custom';

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
const mode = ref<ImportMode>('merge');
const selectedPaths = ref<Set<string>>(initialSelection());
const confirmingReplace = ref(false);
const isImporting = ref(false);
const result = ref<ImportResult | null>(null);
const errorMsg = ref('');

const compatiblePreviews = computed(() => props.previews.filter((preview) => preview.compatible));
const selectedPreviews = computed(() =>
  compatiblePreviews.value.filter((preview) => selectedPaths.value.has(preview.path)),
);
const existingNames = computed(() => new Set(props.tagNames));

function initialSelection(): Set<string> {
  const selected = new Set<string>();
  const names = new Set<string>();
  for (const preview of props.previews) {
    if (preview.compatible && !names.has(preview.tag_name)) {
      selected.add(preview.path);
      names.add(preview.tag_name);
    }
  }
  return selected;
}

const conflictNames = computed(() => {
  if (mode.value === 'replace-custom') return new Set<string>();
  return new Set(
    selectedPreviews.value
      .map((preview) => preview.tag_name)
      .filter((name) => existingNames.value.has(name)),
  );
});
const allOverwrite = computed(() => conflictNames.value.size > 0 && [...conflictNames.value].every((name) => overwriteSet.value.has(name)));
const selectedImportCount = computed(() => {
  if (mode.value === 'replace-custom') return selectedPreviews.value.length;
  return selectedPreviews.value.filter(
    (preview) => !existingNames.value.has(preview.tag_name) || overwriteSet.value.has(preview.tag_name),
  ).length;
});
const addedCustomCount = computed(() =>
  selectedPreviews.value.filter((preview) => !existingNames.value.has(preview.tag_name)).length,
);
const finalCustomCount = computed(() => {
  if (mode.value === 'replace-custom') return selectedPreviews.value.length;
  return props.tagNames.length + addedCustomCount.value;
});
const overCapacity = computed(() =>
  finalCustomCount.value > CUSTOM_TAG_LIMIT
  && (mode.value === 'replace-custom' || addedCustomCount.value > 0),
);
const remainingSlots = computed(() => Math.max(0, CUSTOM_TAG_LIMIT - finalCustomCount.value));
const removedCount = computed(() => props.tagNames.length);

function setMode(next: ImportMode) {
  mode.value = next;
  confirmingReplace.value = false;
}

function toggleIncluded(preview: TagPreview) {
  const next = new Set(selectedPaths.value);
  if (next.has(preview.path)) {
    next.delete(preview.path);
  } else {
    // Only one file with a given tag name can determine the saved settings.
    for (const candidate of compatiblePreviews.value) {
      if (candidate.tag_name === preview.tag_name) next.delete(candidate.path);
    }
    next.add(preview.path);
  }
  selectedPaths.value = next;
  confirmingReplace.value = false;
}

function isAlternative(preview: TagPreview): boolean {
  return compatiblePreviews.value.some(
    (candidate) =>
      candidate.path !== preview.path
      && candidate.tag_name === preview.tag_name
      && selectedPaths.value.has(candidate.path),
  );
}

function toggleOverwrite(name: string) {
  const next = new Set(overwriteSet.value);
  if (next.has(name)) next.delete(name); else next.add(name);
  overwriteSet.value = next;
}

function toggleAllConflicts() {
  overwriteSet.value = allOverwrite.value ? new Set() : new Set(conflictNames.value);
}

async function doImport() {
  if (mode.value === 'replace-custom' && !confirmingReplace.value) {
    confirmingReplace.value = true;
    return;
  }
  errorMsg.value = '';
  isImporting.value = true;
  try {
    const instructions = selectedPreviews.value.map((preview) => ({
      path: preview.path,
      tag_name: preview.tag_name,
      overwrite: !conflictNames.value.has(preview.tag_name) || overwriteSet.value.has(preview.tag_name),
    }));
    result.value = await invoke<ImportResult>('import_tags', {
      savePath: props.savePath,
      instructions,
      mode: mode.value,
    });
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
        <ul class="result-list"><li v-for="(name, index) in result.imported" :key="index">✓ {{ name }}</li></ul>
      </div>
      <div v-if="result.skipped.length" class="result-section">
        <span>Skipped ({{ result.skipped.length }})</span>
        <ul class="result-list"><li v-for="(name, index) in result.skipped" :key="index">– {{ name }}</li></ul>
      </div>
      <div v-if="result.incompatible.length" class="result-section">
        <span>Incompatible ({{ result.incompatible.length }})</span>
        <ul class="result-list"><li v-for="(name, index) in result.incompatible" :key="index">✕ {{ name }}</li></ul>
      </div>
      <div v-if="result.removed.length" class="result-section result-section--removed">
        <span>Removed ({{ result.removed.length }})</span>
        <ul class="result-list"><li v-for="(name, index) in result.removed" :key="index">− {{ name }}</li></ul>
      </div>
    </div>
    <p v-if="result.backup_path" class="hint">
      Backup created at <span class="backup-path">{{ result.backup_path }}</span>
    </p>
    <p v-if="result.imported.length || result.removed.length" class="hint">
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
    <div class="mode-picker" role="group" aria-label="Import mode">
      <button
        class="mode-btn"
        :class="{ 'mode-btn--active': mode === 'merge' }"
        @click="setMode('merge')"
      >
        Merge with current save file
      </button>
      <button
        class="mode-btn"
        :class="{ 'mode-btn--active': mode === 'replace-custom' }"
        @click="setMode('replace-custom')"
      >
        Overwite save file from scratch
      </button>
    </div>
    <p class="hint mode-hint">
      <template v-if="mode === 'merge'">Keep existing tags - add new tags or overwrite old ones.</template>
      <template v-else>Remove all tags, then import new ones.</template>
    </p>
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
          Tag Save File v{{ saveVersion }}
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
          <div v-if="preview.compatible" class="row-actions">
            <button
              class="panel-btn include-btn"
              :class="{ 'include-btn--selected': selectedPaths.has(preview.path) }"
              :aria-pressed="selectedPaths.has(preview.path)"
              @click="toggleIncluded(preview)"
            >
              <v-icon v-if="selectedPaths.has(preview.path)" name="md-check-round" scale="0.7" />
              {{ selectedPaths.has(preview.path) ? 'Included' : 'Include' }}
            </button>
            <!-- Colour carries the outcome: yellow keeps the tag you already
                 have, red replaces it. Both are one click from the other, so the
                 label alone is easy to skim past. -->
            <button
              v-if="selectedPaths.has(preview.path) && conflictNames.has(preview.tag_name)"
              class="panel-btn conflict-btn"
              :class="overwriteSet.has(preview.tag_name) ? 'conflict-btn--overwrite' : 'conflict-btn--skip'"
              @click="toggleOverwrite(preview.tag_name)"
            >
              <v-icon name="md-swaphoriz-round" scale="0.7" />
              {{ overwriteSet.has(preview.tag_name) ? 'Overwrite' : 'Skip' }}
            </button>
            <span v-else-if="selectedPaths.has(preview.path)" class="badge">
              {{ mode === 'replace-custom' ? 'Import' : 'New' }}
            </span>
            <span v-else class="badge badge--muted">
              {{ isAlternative(preview) ? 'Alternative' : 'Excluded' }}
            </span>
          </div>
          <span v-else-if="preview.error" class="badge badge--error">Can’t import</span>
          <span v-else class="badge badge--error">{{ preview.version === null ? 'Unknown version' : `v${preview.version}` }}</span>
        </li>
      </ul>
    </div>
    <!-- Sits directly under the Skip/Overwrite column it explains, close enough
         to the panel to read as its caption rather than as another footnote. -->
    <p v-if="conflictNames.size" class="hint hint--caption">
      Conflicts default to <strong>Skip</strong>.
    </p>

    <p class="capacity" :class="{ 'capacity--error': overCapacity }">
      {{ finalCustomCount }} / {{ CUSTOM_TAG_LIMIT }} custom tag slots used
      <template v-if="finalCustomCount <= CUSTOM_TAG_LIMIT"> · {{ remainingSlots }} remaining</template>
      <template v-else-if="!overCapacity">
        · this save is already over the limit; existing tags can still be overwritten
      </template>
      <template v-else-if="mode === 'merge' && tagNames.length > CUSTOM_TAG_LIMIT">
        · this save is already over the limit; replace its custom tags before adding more
      </template>
      <template v-else>
        · deselect {{ finalCustomCount - CUSTOM_TAG_LIMIT }} tag{{ finalCustomCount - CUSTOM_TAG_LIMIT === 1 ? '' : 's' }}
      </template>
    </p>

    <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>
    <div v-if="confirmingReplace" class="confirm">
      <span class="confirm-text">
        Remove {{ removedCount }} existing custom tag{{ removedCount === 1 ? '' : 's' }} and import
        {{ selectedImportCount }} selected tag{{ selectedImportCount === 1 ? '' : 's' }}?
        Player1–Player4 will be kept. A backup will be created first.
      </span>
      <button class="confirm-btn" @click="confirmingReplace = false">
        <v-icon name="md-close-round" scale="0.7" />
        Cancel
      </button>
      <button class="confirm-btn confirm-btn--danger" @click="doImport">
        <v-icon name="md-delete-round" scale="0.7" />
        Replace
      </button>
    </div>
    <button
      v-else
      class="btn btn-primary"
      :class="{ 'danger-btn': mode === 'replace-custom' }"
      :disabled="selectedImportCount === 0 || overCapacity"
      @click="doImport"
    >
      <v-icon name="md-download-round" scale="0.85" />
      {{ mode === 'replace-custom' ? 'Replace with' : 'Import' }}
      {{ selectedImportCount }} Tag{{ selectedImportCount === 1 ? '' : 's' }}
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
.row-actions { flex-shrink: 0; flex-direction: row !important; align-items: center; gap: 0.4rem; }
.include-btn {
  color: var(--text-muted);

  &--selected { color: var(--text-primary); border-color: var(--accent); }
}
.badge {
  color: var(--text-success);
  font-weight: 700;

  &--error { color: var(--text-failure); }
  &--muted { color: var(--text-muted); }
}
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
  // `currentColor`, not `--line`: at 8% white the underline was invisible, so
  // the only thing advertising the tooltip was a cursor you had to already be
  // hovering to see. Following the text also means it brightens on hover.
  border-bottom: 1px dotted currentColor;
  cursor: help;

  &:hover { color: var(--text-primary); }
}
.mode-picker {
  width: 100%;
  display: grid;
  grid-template-columns: 1fr 1fr;
  padding: 0.2rem;
  border: 1px solid var(--line);
  border-radius: var(--radius-button);
  background: var(--surface-inset);
}
.mode-btn {
  padding: 0.45rem 0.7rem;
  border: 0;
  border-radius: calc(var(--radius-button) - 0.2rem);
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: 0.8rem;
  font-weight: 600;
  cursor: pointer;

  &--active { background: var(--surface-hover); color: var(--text-primary); }
}
.mode-hint { margin-top: -0.65rem; text-align: center; }
.capacity {
  width: 100%;
  margin-top: -0.3rem;
  color: var(--text-muted);
  font-size: 0.78rem;
  text-align: center;

  &--error { color: var(--text-failure); font-weight: 600; }
}
.danger-btn {
  border-color: rgba(248, 113, 113, 0.4);
  background: rgba(248, 113, 113, 0.1);
  color: var(--text-failure);

  &:hover { background: rgba(248, 113, 113, 0.18); border-color: rgba(248, 113, 113, 0.65); }
}
.confirm {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.6rem 0.7rem;
  border: 1px solid rgba(248, 113, 113, 0.4);
  border-radius: 0.4rem;
  background: rgba(248, 113, 113, 0.1);
}
.confirm-text { flex: 1; min-width: 0; font-size: 0.78rem; color: var(--text-failure); }
.confirm-btn {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.3rem 0.6rem;
  border: 1px solid var(--line);
  border-radius: 0.4rem;
  background: var(--surface-hover);
  color: var(--text-primary);
  font-size: 0.75rem;
  cursor: pointer;

  &--danger { border-color: rgba(248, 113, 113, 0.5); color: var(--text-failure); }
}
.backup-path { overflow-wrap: anywhere; color: var(--text-primary); }
.result-panel { width: 100%; display: flex; flex-direction: column; gap: 0.6rem; }
.result-section { padding: 0.75rem; border: 1px solid var(--line); border-radius: var(--radius-panel); background: var(--surface-inset); font-size: 0.8rem; &--success { border-color: rgba(0,255,170,.2); } }
.result-section--removed { border-color: rgba(248, 113, 113, 0.3); }
.result-section > span { color: var(--text-muted); text-transform: uppercase; letter-spacing: .08em; }
.result-list { margin-top: .4rem; }
</style>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import TagDiff from './TagDiff.vue';

interface TagPreview {
  path: string;
  tag_name: string;
  version: number | null;
  compatible: boolean;
}

interface PreviewResult {
  save_version: number | null;
  previews: TagPreview[];
}

interface ImportResult {
  imported: string[];
  skipped: string[];
  incompatible: string[];
}

const props = defineProps<{
  savePath: string;
  /** Tag names already in the destination save, for conflict detection. */
  existingTagNames: string[];
  /** .r2tag files to preview + import. Empty renders nothing. */
  paths: string[];
  /**
   * Optional file path -> publisher's start.gg handle. Supplied for tags pulled
   * from the shared database; used to tell apart two people who happen to have
   * chosen the same in-game tag name.
   */
  startggHandles?: Record<string, string>;
}>();

const emit = defineEmits<{ restart: [] }>();

const previews = ref<TagPreview[]>([]);
const saveVersion = ref<number | null>(null);
const overwriteSet = ref<Set<string>>(new Set());
const isLoading = ref(false);
const isImporting = ref(false);
const result = ref<ImportResult | null>(null);
const errorMsg = ref('');

// Only version-matched tags can be imported; the rest are blocked.
const compatiblePreviews = computed(() => previews.value.filter(p => p.compatible));
const incompatibleCount = computed(() => previews.value.length - compatiblePreviews.value.length);
const hasCompatible = computed(() => compatiblePreviews.value.length > 0);

/**
 * Two different people can end up with the same in-game tag name by pure
 * coincidence — it's just text they typed, unrelated to their start.gg account.
 * A save holds one tag per name, so without help the second import would
 * overwrite (or be skipped behind) the first. Where we know a start.gg handle
 * for a colliding tag, install it under that handle instead so both land.
 * Tags with no known handle keep the plain overwrite/skip behaviour.
 */
const renames = computed(() => {
  const byName = new Map<string, TagPreview[]>();
  for (const p of compatiblePreviews.value) {
    if (!byName.has(p.tag_name)) byName.set(p.tag_name, []);
    byName.get(p.tag_name)!.push(p);
  }
  const out: Record<string, string> = {};
  for (const group of byName.values()) {
    if (group.length < 2) continue;
    for (const p of group) {
      const handle = props.startggHandles?.[p.path];
      if (handle && handle !== p.tag_name) out[p.path] = handle;
    }
  }
  return out;
});

const conflictNames = computed(() =>
  new Set(
    compatiblePreviews.value
      // A renamed tag goes in under a different name, so it no longer collides
      // with whatever is already in the save under its original name.
      .filter(p => !renames.value[p.path])
      .map(p => p.tag_name)
      .filter(n => props.existingTagNames.includes(n)),
  )
);
const hasConflicts = computed(() => conflictNames.value.size > 0);
const allOverwrite = computed(() =>
  conflictNames.value.size > 0 && [...conflictNames.value].every(n => overwriteSet.value.has(n))
);

// Whenever the set of files changes, re-read previews against the save.
watch(
  () => props.paths,
  async (paths) => {
    result.value = null;
    errorMsg.value = '';
    overwriteSet.value = new Set();
    previews.value = [];
    saveVersion.value = null;
    if (!paths || paths.length === 0) return;

    isLoading.value = true;
    await nextTick();
    try {
      const res = await invoke<PreviewResult>('get_tag_previews', {
        r2tagPaths: paths,
        savePath: props.savePath,
      });
      previews.value = res.previews;
      saveVersion.value = res.save_version;
    } catch (err) {
      errorMsg.value = String(err);
    } finally {
      isLoading.value = false;
    }
  },
  { immediate: true }
);

function toggleAllConflicts() {
  overwriteSet.value = allOverwrite.value ? new Set() : new Set(conflictNames.value);
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
    const instructions = compatiblePreviews.value.map(p => ({
      path: p.path,
      tag_name: p.tag_name,
      overwrite: !conflictNames.value.has(p.tag_name) || overwriteSet.value.has(p.tag_name),
      rename: renames.value[p.path] ?? null,
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

function importMore() {
  result.value = null;
  emit('restart');
}
</script>

<template>
  <Transition name="content-swap" mode="out-in">
    <!-- Result -->
    <div v-if="result" key="result" class="view-stack">
      <div class="result-panel">
        <div v-if="result.imported.length > 0" class="result-section result-section--success">
          <span class="result-section-label">Imported ({{ result.imported.length }})</span>
          <ul class="result-list">
            <li v-for="name in result.imported" :key="name" class="result-list-item result-list-item--imported">{{ name }}</li>
          </ul>
        </div>
        <div v-if="result.skipped.length > 0" class="result-section">
          <span class="result-section-label">Skipped ({{ result.skipped.length }})</span>
          <ul class="result-list">
            <li v-for="name in result.skipped" :key="name" class="result-list-item result-list-item--skipped">{{ name }}</li>
          </ul>
        </div>
        <div v-if="result.incompatible.length > 0" class="result-section">
          <span class="result-section-label">Incompatible ({{ result.incompatible.length }})</span>
          <ul class="result-list">
            <li v-for="name in result.incompatible" :key="name" class="result-list-item result-list-item--incompatible">{{ name }}</li>
          </ul>
        </div>
      </div>
      <button class="btn btn-primary" @click="importMore">Import More</button>
    </div>

    <!-- Loading -->
    <div v-else-if="isLoading" key="reading" class="loading-panel">Reading tag files...</div>
    <div v-else-if="isImporting" key="writing" class="loading-panel">Writing to save file...</div>

    <!-- Preview + import -->
    <div v-else-if="previews.length > 0" key="import" class="view-stack">
      <div class="tag-panel">
        <div class="tag-panel-header">
          <span class="tag-panel-label">
            Tags to Import
            <span v-if="saveVersion !== null" class="save-version">Save v{{ saveVersion }}</span>
          </span>
          <div v-if="hasConflicts || incompatibleCount > 0" class="tag-panel-actions">
            <span v-if="incompatibleCount > 0" class="incompatible-badge">{{ incompatibleCount }} incompatible</span>
            <span v-if="hasConflicts" class="conflict-badge">
              {{ conflictNames.size }} conflict{{ conflictNames.size === 1 ? '' : 's' }}
            </span>
            <button v-if="hasConflicts" class="select-all-btn" @click="toggleAllConflicts">
              {{ allOverwrite ? 'Skip All' : 'Overwrite All' }}
            </button>
          </div>
        </div>

        <ul class="tag-list">
          <li
            v-for="preview in previews"
            :key="preview.path"
            class="tag-row"
            :class="{ 'tag-row--incompatible': !preview.compatible }"
          >
            <div class="tag-info">
              <span class="tag-name">
                {{ preview.tag_name }}
                <!-- Renamed to keep two same-named tags apart; say so plainly. -->
                <span v-if="renames[preview.path]" class="tag-renamed">
                  → installs as “{{ renames[preview.path] }}”
                </span>
              </span>
              <span class="tag-source">{{ preview.path.split(/[\\/]/).pop() }}</span>
              <!-- Only meaningful for tags we can actually read into this save. -->
              <TagDiff v-if="preview.compatible" :path="preview.path" />
            </div>

            <div v-if="!preview.compatible" class="incompatible-tag-badge">
              {{ preview.version === null ? 'Unknown version' : `v${preview.version}` }}
            </div>
            <div v-else-if="conflictNames.has(preview.tag_name)" class="conflict-toggle">
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

      <div v-if="incompatibleCount > 0" class="conflict-hint">
        <strong>{{ incompatibleCount }}</strong> tag{{ incompatibleCount === 1 ? '' : 's' }}
        can't be imported — saved under a different game version<template v-if="saveVersion !== null">
        (this save is <strong>v{{ saveVersion }}</strong>)</template>.
      </div>

      <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>

      <button class="btn btn-primary" :disabled="!hasCompatible" @click="doImport">
        {{ hasCompatible && incompatibleCount > 0 ? `Import ${compatiblePreviews.length} Compatible` : 'Import' }}
      </button>
    </div>

    <!-- Error with no previews -->
    <div v-else-if="errorMsg" key="error" class="error-msg">{{ errorMsg }}</div>

    <div v-else key="empty" class="panel-empty" />
  </Transition>
</template>

<style scoped lang="scss">
.tag-panel-actions {
  display: flex;
  align-items: center;
  gap: 0.5em;
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

.conflict-badge {
  font-size: 0.85em;
  font-weight: 600;
  color: var(--text-warning);
  background: rgba(250, 204, 21, 0.1);
  border: 1px solid rgba(250, 204, 21, 0.25);
  border-radius: 0.5em;
  padding: 0.25em 0.6em;
}

.incompatible-badge {
  font-size: 0.85em;
  font-weight: 600;
  color: var(--text-failure);
  background: rgba(248, 113, 113, 0.1);
  border: 1px solid rgba(248, 113, 113, 0.3);
  border-radius: 0.5em;
  padding: 0.25em 0.6em;
}

.save-version {
  margin-left: 0.5em;
  font-size: 0.75em;
  font-weight: 600;
  letter-spacing: 0.04em;
  color: var(--text-muted);
}

.tag-row {
  justify-content: space-between;
}

.tag-row--incompatible {
  opacity: 0.6;

  .tag-name {
    text-decoration: line-through;
    text-decoration-color: var(--text-failure);
  }
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

.tag-renamed {
  font-size: 0.6em;
  color: var(--text-muted);
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

.incompatible-tag-badge {
  flex-shrink: 0;
  margin-left: 0.5em;
  font-size: 0.8em;
  font-weight: 600;
  color: var(--text-failure);
  background: rgba(248, 113, 113, 0.1);
  border: 1px solid rgba(248, 113, 113, 0.3);
  border-radius: 0.4em;
  padding: 0.2em 0.5em;
  white-space: nowrap;
}

.conflict-hint {
  width: 100%;
  font-size: 0.78em;
  color: var(--text-muted);
  padding: 0 0.25em;
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
    &::before { content: '✓ '; }
  }

  &--skipped {
    color: var(--text-muted);
    &::before { content: '– '; }
  }

  &--incompatible {
    color: var(--text-failure);
    &::before { content: '✕ '; }
  }
}

.panel-empty {
  display: none;
}
</style>

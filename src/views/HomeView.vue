<script setup lang="ts">
// One screen instead of a menu of destinations.
//
// The save is found and read on launch, so the common case needs no clicks —
// the old choose-a-file + Load pair collapses into a status line that only asks
// for input when the file genuinely isn't where it should be. Your tags sit on
// the left; where new tags come from is spelled out on the right, with the
// database browser living inside its own source tile rather than as a separate
// section.
//
// Export and import-to-file still exist, but as a per-tag action and the last
// "or" — sharing to and installing from the database is what people come for.

import { ref, computed, onMounted, nextTick, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import AnimatedCard from '../components/AnimatedCard.vue';
import TagDiff from '../components/TagDiff.vue';
import TagImportPanel from '../components/TagImportPanel.vue';
import SharedTagBrowser, { type SharedTag } from '../components/SharedTagBrowser.vue';
import type { SaveFileState } from '../types';

const EXPECTED_SAVE_FILE_NAME = 'Rivals2_PlayerTagSaveSlot.sav';

const emit = defineEmits<{
  share: [];
  export: [];
  stateChange: [state: SaveFileState];
}>();

const savePath = ref('');
const savePathError = ref(false);
const tagNames = ref<string[]>([]);
const loadingSave = ref(false);
const saveError = ref('');

const sharedTags = ref<SharedTag[]>([]);
const selected = ref<Set<string>>(new Set());
const previewPaths = ref<Record<string, string>>({});
const startggHandles = ref<Record<string, string>>({});

const bracketUrl = ref('');
const bracketBusy = ref(false);
const bracketStatus = ref('');
const bracketStatusKind = ref<'' | 'success' | 'warn' | 'error'>('');
const bracketMisses = ref<{ entrant: string; gamerTag: string; slug: string }[]>([]);

const importPaths = ref<string[]>([]);
const busy = ref(false);
const folderResult = ref('');

const hasSave = computed(() => !!savePath.value && !savePathError.value);
const selectedCount = computed(() => selected.value.size);

watch([savePath, savePathError, tagNames], () => {
  emit('stateChange', {
    savePath: savePath.value,
    savePathError: savePathError.value,
    tagNames: tagNames.value,
    hasLoaded: tagNames.value.length > 0,
  });
});

// ---- save file -------------------------------------------------------------

async function readTags() {
  if (!savePath.value) return;
  loadingSave.value = true;
  saveError.value = '';
  try {
    tagNames.value = await invoke<string[]>('get_tag_names', { savePath: savePath.value });
  } catch (err) {
    saveError.value = String(err);
    tagNames.value = [];
  } finally {
    loadingSave.value = false;
  }
}

onMounted(async () => {
  try {
    const detected = await invoke<string>('get_default_save_path');
    if (detected) {
      savePath.value = detected;
      savePathError.value = false;
      await readTags();
    }
  } catch {
    /* fall through to the choose-a-file prompt */
  }
});

async function chooseSaveFile() {
  const defaultPath = await invoke<string>('get_default_save_path');
  const filePath = await open({
    multiple: false,
    title: 'Choose a Save File',
    filters: [{ name: '.sav file', extensions: ['sav'] }],
    ...(defaultPath ? { defaultPath } : {}),
  });
  if (!filePath) return;
  savePath.value = filePath;
  savePathError.value = (filePath.split(/[\\/]/).pop() ?? '') !== EXPECTED_SAVE_FILE_NAME;
  tagNames.value = [];
  if (!savePathError.value) await readTags();
}

// ---- shared database -------------------------------------------------------

function toggle(file: string) {
  const next = new Set(selected.value);
  if (next.has(file)) next.delete(file);
  else next.add(file);
  selected.value = next;
}

async function preview(file: string) {
  try {
    const [path] = await invoke<string[]>('download_tags', { files: [file], destDir: null });
    if (path) previewPaths.value = { ...previewPaths.value, [file]: path };
  } catch {
    /* a failed peek shouldn't disturb the page */
  }
}

async function findBracket() {
  bracketStatus.value = '';
  bracketStatusKind.value = '';
  bracketMisses.value = [];
  if (!bracketUrl.value.trim()) return;
  bracketBusy.value = true;
  try {
    const res = await invoke<{ event: string; entrants: typeof bracketMisses.value }>(
      'startgg_event',
      { url: bracketUrl.value },
    );
    const slugs = new Set(res.entrants.map(e => e.slug));
    const matches = sharedTags.value.filter(t => t.startggSlug && slugs.has(t.startggSlug));
    selected.value = new Set(matches.map(t => t.file));

    const tagged = new Set(sharedTags.value.filter(t => t.startggSlug).map(t => t.startggSlug));
    const seen = new Set<string>();
    bracketMisses.value = res.entrants.filter(e => {
      if (!e.slug || seen.has(e.slug)) return false;
      seen.add(e.slug);
      return !tagged.has(e.slug);
    });

    const evName = res.event ? ` for “${res.event}”` : '';
    if (!matches.length) {
      bracketStatus.value = `No published tags match the ${slugs.size} entrant(s)${evName}.`;
      bracketStatusKind.value = 'warn';
    } else {
      bracketStatus.value = `Selected ${matches.length} tag(s)${evName}.`;
      bracketStatusKind.value = 'success';
    }
  } catch (err) {
    bracketStatus.value = String(err);
    bracketStatusKind.value = 'error';
  } finally {
    bracketBusy.value = false;
  }
}

// ---- installing ------------------------------------------------------------

async function installSelected() {
  if (!selected.value.size || !hasSave.value) return;
  busy.value = true;
  await nextTick();
  try {
    const files = [...selected.value];
    const paths = await invoke<string[]>('download_tags', { files, destDir: null });
    const byFile = new Map(sharedTags.value.map(t => [t.file, t.startggTag]));
    const handles: Record<string, string> = {};
    paths.forEach((p, i) => {
      const tag = byFile.get(files[i]);
      if (tag) handles[p] = tag;
    });
    startggHandles.value = handles;
    importPaths.value = paths;
  } catch (err) {
    saveError.value = String(err);
  } finally {
    busy.value = false;
  }
}

async function saveToFolder() {
  if (!selected.value.size) return;
  const dir = await open({ directory: true, title: 'Choose Download Folder' });
  if (!dir) return;
  busy.value = true;
  try {
    const written = await invoke<string[]>('download_tags', {
      files: [...selected.value],
      destDir: dir,
    });
    folderResult.value = `Saved ${written.length} tag(s).`;
  } catch (err) {
    saveError.value = String(err);
  } finally {
    busy.value = false;
  }
}

async function chooseFiles() {
  const picked = await open({
    multiple: true,
    title: 'Choose .r2tag Files',
    filters: [{ name: '.r2tag file', extensions: ['r2tag'] }],
  });
  if (!picked) return;
  startggHandles.value = {};
  importPaths.value = Array.isArray(picked) ? picked : [picked];
}

function doneImporting() {
  importPaths.value = [];
  selected.value = new Set();
  readTags();
}
</script>

<template>
  <AnimatedCard wide>
    <div class="home-head">
      <span class="home-title">Rivals II Tag Tool</span>
      <span v-if="hasSave" class="home-save">
        <span class="home-dot" aria-hidden="true"></span>
        <span class="home-save-name">{{ savePath.split(/[\\/]/).pop() }}</span>
        <span class="home-save-count">
          {{ loadingSave ? 'reading…' : `${tagNames.length} tags` }}
        </span>
        <button class="linkish" @click="chooseSaveFile">Change</button>
      </span>
      <button v-else class="btn btn-primary" @click="chooseSaveFile">Choose a Save File</button>
    </div>

    <p v-if="saveError" class="error-msg">{{ saveError }}</p>

    <Transition name="content-swap" mode="out-in">
      <!-- Installing takes over the card: it's a decision point. -->
      <div v-if="importPaths.length" key="import" class="view-stack">
        <TagImportPanel
          :save-path="savePath"
          :existing-tag-names="tagNames"
          :paths="importPaths"
          :startgg-handles="startggHandles"
          @restart="doneImporting"
        />
        <button class="linkish" @click="doneImporting">← Back</button>
      </div>

      <div v-else key="home" class="view-stack">
        <div class="home-cols">
          <!-- Your tags -->
          <div class="tag-panel">
            <div class="tag-panel-header">
              <span class="tag-panel-label">Your Tags</span>
            </div>
            <p v-if="!hasSave" class="home-empty">Choose a save file to see your tags.</p>
            <p v-else-if="!tagNames.length && !loadingSave" class="home-empty">
              No custom tags yet.
            </p>
            <ul v-else class="tag-list">
              <li v-for="name in tagNames" :key="name" class="home-tag">
                <div class="home-tag-head">
                  <span class="tag-name">{{ name }}</span>
                  <span class="home-tag-actions">
                    <button class="linkish" @click="emit('share')">Share</button>
                    <button class="linkish" @click="emit('export')">Export</button>
                  </span>
                </div>
                <TagDiff :save-path="savePath" :tag-name="name" />
              </li>
            </ul>
          </div>

          <!-- Where new tags come from -->
          <div class="home-sources">
            <div class="source source--primary">
              <div class="source-title">Everyone in a bracket</div>
              <div class="source-sub">Paste a start.gg link.</div>
              <div class="source-row">
                <input
                  v-model="bracketUrl"
                  class="home-input"
                  type="text"
                  placeholder="start.gg/tournament/…"
                  @keydown.enter="findBracket"
                />
                <button class="btn" :disabled="bracketBusy || !bracketUrl.trim()" @click="findBracket">
                  {{ bracketBusy ? '…' : 'Find' }}
                </button>
              </div>
              <p v-if="bracketStatus" class="source-status" :class="`source-status--${bracketStatusKind}`">
                {{ bracketStatus }}
              </p>
              <details v-if="bracketMisses.length" class="home-misses">
                <summary>{{ bracketMisses.length }} without a tag</summary>
                <p>{{ bracketMisses.map(e => e.gamerTag || e.entrant || e.slug).join(', ') }}</p>
              </details>
            </div>

            <div class="home-or">or</div>

            <!-- The database lives in its own source tile, not a separate section. -->
            <div class="source source--browser">
              <SharedTagBrowser
                :selected="selected"
                :preview-paths="previewPaths"
                @toggle="toggle"
                @loaded="sharedTags = $event"
                @preview="preview"
              />
            </div>

            <div class="home-or">or</div>

            <div class="source">
              <div class="source-title">From files on this PC</div>
              <div class="source-sub">.r2tag files someone sent you.</div>
              <button class="btn" @click="chooseFiles">Choose Files</button>
            </div>
          </div>
        </div>

        <div class="home-actions">
          <button
            class="btn btn-primary"
            :disabled="!selectedCount || !hasSave || busy"
            @click="installSelected"
          >
            {{ busy ? 'Downloading…' : `Install ${selectedCount || ''}`.trim() }}
          </button>
          <button class="btn" :disabled="!selectedCount || busy" @click="saveToFolder">
            Save to Folder
          </button>
          <span v-if="selectedCount && !hasSave" class="home-hint">Choose a save file first.</span>
          <span v-else-if="folderResult" class="home-hint">{{ folderResult }}</span>
        </div>
      </div>
    </Transition>
  </AnimatedCard>
</template>

<style scoped lang="scss">
.home-head {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.75rem;
  min-height: 2rem;
}

.home-title {
  font-size: 1.05rem;
  letter-spacing: 0.02em;
}

.home-save {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.75rem;
}

.home-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--text-success);
}

.home-save-name {
  font-family: 'Ubuntu Sans Mono Variable', monospace;
  color: var(--text-muted);
}

.home-save-count {
  color: var(--text-muted);
}

.home-cols {
  width: 100%;
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.75rem;
  align-items: start;
}

.home-sources {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.home-empty {
  margin: 0;
  padding: 0.6em 0.25em;
  font-size: 1em;
  color: var(--text-muted);
}

.home-tag {
  padding: 0.4em 0.25em;
  border-bottom: 1px solid var(--line-divider);
}

.home-tag-head {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.home-tag-actions {
  margin-left: auto;
  display: flex;
  gap: 0.5rem;
}

.source {
  padding: 0.6rem 0.75rem;
  background: var(--surface-inset);
  border: 1px solid var(--line-subtle);
  border-radius: var(--radius-panel);
  font-size: 0.75rem;

  &--primary {
    border-color: var(--accent);
  }

  &--browser {
    padding: 0.5rem 0.6rem;
  }
}

.source-title {
  font-size: 1em;
}

.source-sub {
  color: var(--text-muted);
  margin-bottom: 0.4em;
}

.source-row {
  display: flex;
  gap: 0.35rem;
  align-items: center;
}

/* Inputs are styled per-view in this app; match the inset panels rather than
   inheriting the browser default (which rendered as a white box). */
.home-input {
  flex: 1;
  min-width: 0;
  font-family: inherit;
  font-size: 1em;
  color: var(--text-primary);
  background: rgba(0, 0, 0, 0.25);
  border: 1px solid var(--line);
  border-radius: var(--radius-button);
  padding: 0.4em 0.6em;

  &::placeholder {
    color: rgba(255, 255, 255, 0.35);
  }

  &:focus-visible {
    outline: 2px solid rgba(99, 102, 241, 0.6);
    outline-offset: 1px;
  }
}

.source-status {
  margin: 0.4em 0 0;

  &--success { color: var(--text-success); }
  &--warn { color: var(--text-warning); }
  &--error { color: var(--text-failure); }
}

.home-misses {
  margin-top: 0.3em;
  color: var(--text-muted);

  summary { cursor: pointer; }
  p { margin: 0.25em 0 0; }
}

.home-or {
  text-align: center;
  font-size: 0.7rem;
  color: var(--text-muted);
}

.home-actions {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.home-hint {
  font-size: 0.75rem;
  color: var(--text-muted);
}

.linkish {
  padding: 0;
  border: none;
  background: none;
  font: inherit;
  font-size: 0.75rem;
  color: var(--text-muted);
  cursor: pointer;
  text-decoration: underline;
  text-underline-offset: 2px;

  &:hover { color: var(--text-primary); }
}
</style>

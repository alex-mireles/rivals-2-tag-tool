<script setup lang="ts">
// One screen instead of a menu of destinations.
//
// The save is found and read on launch, so the common case needs no clicks at
// all — the old "choose a file, then press Load" pair collapses into a status
// line that only asks for input when the file genuinely isn't where it should
// be. Your tags sit on the left; where new tags come from is spelled out on the
// right; the shared database is always on screen underneath rather than being
// somewhere you navigate to.
//
// Export-to-file and import-from-file still exist, but as secondary actions
// rather than top-level choices: sharing to and installing from the database is
// what people are here for.

import { ref, computed, onMounted, nextTick, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { SaveFileState } from '../types';
import TagDiff from '../components/TagDiff.vue';
import TagImportPanel from '../components/TagImportPanel.vue';
import SharedTagBrowser, { type SharedTag } from '../components/SharedTagBrowser.vue';

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

// Share/export are separate screens, so they need the save state too.
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

// The save lives at a known path; look there first and just read it.
onMounted(async () => {
  try {
    const detected = await invoke<string>('get_default_save_path');
    if (detected) {
      savePath.value = detected;
      savePathError.value = false;
      await readTags();
    }
  } catch {
    /* fall through to the "choose a file" prompt */
  }
});

async function chooseSaveFile() {
  const defaultPath = await invoke<string>('get_default_save_path');
  const filePath = await open({
    multiple: false,
    title: 'Choose a save file',
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

function onLoaded(tags: SharedTag[]) {
  sharedTags.value = tags;
}

/** Fetch one tag so its diff can be shown without installing it. */
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
      bracketStatus.value = `No published tags match the ${slugs.size} linked entrant(s)${evName}.`;
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

/** Download the selected tags as files instead of installing them. */
async function saveToFolder() {
  if (!selected.value.size) return;
  const dir = await open({ directory: true, title: 'Choose a folder' });
  if (!dir) return;
  busy.value = true;
  try {
    const written = await invoke<string[]>('download_tags', {
      files: [...selected.value],
      destDir: dir,
    });
    folderResult.value = `Saved ${written.length} tag(s) to ${dir}`;
  } catch (err) {
    saveError.value = String(err);
  } finally {
    busy.value = false;
  }
}

async function chooseFiles() {
  const picked = await open({
    multiple: true,
    title: 'Choose .r2tag files',
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
  <div class="home">
    <!-- Save file: a quiet status line once it's found, a prompt when it isn't. -->
    <div class="save-bar" :class="{ 'save-bar--needs-input': !hasSave }">
      <template v-if="hasSave">
        <span class="save-dot" aria-hidden="true"></span>
        <span class="save-name">{{ savePath.split(/[\\/]/).pop() }}</span>
        <span class="save-meta">
          {{ loadingSave ? 'reading…' : `${tagNames.length} tag${tagNames.length === 1 ? '' : 's'}` }}
        </span>
        <button class="linkish" @click="chooseSaveFile">Change</button>
      </template>
      <template v-else>
        <span class="save-name">{{ EXPECTED_SAVE_FILE_NAME }} not found</span>
        <button class="btn btn-primary btn-sm" @click="chooseSaveFile">Choose a save file</button>
      </template>
    </div>
    <p v-if="saveError" class="error-line">{{ saveError }}</p>

    <!-- Installing takes over: it's a decision point, not a background task. -->
    <div v-if="importPaths.length" class="panel">
      <TagImportPanel
        :save-path="savePath"
        :existing-tag-names="tagNames"
        :paths="importPaths"
        :startgg-handles="startggHandles"
        @restart="doneImporting"
      />
      <button class="linkish back" @click="doneImporting">← Back</button>
    </div>

    <template v-else>
      <div class="columns">
        <!-- Your tags -->
        <section class="panel">
          <h2 class="panel-title">Your tags</h2>
          <p class="panel-sub">In your save file</p>

          <p v-if="!hasSave" class="empty">Choose a save file to see your tags.</p>
          <p v-else-if="!tagNames.length && !loadingSave" class="empty">
            No custom tags in this save yet.
          </p>
          <ul v-else class="rows">
            <li v-for="name in tagNames" :key="name" class="row">
              <div class="row-main">
                <span class="row-name">{{ name }}</span>
                <span class="row-actions">
                  <button class="linkish" @click="emit('share')">Share</button>
                  <button class="linkish" @click="emit('export')">Export</button>
                </span>
              </div>
              <TagDiff :save-path="savePath" :tag-name="name" />
            </li>
          </ul>
        </section>

        <!-- Where new tags come from -->
        <section class="panel">
          <h2 class="panel-title">Get tags</h2>
          <p class="panel-sub">Pick a source</p>

          <div class="source source--primary">
            <div class="source-title">Everyone in a bracket</div>
            <div class="source-sub">Paste a start.gg link and we'll pick out who has a tag.</div>
            <div class="source-row">
              <input
                v-model="bracketUrl"
                type="text"
                placeholder="start.gg/tournament/…/event/…"
                @keydown.enter="findBracket"
              />
              <button class="btn btn-sm" :disabled="bracketBusy || !bracketUrl.trim()" @click="findBracket">
                {{ bracketBusy ? 'Looking…' : 'Find' }}
              </button>
            </div>
            <p v-if="bracketStatus" class="source-status" :class="`source-status--${bracketStatusKind}`">
              {{ bracketStatus }}
            </p>
            <details v-if="bracketMisses.length" class="misses">
              <summary>
                {{ bracketMisses.length }} entrant{{ bracketMisses.length === 1 ? '' : 's' }}
                without a published tag
              </summary>
              <p>{{ bracketMisses.map(e => e.gamerTag || e.entrant || e.slug).join(', ') }}</p>
            </details>
          </div>

          <div class="or">or</div>

          <div class="source">
            <div class="source-title">Pick from the database</div>
            <div class="source-sub">Browse everything published, below.</div>
          </div>

          <div class="or">or</div>

          <div class="source">
            <div class="source-title">From files on this PC</div>
            <div class="source-sub">.r2tag files someone sent you.</div>
            <button class="btn btn-sm" @click="chooseFiles">Choose files</button>
          </div>
        </section>
      </div>

      <!-- The database itself, always on screen. -->
      <section class="panel">
        <SharedTagBrowser
          :selected="selected"
          :preview-paths="previewPaths"
          @toggle="toggle"
          @loaded="onLoaded"
          @preview="preview"
        />
        <div class="install-bar">
          <button
            class="btn btn-primary"
            :disabled="!selectedCount || !hasSave || busy"
            @click="installSelected"
          >
            {{ busy ? 'Downloading…' : `Install ${selectedCount || ''} to save`.trim() }}
          </button>
          <button class="btn btn-sm" :disabled="!selectedCount || busy" @click="saveToFolder">
            Save to folder
          </button>
          <span v-if="selectedCount && !hasSave" class="hint">Choose a save file first.</span>
          <span v-else-if="folderResult" class="hint">{{ folderResult }}</span>
        </div>
      </section>
    </template>
  </div>
</template>

<style scoped lang="scss">
.home {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  padding: 0.5rem 0 1rem;
}

.save-bar {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.45rem 0.75rem;
  border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
  border-radius: 8px;
  font-size: 0.85rem;

  &--needs-input {
    border-color: var(--text-muted);
  }
}

.save-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #7fd39a;
}

.save-name {
  font-family: 'Ubuntu Sans Mono Variable', monospace;
}

.save-meta {
  color: var(--text-muted);
}

.save-bar .linkish,
.install-bar .hint {
  margin-left: auto;
}

.error-line {
  margin: 0;
  font-size: 0.85rem;
  color: #e06c75;
}

.columns {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.75rem;
  align-items: start;
}

.panel {
  border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
  border-radius: 10px;
  padding: 0.75rem 0.9rem;
}

.panel-title {
  margin: 0;
  font-size: 0.95rem;
  font-weight: 500;
}

.panel-sub {
  margin: 0 0 0.6rem;
  font-size: 0.8rem;
  color: var(--text-muted);
}

.empty {
  margin: 0;
  font-size: 0.85rem;
  color: var(--text-muted);
}

.rows {
  list-style: none;
  margin: 0;
  padding: 0;
  max-height: 18rem;
  overflow-y: auto;
}

.row {
  padding: 0.35rem 0;
  border-top: 1px solid var(--border, rgba(255, 255, 255, 0.08));

  &:first-child {
    border-top: none;
  }
}

.row-main {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
}

.row-name {
  font-weight: 500;
}

.row-actions {
  margin-left: auto;
  display: flex;
  gap: 0.5rem;
}

/* The bracket path is what a TO reaches for, so it gets the emphasis; the
   others stay available but visibly secondary. */
.source {
  border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
  border-radius: 8px;
  padding: 0.55rem 0.7rem;

  &--primary {
    border-color: var(--text-accent, #6ea8fe);
  }
}

.source-title {
  font-size: 0.88rem;
}

.source-sub {
  font-size: 0.78rem;
  color: var(--text-muted);
  margin-bottom: 0.35rem;
}

.source-row {
  display: flex;
  gap: 0.4rem;

  input {
    flex: 1;
    min-width: 0;
  }
}

.source-status {
  margin: 0.35rem 0 0;
  font-size: 0.78rem;

  &--success { color: #7fd39a; }
  &--warn { color: #e5c07b; }
  &--error { color: #e06c75; }
}

.misses {
  margin-top: 0.3rem;
  font-size: 0.78rem;
  color: var(--text-muted);

  summary { cursor: pointer; }
  p { margin: 0.25rem 0 0; }
}

.or {
  text-align: center;
  font-size: 0.75rem;
  color: var(--text-muted);
  margin: 0.3rem 0;
}

.install-bar {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  margin-top: 0.6rem;
}

.hint {
  font-size: 0.8rem;
  color: var(--text-muted);
}

.back {
  margin-top: 0.5rem;
}

.linkish {
  padding: 0;
  border: none;
  background: none;
  font: inherit;
  font-size: 0.8rem;
  color: var(--text-muted);
  cursor: pointer;
  text-decoration: underline;
  text-underline-offset: 2px;

  &:hover { color: var(--text-primary, inherit); }
}

@media (max-width: 720px) {
  .columns { grid-template-columns: 1fr; }
}
</style>

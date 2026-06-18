<script setup lang="ts">
import { ref, computed, onMounted, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import AnimatedCard from '../components/AnimatedCard.vue';
import SavePathBar from '../components/SavePathBar.vue';
import ViewHeader from '../components/ViewHeader.vue';
import TagImportPanel from '../components/TagImportPanel.vue';

interface SharedTag {
  name: string;
  author: string;
  file: string;
  startggSlug: string;
  startggTag: string;
}

interface EventEntrant {
  entrant: string;
  gamerTag: string;
  slug: string;
}

interface EventResult {
  event: string;
  entrants: EventEntrant[];
}

defineProps<{
  savePath: string;
  tagNames: string[];
}>();

const emit = defineEmits<{
  'go-back': [];
}>();

const sharedTags = ref<SharedTag[]>([]);
const selected = ref<Set<string>>(new Set());
const loadingList = ref(false);
const listError = ref('');

const bracketUrl = ref('');
const bracketBusy = ref(false);
const bracketStatus = ref('');
const bracketStatusKind = ref<'' | 'success' | 'warn' | 'error'>('');

const downloading = ref(false);
const paths = ref<string[]>([]);
const downloadResult = ref<{ count: number; dir: string } | null>(null);

const allSelected = computed(
  () => sharedTags.value.length > 0 && selected.value.size === sharedTags.value.length
);

onMounted(loadList);

async function loadList() {
  loadingList.value = true;
  listError.value = '';
  try {
    sharedTags.value = await invoke<SharedTag[]>('fetch_shared_tags');
  } catch (err) {
    listError.value = String(err);
  } finally {
    loadingList.value = false;
  }
}

function toggle(file: string) {
  if (selected.value.has(file)) selected.value.delete(file);
  else selected.value.add(file);
  selected.value = new Set(selected.value);
}

function toggleAll() {
  selected.value = allSelected.value ? new Set() : new Set(sharedTags.value.map(t => t.file));
}

async function findBracket() {
  bracketStatus.value = '';
  bracketStatusKind.value = '';
  if (!bracketUrl.value.trim()) return;
  bracketBusy.value = true;
  try {
    const res = await invoke<EventResult>('startgg_event', { url: bracketUrl.value });
    const slugs = new Set(res.entrants.map(e => e.slug));
    const matches = sharedTags.value.filter(t => t.startggSlug && slugs.has(t.startggSlug));
    selected.value = new Set(matches.map(t => t.file));
    const evName = res.event ? ` for “${res.event}”` : '';
    if (matches.length === 0) {
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

// Download to a temp dir and hand the files to the import preview.
async function importToSave() {
  if (selected.value.size === 0) return;
  downloading.value = true;
  listError.value = '';
  await nextTick();
  try {
    paths.value = await invoke<string[]>('download_tags', {
      files: [...selected.value],
      destDir: null,
    });
  } catch (err) {
    listError.value = String(err);
  } finally {
    downloading.value = false;
  }
}

// Download the .r2tag files to a folder the user picks.
async function downloadToFolder() {
  if (selected.value.size === 0) return;
  const dir = await open({ directory: true, title: 'Choose Download Folder' });
  if (!dir) return;
  downloading.value = true;
  listError.value = '';
  await nextTick();
  try {
    const written = await invoke<string[]>('download_tags', {
      files: [...selected.value],
      destDir: dir,
    });
    downloadResult.value = { count: written.length, dir };
  } catch (err) {
    listError.value = String(err);
  } finally {
    downloading.value = false;
  }
}

function backToBrowse() {
  paths.value = [];
  downloadResult.value = null;
}

function tagSub(t: SharedTag): string {
  const parts = [];
  if (t.startggTag) parts.push(`@${t.startggTag}`);
  if (t.author) parts.push(`by ${t.author}`);
  return parts.join(' · ');
}
</script>

<template>
  <AnimatedCard>
    <ViewHeader title="Download from Site" @go-back="emit('go-back')" />

    <SavePathBar :label="savePath" />

    <Transition name="content-swap" mode="out-in">
      <div v-if="downloading" key="dl" class="loading-panel">Downloading tags...</div>

      <!-- Files downloaded to a folder -->
      <div v-else-if="downloadResult" key="dlresult" class="view-stack">
        <div class="result-panel result-panel--success">
          <span class="result-panel-msg">
            Downloaded {{ downloadResult.count }} tag{{ downloadResult.count === 1 ? '' : 's' }} to
            <span class="result-panel-path">{{ downloadResult.dir }}</span>
          </span>
        </div>
        <button class="btn btn-primary" @click="backToBrowse">Back to Browse</button>
      </div>

      <!-- Downloaded -> import -->
      <div v-else-if="paths.length > 0" key="import" class="view-stack">
        <button class="back-to-browse" @click="backToBrowse">← Back to browse</button>
        <TagImportPanel
          :save-path="savePath"
          :existing-tag-names="tagNames"
          :paths="paths"
          @restart="backToBrowse"
        />
      </div>

      <!-- Browse + bracket selection -->
      <div v-else key="browse" class="view-stack">
        <div class="bracket-field">
          <label class="bracket-label">Download a whole bracket</label>
          <p class="hint">Paste a start.gg event URL to auto-select every published tag belonging to an entrant.</p>
          <div class="bracket-row">
            <input
              v-model="bracketUrl"
              type="text"
              class="bracket-input"
              autocomplete="off"
              spellcheck="false"
              placeholder="https://www.start.gg/tournament/…/event/…"
              @keydown.enter.prevent="findBracket"
            />
            <button class="find-btn" :disabled="bracketBusy || !bracketUrl.trim()" @click="findBracket">
              {{ bracketBusy ? '…' : 'Find tags' }}
            </button>
          </div>
          <p v-if="bracketStatus" class="bracket-status" :class="`bracket-status--${bracketStatusKind}`">
            {{ bracketStatus }}
          </p>
        </div>

        <div class="tag-panel">
          <div class="tag-panel-header">
            <span class="tag-panel-label">Available Tags</span>
            <button v-if="sharedTags.length > 0" class="select-all-btn" @click="toggleAll">
              {{ allSelected ? 'Clear' : 'Select all' }}
            </button>
          </div>

          <div v-if="loadingList" class="list-note">Loading shared tags…</div>
          <div v-else-if="listError" class="error-msg">{{ listError }}</div>
          <div v-else-if="sharedTags.length === 0" class="list-note">No shared tags published yet.</div>
          <ul v-else class="tag-list">
            <li
              v-for="t in sharedTags"
              :key="t.file"
              class="tag-row tag-row--selectable"
              @click="toggle(t.file)"
            >
              <div class="tag-checkbox" :class="{ 'tag-checkbox--checked': selected.has(t.file) }">
                <svg v-if="selected.has(t.file)" xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24">
                  <path fill="currentColor" d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
                </svg>
              </div>
              <div class="tag-info">
                <span class="tag-name">{{ t.name }}</span>
                <span v-if="tagSub(t)" class="tag-sub">{{ tagSub(t) }}</span>
              </div>
            </li>
          </ul>
        </div>

        <p v-if="selected.size === 0" class="action-hint">Select tags to import or download.</p>
        <div v-else class="action-grid">
          <button class="btn btn-primary" @click="importToSave">
            Import {{ selected.size }} to Save
          </button>
          <button class="btn btn-primary btn-primary-muted" @click="downloadToFolder">
            Download {{ selected.size }} File{{ selected.size === 1 ? '' : 's' }}
          </button>
        </div>
      </div>
    </Transition>
  </AnimatedCard>
</template>

<style scoped lang="scss">
.bracket-field {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 0.3em;
}

.bracket-label {
  font-size: 0.9em;
  color: var(--text-muted);
}

.hint {
  font-size: 0.75em;
  color: var(--text-muted);
  line-height: 1.35;
}

.bracket-row {
  display: flex;
  gap: 0.5em;
  margin-top: 0.15em;
}

.bracket-input {
  flex: 1;
  min-width: 0;
  font-family: inherit;
  font-size: 0.85rem;
  color: var(--text-primary);
  background: var(--surface-inset);
  border: 1px solid var(--line);
  border-radius: var(--radius-button);
  padding: 0.5em 0.7em;
  transition: border-color 0.15s;

  &:focus {
    outline: none;
    border-color: var(--accent);
  }

  &::placeholder {
    color: var(--text-muted);
  }
}

.find-btn {
  flex-shrink: 0;
  padding: 0.5em 0.9em;
  border-radius: var(--radius-button);
  border: 1px solid var(--line);
  background: var(--surface-hover);
  color: var(--text-primary);
  font-size: 0.85em;
  font-weight: 600;
  cursor: pointer;

  &:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
}

.bracket-status {
  font-size: 0.78em;
  color: var(--text-muted);

  &--success { color: var(--text-success); }
  &--warn { color: var(--text-warning); }
  &--error { color: var(--text-failure); }
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

.tag-row--selectable {
  cursor: pointer;
  gap: 0.75em;

  &:hover {
    background: var(--surface-hover);
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

.tag-info {
  display: flex;
  flex-direction: column;
  gap: 0.1em;
  min-width: 0;
}

.tag-name {
  font-size: 1.2em;
}

.tag-sub {
  font-size: 0.8em;
  color: var(--text-muted);
}

.list-note {
  padding: 1em;
  text-align: center;
  font-size: 0.85em;
  color: var(--text-muted);
}

.back-to-browse {
  align-self: flex-start;
  background: none;
  border: none;
  color: var(--text-muted);
  font-size: 0.82em;
  cursor: pointer;
  padding: 0;

  &:hover {
    color: var(--text-primary);
  }
}

.action-grid {
  width: 100%;
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.625rem;

  .btn {
    width: 100%;
    white-space: nowrap;
  }
}

.action-hint {
  width: 100%;
  text-align: center;
  font-size: 0.82em;
  color: var(--text-muted);
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
    font-size: 0.9em;
    color: var(--text-success);
  }

  &-path {
    font-family: 'Ubuntu Sans Mono Variable', monospace;
    word-break: break-all;
  }
}
</style>

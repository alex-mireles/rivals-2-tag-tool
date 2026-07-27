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

import { ref, reactive, computed, onMounted, nextTick, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import AnimatedCard from '../components/AnimatedCard.vue';
import StartggLink from '../components/StartggLink.vue';
import TagDiff from '../components/TagDiff.vue';
import SharedTagBrowser, { type SharedTag } from '../components/SharedTagBrowser.vue';
import type { SaveFileState, StartggLinkValue } from '../types';

const EXPECTED_SAVE_FILE_NAME = 'Rivals2_PlayerTagSaveSlot.sav';

interface TagPreviewInfo {
  path: string;
  tag_name: string;
  version: number | null;
  compatible: boolean;
}

interface ImportOutcome {
  imported: string[];
  skipped: string[];
  incompatible: string[];
}

const emit = defineEmits<{ stateChange: [state: SaveFileState] }>();

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

// Submitting happens on this page: pick tags on the left, link each to a
// start.gg account, send. No separate screen to bounce through.
const included = ref<Set<string>>(new Set());
const links = reactive<Record<string, StartggLinkValue | null>>({});
const sharing = ref(false);
const shareResult = ref<{ pr: string; number: number; count: number } | null>(null);

const busy = ref(false);
const folderResult = ref('');
const installResult = ref('');
const fileInstallResult = ref('');
// Installing overwrites same-named tags already in the save by default; the
// checkbox next to Install lets you opt out.
const overwriteExisting = ref(true);

const hasSave = computed(() => !!savePath.value && !savePathError.value);
const selectedCount = computed(() => selected.value.size);

/** Abbreviate a long absolute path to its first three segments (drive, "Users",
 *  username), an ellipsis, then the file name — keeps the user-identifying
 *  part visible without spelling out every folder in between. Paths that are
 *  already short (4 segments or fewer) are shown in full. */
function abbreviatePath(path: string): string {
  const sep = path.includes('\\') ? '\\' : '/';
  const parts = path.split(/[\\/]/).filter(Boolean);
  if (parts.length <= 4) return path;
  const head = parts.slice(0, 3).join(sep);
  const fileName = parts[parts.length - 1];
  return `${head}${sep}...${sep}${fileName}`;
}

const abbreviatedSavePath = computed(() => abbreviatePath(savePath.value));

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

// ---- submitting your tags --------------------------------------------------

const includedNames = computed(() => tagNames.value.filter(n => included.value.has(n)));
const allLinked = computed(
  () => includedNames.value.length > 0 && includedNames.value.every(n => !!links[n]?.slug),
);
const missingLinks = computed(() => includedNames.value.filter(n => !links[n]?.slug).length);

function toggleMine(name: string) {
  const next = new Set(included.value);
  if (next.has(name)) next.delete(name);
  else {
    next.add(name);
    if (!(name in links)) links[name] = null;
  }
  included.value = next;
}

async function submitTags() {
  saveError.value = '';
  sharing.value = true;
  await nextTick();
  try {
    const items = includedNames.value.map(name => ({
      tagName: name,
      startgg: links[name] as StartggLinkValue,
    }));
    const res = await invoke<{ pr: string; number: number }>('share_tags_to_site', {
      savePath: savePath.value,
      items,
    });
    shareResult.value = { ...res, count: items.length };
    included.value = new Set();
  } catch (err) {
    saveError.value = String(err);
  } finally {
    sharing.value = false;
  }
}

async function exportSelected() {
  const dir = await open({ directory: true, title: 'Choose Export Folder' });
  if (!dir) return;
  sharing.value = true;
  try {
    const written = await invoke<string[]>('export_tags', {
      savePath: savePath.value,
      tagNames: includedNames.value,
      outputDir: dir,
      startgg: null,
    });
    shareResult.value = null;
    folderResult.value = `Exported ${written.length} tag(s).`;
    included.value = new Set();
  } catch (err) {
    saveError.value = String(err);
  } finally {
    sharing.value = false;
  }
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

/**
 * Preview + import a batch of .r2tag files into the loaded save, in place —
 * no full-screen takeover. Only version-compatible tags are sent to
 * import_tags; the rest are reported as incompatible.
 *
 * `handles` is an optional file-path -> start.gg handle map. Two different
 * people can end up with the same in-game tag name by coincidence; a save
 * holds one tag per name, so without help the second import would collide
 * with the first. Where we know a start.gg handle for a colliding tag, it's
 * installed under that handle instead so both land — the same collision
 * safety TagImportPanel used to provide.
 */
async function installFiles(paths: string[], handles: Record<string, string>): Promise<string> {
  const previewRes = await invoke<{ save_version: number | null; previews: TagPreviewInfo[] }>(
    'get_tag_previews',
    { r2tagPaths: paths, savePath: savePath.value },
  );
  const compatible = previewRes.previews.filter(p => p.compatible);
  const incompatibleFromPreview = previewRes.previews.length - compatible.length;

  const byName = new Map<string, TagPreviewInfo[]>();
  for (const p of compatible) {
    if (!byName.has(p.tag_name)) byName.set(p.tag_name, []);
    byName.get(p.tag_name)!.push(p);
  }
  const renames: Record<string, string> = {};
  for (const group of byName.values()) {
    if (group.length < 2) continue;
    for (const p of group) {
      const handle = handles[p.path];
      if (handle && handle !== p.tag_name) renames[p.path] = handle;
    }
  }

  const instructions = compatible.map(p => ({
    path: p.path,
    tag_name: p.tag_name,
    overwrite: overwriteExisting.value,
    rename: renames[p.path] ?? null,
  }));

  const result = await invoke<ImportOutcome>('import_tags', {
    savePath: savePath.value,
    instructions,
  });

  const totalIncompatible = result.incompatible.length + incompatibleFromPreview;
  const parts: string[] = [];
  if (result.imported.length) parts.push(`Installed ${result.imported.length}`);
  if (result.skipped.length) parts.push(`skipped ${result.skipped.length}`);
  if (totalIncompatible) parts.push(`${totalIncompatible} incompatible`);
  return parts.length ? parts.join(' · ') : 'Nothing to install.';
}

async function installSelected() {
  if (!selected.value.size || !hasSave.value) return;
  busy.value = true;
  folderResult.value = '';
  installResult.value = '';
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
    installResult.value = await installFiles(paths, handles);
    selected.value = new Set();
    await readTags();
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
  installResult.value = '';
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
  const paths = Array.isArray(picked) ? picked : [picked];
  if (!hasSave.value) {
    fileInstallResult.value = 'Choose a save file first.';
    return;
  }
  busy.value = true;
  fileInstallResult.value = '';
  try {
    fileInstallResult.value = await installFiles(paths, {});
    await readTags();
  } catch (err) {
    saveError.value = String(err);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <AnimatedCard wide static>
    <div class="home-head">
      <span class="home-title">Rivals II Tag Tool</span>
      <span v-if="hasSave" class="home-save">
        <span class="home-dot" aria-hidden="true"></span>
        <span class="home-save-name" :title="savePath">{{ abbreviatedSavePath }}</span>
        <span class="home-save-count">
          {{ loadingSave ? 'reading…' : `${tagNames.length} tags` }}
        </span>
        <button class="linkish" @click="chooseSaveFile">Change</button>
      </span>
      <button v-else class="btn btn-primary" @click="chooseSaveFile">Choose a Save File</button>
    </div>

    <p v-if="saveError" class="error-msg">{{ saveError }}</p>

    <div class="view-stack">
      <div class="home-cols">
        <!-- Submit your tags -->
        <div class="home-mine">
          <div class="home-sources-head">
            <span class="tag-panel-label">Submit your tags</span>
            <span v-if="includedNames.length" class="home-sel">{{ includedNames.length }}</span>
          </div>

          <div class="tag-panel">
            <div v-if="shareResult" class="home-shared">
              <p>Submitted {{ shareResult.count }} tag(s).</p>
              <button class="btn" @click="openUrl(shareResult.pr)">
                View pull request #{{ shareResult.number }}
              </button>
              <button class="linkish" @click="shareResult = null">Submit more</button>
            </div>

            <template v-else>
              <p v-if="!hasSave" class="home-empty">Choose a save file to see your tags.</p>
              <p v-else-if="!tagNames.length && !loadingSave" class="home-empty">
                No custom tags yet.
              </p>
              <ul v-else class="tag-list" :class="{ 'tag-list--open': includedNames.length > 0 }">
                <li v-for="name in tagNames" :key="name" class="home-tag">
                  <div class="home-tag-head" @click="toggleMine(name)">
                    <span
                      class="tag-checkbox"
                      :class="{ 'tag-checkbox--checked': included.has(name) }"
                      aria-hidden="true"
                    >
                      <svg v-if="included.has(name)" xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24">
                        <path fill="currentColor" d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
                      </svg>
                    </span>
                    <span class="tag-name">{{ name }}</span>
                  </div>
                  <div class="home-tag-body">
                    <TagDiff :save-path="savePath" :tag-name="name" />
                    <StartggLink
                      v-if="included.has(name)"
                      :model-value="links[name] ?? null"
                      @update:model-value="(v: StartggLinkValue | null) => (links[name] = v)"
                    />
                  </div>
                </li>
              </ul>

              <div v-if="includedNames.length" class="home-submit">
                <button
                  class="btn btn-primary"
                  :disabled="!allLinked || sharing"
                  @click="submitTags"
                >
                  {{ sharing ? 'Uploading…' : `Submit ${includedNames.length}` }}
                </button>
                <button class="btn" :disabled="sharing" @click="exportSelected">Export</button>
                <span v-if="missingLinks" class="home-hint">
                  {{ missingLinks }} still need a start.gg account
                </span>
              </div>
            </template>
          </div>
        </div>

          <!-- Where new tags come from -->
          <div class="home-sources">
            <div class="home-sources-head">
              <span class="tag-panel-label">Install tags to your setup</span>
            </div>

            <div class="source">
              <div class="source-title">Everyone in a bracket</div>
              <div class="source-row">
                <input
                  v-model="bracketUrl"
                  class="home-input"
                  type="text"
                  placeholder="Paste a start.gg link…"
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
              <button class="btn" :disabled="busy" @click="chooseFiles">
                {{ busy ? 'Installing…' : 'Choose Files' }}
              </button>
              <p v-if="fileInstallResult" class="source-status">{{ fileInstallResult }}</p>
            </div>
          </div>
        </div>

        <div class="home-actions">
          <button
            class="btn btn-primary"
            :disabled="!selectedCount || !hasSave || busy"
            @click="installSelected"
          >
            {{ busy ? 'Installing…' : `Install ${selectedCount || ''}`.trim() }}
          </button>
          <button class="btn" :disabled="!selectedCount || busy" @click="saveToFolder">
            Save to Folder
          </button>
          <div class="home-overwrite" @click="overwriteExisting = !overwriteExisting">
            <span
              class="tag-checkbox"
              :class="{ 'tag-checkbox--checked': overwriteExisting }"
              aria-hidden="true"
            >
              <svg v-if="overwriteExisting" xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24">
                <path fill="currentColor" d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
              </svg>
            </span>
            <span>Overwrite existing tags</span>
          </div>
          <span v-if="selectedCount && !hasSave" class="home-hint">Choose a save file first.</span>
          <span v-else-if="installResult" class="home-hint">{{ installResult }}</span>
          <span v-else-if="folderResult" class="home-hint">{{ folderResult }}</span>
        </div>
      </div>
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
  max-width: 24rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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

.home-sources,
.home-mine {
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
  cursor: pointer;
}

.home-tag-body {
  padding-left: 1.85em;
}

/* The app draws its own checkbox; a native one is a bright OS control here. */
.tag-checkbox {
  flex-shrink: 0;
  width: 15px;
  height: 15px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1.5px solid rgba(255, 255, 255, 0.25);
  border-radius: 3px;
  color: #fff;

  &--checked {
    background: var(--accent);
    border-color: var(--accent);
  }
}

/* With a tag selected, the start.gg linker and its results dropdown live
   inside this list. Letting it grow unbounded pushed the whole card past the
   window; cap it instead, generous enough to roughly match the height of the
   shared-database tile on the right. */
.tag-list--open {
  max-height: 30rem;
  overflow-y: auto;
}

.home-sel {
  color: var(--text-muted);
}

.home-shared {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.5em;
  padding: 0.5em 0.25em 0.75em;

  p { margin: 0; }
}

.home-submit {
  display: flex;
  align-items: center;
  gap: 0.5em;
  padding: 0.6em 0.25em 0.4em;
  border-top: 1px solid var(--line-divider);
  flex-wrap: wrap;

  .btn {
    flex: 1 1 0;
    width: auto;
  }
}

.home-sources-head {
  padding-bottom: 0.2em;
  min-height: 2.5em;
  display: flex;
  align-items: center;
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

  .btn {
    flex: 0 0 auto;
    width: auto;
    padding-inline: 1em;
  }
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
  flex-wrap: wrap;

  .btn {
    flex: 1 1 0;
    width: auto;
  }
}

.home-overwrite {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.75rem;
  color: var(--text-muted);
  cursor: pointer;
  user-select: none;
  white-space: nowrap;

  &:hover {
    color: var(--text-primary);
  }
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

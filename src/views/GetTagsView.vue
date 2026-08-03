<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import AnimatedCard from '../components/AnimatedCard.vue';
import CloudSearchPanel from '../components/CloudSearchPanel.vue';
import ImportReview from '../components/ImportReview.vue';
import SaveStatusNotice from '../components/SaveStatusNotice.vue';
import TabBar from '../components/TabBar.vue';
import ViewHeader from '../components/ViewHeader.vue';
import { apiBaseUrl, cloudConfigured, CLOUD_UNCONFIGURED_MESSAGE } from '../cloud';
import { useCloudSearch } from '../composables/useCloudSearch';
import { useSaveFile } from '../composables/useSaveFile';
import { useStagedTags } from '../composables/useStagedTags';
import type { CloudDownload, PackSummary, PreviewResult, TagPreview, UnpackResult } from '../types';

/**
 * Everything that brings tags *onto* this PC. Cloud lookup comes first; local
 * files are the offline fallback.
 *
 * Player search, tournament search, `.r2tag` files, and `.r2pack` archives are
 * four producers of the same {previews, staged paths} pair, consumed by a
 * single <ImportReview>. Cloud browsing and pack-saving deliberately work with
 * no save file loaded — the machine doing the downloading may not have the game
 * installed at all.
 */

type TabName = 'player' | 'tournament' | 'files';

const TABS = [
  { id: 'player', label: 'Find Player' },
  { id: 'tournament', label: 'Find Tournament' },
  { id: 'files', label: 'From Files' },
] as const satisfies readonly { id: TabName; label: string }[];

const emit = defineEmits<{ 'go-back': [] }>();

const save = useSaveFile();
const search = useCloudSearch();
const staged = useStagedTags();

const tab = ref<TabName>('player');
const errorMsg = ref(cloudConfigured ? '' : CLOUD_UNCONFIGURED_MESSAGE);
const isWorking = ref(false);
const workingLabel = ref('');

const previews = ref<TagPreview[]>([]);
const saveVersion = ref<number | null>(null);
const packInfo = ref<UnpackResult | null>(null);
const packSaved = ref<PackSummary | null>(null);

const isCloudTab = computed(() => tab.value !== 'files');
const busy = computed(() => isWorking.value || search.isWorking.value);

/** A pack built for a different save format can't contribute anything. */
const packVersionMismatch = computed(() => {
  const declared = packInfo.value?.declaredSaveVersion;
  return declared != null && saveVersion.value != null && declared !== saveVersion.value;
});

async function switchTab(next: TabName) {
  if (next === tab.value) return;
  search.cancel();
  tab.value = next;
  search.reset();
  errorMsg.value = cloudConfigured ? '' : CLOUD_UNCONFIGURED_MESSAGE;
  packSaved.value = null;
  await resetImport();
}

async function runSearch() {
  errorMsg.value = '';
  packSaved.value = null;
  errorMsg.value =
    tab.value === 'player' ? await search.searchPlayer() : await search.searchTournament();
}

/** Download the current selection into staging, paired with its metadata. */
async function downloadSelected(): Promise<CloudDownload[]> {
  const tags = search.selectedResults.value.map((tag) => ({
    startggUserId: tag.startggUserId,
    uncompressedSha256: tag.uncompressedSha256,
  }));
  const downloads = await invoke<CloudDownload[]>('cloud_download_tags', { apiBaseUrl, tags });
  staged.add(downloads.map((download) => download.path));
  return downloads;
}

async function review(paths: string[]) {
  const result = await invoke<PreviewResult>('get_tag_previews', {
    r2tagPaths: paths,
    savePath: save.path.value,
  });
  previews.value = result.previews;
  saveVersion.value = result.save_version;
}

async function importSelected() {
  if (!search.selectedResults.value.length) return;
  errorMsg.value = '';
  isWorking.value = true;
  workingLabel.value = 'Downloading tags…';
  try {
    await staged.cleanup();
    await review((await downloadSelected()).map((download) => download.path));
  } catch (error) {
    await staged.cleanup();
    errorMsg.value = String(error);
  } finally {
    isWorking.value = false;
  }
}

/** Turn a tournament slug into a sensible default archive name. */
function defaultPackName(): string {
  const fromSlug = search.tournamentSlug.value.split('/').filter(Boolean).pop();
  const raw = fromSlug || search.tournamentName.value || 'tags';
  const cleaned = raw
    .replace(/[^A-Za-z0-9 ._-]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, 60);
  return `${cleaned || 'tags'}.r2pack`;
}

async function savePack() {
  if (!search.selectedResults.value.length) return;
  errorMsg.value = '';
  packSaved.value = null;

  // Dialog first: cancelling here should cost no network traffic.
  const outputPath = await saveDialog({
    title: 'Save Tag Pack',
    defaultPath: defaultPackName(),
    filters: [{ name: 'Tag pack', extensions: ['r2pack'] }],
  });
  if (!outputPath) return;

  isWorking.value = true;
  workingLabel.value = 'Downloading and packing tags…';
  try {
    const downloads = await downloadSelected();
    const byUser = new Map(downloads.map((download) => [download.startggUserId, download.path]));
    const entries = search.selectedResults.value.flatMap((tag) => {
      const path = byUser.get(tag.startggUserId);
      return path ? [{ path, gamerTag: tag.gamerTag, startggSlug: tag.startggSlug }] : [];
    });

    packSaved.value = await invoke<PackSummary>('pack_tag_files', {
      entries,
      outputPath,
      label: search.tournamentName.value || null,
      source: search.tournamentSlug.value || null,
    });
  } catch (error) {
    errorMsg.value = String(error);
  } finally {
    isWorking.value = false;
  }
  // Staged downloads are kept: the user may still import the same selection.
}

async function chooseFiles() {
  errorMsg.value = '';
  const picked = await open({
    multiple: true,
    title: 'Choose Tag Files',
    filters: [{ name: 'Tags', extensions: ['r2tag', 'r2pack'] }],
  });
  const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
  if (!paths.length) return;

  isWorking.value = true;
  workingLabel.value = 'Reading tag files…';
  try {
    await staged.cleanup();
    packInfo.value = null;

    const loose: string[] = [];
    const extracted: string[] = [];
    for (const path of paths) {
      if (path.toLowerCase().endsWith('.r2pack')) {
        const result = await invoke<UnpackResult>('unpack_r2pack', { archivePath: path });
        staged.add(result.paths);
        extracted.push(...result.paths);
        // One banner only; multi-pack selections are an edge case.
        packInfo.value = packInfo.value ?? result;
      } else {
        loose.push(path);
      }
    }

    await review([...loose, ...extracted]);
  } catch (error) {
    await staged.cleanup();
    packInfo.value = null;
    errorMsg.value = String(error);
  } finally {
    isWorking.value = false;
  }
}

async function resetImport() {
  previews.value = [];
  saveVersion.value = null;
  packInfo.value = null;
  await staged.cleanup();
}

async function importFinished() {
  await save.reload();
  await staged.cleanup();
}

// Module-scoped composables have no lifecycle of their own: without this an
// in-flight tournament scan would keep paging after the user pressed Back, and
// staged downloads would sit in the cache until the 24h sweep.
onBeforeUnmount(() => {
  search.cancel();
  void staged.cleanup();
});
</script>

<template>
  <AnimatedCard>
    <ViewHeader title="Get Tags" @go-back="emit('go-back')" />

    <ImportReview
      v-if="previews.length"
      :save-path="save.path.value"
      :tag-names="[...save.tagNames.value]"
      :previews="previews"
      :save-version="saveVersion"
      :reset-label="isCloudTab ? 'Back to Search' : 'Choose More Files'"
      @reset="resetImport"
      @finished="importFinished"
    >
      <template #banner>
        <p v-if="packInfo && packVersionMismatch" class="banner banner--warn">
          This pack was made for save v{{ packInfo.declaredSaveVersion }}; your save is
          v{{ saveVersion }} — none of these tags can be imported.
        </p>
        <p v-else-if="packInfo?.label" class="banner">
          From <strong>{{ packInfo.label }}</strong> · {{ packInfo.entryCount }} tag(s)
        </p>
        <p v-else-if="packInfo && !packInfo.manifestOk" class="banner">
          No pack info — reading tags directly.
        </p>
        <p v-if="packInfo?.skipped.length" class="banner banner--warn">
          {{ packInfo.skipped.length }} tag(s) in the pack were damaged and were left out.
        </p>
      </template>
    </ImportReview>

    <template v-else>
      <TabBar :tabs="TABS" :model-value="tab" @update:model-value="switchTab" />

      <div v-if="isCloudTab" class="view-stack">
        <CloudSearchPanel
          v-model:query="search.query.value"
          :placeholder="tab === 'player' ? 'Exact gamer tag or profile URL' : 'Tournament URL or slug'"
          :results="search.results.value"
          :selected="search.selected.value"
          :all-selected="search.allSelected.value"
          :is-working="busy"
          :disabled="!cloudConfigured"
          @search="runSearch"
          @toggle="search.toggleSelected"
          @toggle-all="search.toggleAll"
        />

        <p v-if="search.progress.value" class="hint">{{ search.progress.value }}</p>

        <template v-if="search.isWorking.value">
          <div class="loading-panel">
            {{ tab === 'tournament' ? 'Scanning registered players…' : 'Searching…' }}
          </div>
          <button v-if="tab === 'tournament'" class="btn btn-primary btn-primary-muted" @click="search.cancel()">
            Stop Scanning
          </button>
        </template>

        <div v-else-if="isWorking" class="loading-panel">{{ workingLabel }}</div>

        <template v-else-if="packSaved">
          <div class="result-panel result-panel--success">
            <span class="result-panel-msg">
              Saved {{ packSaved.entryCount }} tag{{ packSaved.entryCount === 1 ? '' : 's' }} to
              <span class="result-panel-path">{{ packSaved.outputPath }}</span>
            </span>
          </div>
          <div class="action-row">
            <button class="btn btn-primary" @click="packSaved = null">Back to Results</button>
            <button
              class="btn btn-primary btn-primary-muted"
              @click="revealItemInDir(packSaved!.outputPath)"
            >
              Show in Folder
            </button>
          </div>
        </template>

        <template v-else-if="search.results.value.length">
          <SaveStatusNotice v-if="!save.canWriteSave.value" context="download" />
          <div class="action-row">
            <button
              class="btn btn-primary"
              :disabled="!search.selected.value.size || !save.canWriteSave.value"
              @click="importSelected"
            >
              Import {{ search.selected.value.size }} Tag{{ search.selected.value.size === 1 ? '' : 's' }}
            </button>
            <button
              class="btn btn-primary btn-primary-muted"
              :disabled="!search.selected.value.size"
              @click="savePack"
            >
              Save as .r2pack
            </button>
          </div>
        </template>
      </div>

      <div v-else class="view-stack">
        <div v-if="isWorking" class="loading-panel">{{ workingLabel }}</div>
        <template v-else>
          <SaveStatusNotice v-if="!save.canWriteSave.value" context="import" />
          <button class="btn btn-primary" :disabled="!save.canWriteSave.value" @click="chooseFiles">
            Choose Tag Files
          </button>
          <p class="hint hint--center">
            Pick <code>.r2tag</code> files, or a <code>.r2pack</code> from a tournament.
          </p>
        </template>
      </div>

      <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>
    </template>
  </AnimatedCard>
</template>

<style scoped lang="scss">
.action-row {
  width: 100%;
  display: flex;
  gap: 0.625rem;

  .btn {
    flex: 1;
    font-size: 0.82em;
  }
}

.hint {
  width: 100%;
  color: var(--text-muted);
  font-size: 0.78rem;

  &--center {
    text-align: center;
  }
}

.banner {
  width: 100%;
  font-size: 0.78rem;
  color: var(--text-muted);

  &--warn {
    color: var(--text-warning);
  }
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
    font-family: 'Ubuntu Sans Mono Variable', monospace;
    word-break: break-all;
  }
}
</style>

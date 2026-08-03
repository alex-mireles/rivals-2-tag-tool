<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import AnimatedCard from '../components/AnimatedCard.vue';
import ImportReview from '../components/ImportReview.vue';
import SavePathBar from '../components/SavePathBar.vue';
import ViewHeader from '../components/ViewHeader.vue';
import type { CloudTagMetadata, CloudUser, PreviewResult, TagPreview, TournamentTagPage } from '../types';

type TabName = 'player' | 'tournament' | 'mine';
interface AuthRequest { requestId: string; pollToken: string; authorizationUrl: string; expiresAt: number }
interface PollResult { status: 'pending' | 'complete'; sessionToken?: string; user?: CloudUser }

const props = defineProps<{ savePath: string; tagNames: string[] }>();
const emit = defineEmits<{ 'go-back': []; 'tags-changed': [names: string[]] }>();
const apiBaseUrl = import.meta.env.VITE_CLOUD_API_BASE_URL?.trim() ?? '';

const tab = ref<TabName>('player');
const query = ref('');
const results = ref<CloudTagMetadata[]>([]);
const selected = ref<Set<string>>(new Set());
const errorMsg = ref(apiBaseUrl ? '' : 'Cloud service URL is not configured for this build.');
const isWorking = ref(false);
const progress = ref('');
const previews = ref<TagPreview[]>([]);
const saveVersion = ref<number | null>(null);
const cloudPaths = ref<string[]>([]);
const sessionToken = ref('');
const signedInUser = ref<CloudUser | null>(null);
const publishedTag = ref<CloudTagMetadata | null>(null);
const uploadTagName = ref(props.tagNames[0] ?? '');
let cancelTournament = false;

const selectedResults = computed(() => results.value.filter((tag) => selected.value.has(tag.startggUserId)));

function setResults(items: CloudTagMetadata[], selectAll = true) {
  results.value = items;
  selected.value = selectAll ? new Set(items.map((item) => item.startggUserId)) : new Set();
}

function toggleSelected(id: string) {
  const next = new Set(selected.value);
  if (next.has(id)) next.delete(id); else next.add(id);
  selected.value = next;
}

function switchTab(next: TabName) {
  cancelTournament = true;
  tab.value = next;
  setResults([]);
  progress.value = '';
  errorMsg.value = apiBaseUrl ? '' : errorMsg.value;
}

async function searchPlayer() {
  if (!query.value.trim() || !apiBaseUrl) return;
  errorMsg.value = '';
  isWorking.value = true;
  try {
    setResults(await invoke<CloudTagMetadata[]>('cloud_search_tags', { apiBaseUrl, query: query.value }));
    if (!results.value.length) progress.value = 'No published cloud tag matched that exact gamer tag or profile.';
  } catch (error) {
    errorMsg.value = String(error);
  } finally {
    isWorking.value = false;
  }
}

const wait = (milliseconds: number) => new Promise((resolve) => window.setTimeout(resolve, milliseconds));

async function searchTournament() {
  if (!query.value.trim() || !apiBaseUrl) return;
  errorMsg.value = '';
  isWorking.value = true;
  cancelTournament = false;
  const matches = new Map<string, CloudTagMetadata>();
  try {
    let page = 1;
    let totalPages = 1;
    do {
      const response = await invoke<TournamentTagPage>('cloud_tournament_tags', { apiBaseUrl, slug: query.value, page });
      totalPages = response.totalPages;
      for (const tag of response.matches) matches.set(tag.startggUserId, tag);
      progress.value = `${response.tournamentName}: scanned page ${page} of ${Math.max(totalPages, 1)} · ${matches.size} uploaded tag(s) found`;
      page += 1;
      if (page <= totalPages && !cancelTournament) await wait(1100);
    } while (page <= totalPages && !cancelTournament);
    setResults([...matches.values()]);
  } catch (error) {
    errorMsg.value = String(error);
  } finally {
    isWorking.value = false;
  }
}

async function cleanupDownloads() {
  if (!cloudPaths.value.length) return;
  const paths = cloudPaths.value;
  cloudPaths.value = [];
  await invoke('cleanup_cloud_files', { paths }).catch(() => undefined);
}

async function prepareImport() {
  if (!selectedResults.value.length) return;
  errorMsg.value = '';
  isWorking.value = true;
  try {
    await cleanupDownloads();
    const tags = selectedResults.value.map((tag) => ({
      startggUserId: tag.startggUserId,
      uncompressedSha256: tag.uncompressedSha256,
    }));
    cloudPaths.value = await invoke<string[]>('cloud_download_tags', { apiBaseUrl, tags });
    const result = await invoke<PreviewResult>('get_tag_previews', { r2tagPaths: cloudPaths.value, savePath: props.savePath });
    previews.value = result.previews;
    saveVersion.value = result.save_version;
  } catch (error) {
    await cleanupDownloads();
    errorMsg.value = String(error);
  } finally {
    isWorking.value = false;
  }
}

async function resetImport() {
  previews.value = [];
  saveVersion.value = null;
  await cleanupDownloads();
}

async function importFinished() {
  emit('tags-changed', await invoke<string[]>('get_tag_names', { savePath: props.savePath }));
  await cleanupDownloads();
}

async function signIn() {
  if (!apiBaseUrl) return;
  errorMsg.value = '';
  isWorking.value = true;
  try {
    const request = await invoke<AuthRequest>('cloud_begin_auth', { apiBaseUrl });
    await openUrl(request.authorizationUrl);
    progress.value = 'Complete authentication in your browser…';
    while (Date.now() / 1000 < request.expiresAt) {
      await wait(1500);
      const poll = await invoke<PollResult>('cloud_poll_auth', {
        apiBaseUrl, requestId: request.requestId, pollToken: request.pollToken,
      });
      if (poll.status === 'complete' && poll.sessionToken && poll.user) {
        sessionToken.value = poll.sessionToken;
        signedInUser.value = poll.user;
        const owned = await invoke<CloudTagMetadata[]>('cloud_search_tags', { apiBaseUrl, query: poll.user.slug });
        publishedTag.value = owned.find((tag) => tag.startggUserId === poll.user?.startggUserId) ?? null;
        progress.value = '';
        return;
      }
    }
    throw new Error('Authentication request expired.');
  } catch (error) {
    errorMsg.value = String(error);
  } finally {
    isWorking.value = false;
  }
}

async function signOut() {
  if (sessionToken.value) await invoke('cloud_end_session', { apiBaseUrl, sessionToken: sessionToken.value }).catch(() => undefined);
  sessionToken.value = '';
  signedInUser.value = null;
  publishedTag.value = null;
}

async function uploadTag() {
  if (!sessionToken.value || !uploadTagName.value) return;
  errorMsg.value = '';
  isWorking.value = true;
  try {
    publishedTag.value = await invoke<CloudTagMetadata>('cloud_upload_tag', {
      apiBaseUrl, sessionToken: sessionToken.value, savePath: props.savePath, tagName: uploadTagName.value,
    });
    progress.value = 'Cloud tag published successfully.';
  } catch (error) {
    errorMsg.value = String(error);
  } finally {
    isWorking.value = false;
  }
}

async function deleteTag() {
  if (!sessionToken.value || !publishedTag.value || !window.confirm('Remove your published cloud tag?')) return;
  isWorking.value = true;
  try {
    await invoke('cloud_delete_tag', { apiBaseUrl, sessionToken: sessionToken.value });
    publishedTag.value = null;
    progress.value = 'Cloud tag removed.';
  } catch (error) {
    errorMsg.value = String(error);
  } finally {
    isWorking.value = false;
  }
}

onBeforeUnmount(() => {
  cancelTournament = true;
  void cleanupDownloads();
});
</script>

<template>
  <AnimatedCard>
    <ViewHeader title="Cloud Tags" @go-back="emit('go-back')" />
    <SavePathBar :label="savePath" />

    <ImportReview v-if="previews.length" :save-path="savePath" :tag-names="tagNames" :previews="previews" :save-version="saveVersion" reset-label="Back to Search" @reset="resetImport" @finished="importFinished" />

    <template v-else>
      <div class="tabs">
        <button v-for="name in (['player', 'tournament', 'mine'] as TabName[])" :key="name" :class="{ active: tab === name }" @click="switchTab(name)">
          {{ name === 'player' ? 'Find Player' : name === 'tournament' ? 'Find Tournament' : 'My Tag' }}
        </button>
      </div>

      <div v-if="tab !== 'mine'" class="view-stack">
        <div class="search-row">
          <input v-model="query" :placeholder="tab === 'player' ? 'Exact gamer tag or profile URL' : 'Tournament URL or slug'" @keyup.enter="tab === 'player' ? searchPlayer() : searchTournament()" />
          <button class="btn btn-primary" :disabled="isWorking || !query.trim() || !apiBaseUrl" @click="tab === 'player' ? searchPlayer() : searchTournament()">Search</button>
        </div>
        <p v-if="progress" class="hint">{{ progress }}</p>
        <div v-if="isWorking" class="loading-panel">{{ tab === 'tournament' ? 'Scanning registered players…' : 'Searching…' }}</div>
        <div v-else-if="results.length" class="tag-panel">
          <div class="tag-panel-header"><span class="tag-panel-label">Available Tags</span><span>{{ selected.size }} selected</span></div>
          <ul class="tag-list">
            <li v-for="tag in results" :key="tag.startggUserId" class="tag-row cloud-row" @click="toggleSelected(tag.startggUserId)">
              <input type="checkbox" :checked="selected.has(tag.startggUserId)" @click.stop="toggleSelected(tag.startggUserId)" />
              <div><strong>{{ tag.gamerTag }}</strong><small>{{ tag.startggSlug }} · in-game: {{ tag.tagName }}</small></div>
              <span v-if="tag.saveVersion !== null">v{{ tag.saveVersion }}</span>
            </li>
          </ul>
        </div>
        <button v-if="results.length" class="btn btn-primary" :disabled="selected.size === 0 || isWorking" @click="prepareImport">Review {{ selected.size }} Selected Tag{{ selected.size === 1 ? '' : 's' }}</button>
      </div>

      <div v-else class="view-stack">
        <button v-if="!signedInUser" class="btn btn-primary" :disabled="isWorking || !apiBaseUrl" @click="signIn">Sign in with start.gg</button>
        <template v-else>
          <div class="identity"><strong>{{ signedInUser.gamerTag }}</strong><span>{{ signedInUser.slug }}</span><button @click="signOut">Sign out</button></div>
          <div v-if="publishedTag" class="published"><span>Published tag</span><strong>{{ publishedTag.tagName }}</strong><small>Updated {{ new Date(publishedTag.updatedAt).toLocaleString() }}</small></div>
          <label class="upload-label">Tag from loaded save
            <select v-model="uploadTagName" :disabled="!tagNames.length"><option v-for="name in tagNames" :key="name" :value="name">{{ name }}</option></select>
          </label>
          <p class="disclosure">Publishing makes your start.gg gamer tag, profile slug, in-game tag name, and controls file publicly downloadable.</p>
          <button class="btn btn-primary" :disabled="isWorking || !uploadTagName" @click="uploadTag">{{ publishedTag ? 'Replace Published Tag' : 'Publish Tag' }}</button>
          <button v-if="publishedTag" class="danger-btn" :disabled="isWorking" @click="deleteTag">Delete Published Tag</button>
        </template>
        <div v-if="isWorking" class="loading-panel">Working with the cloud service…</div>
        <p v-if="progress" class="hint">{{ progress }}</p>
      </div>
      <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>
    </template>
  </AnimatedCard>
</template>

<style scoped lang="scss">
.tabs { width: 100%; display: flex; gap: .35rem; }
.tabs button { flex: 1; border: 1px solid var(--line); border-radius: .4rem; padding: .45rem; background: var(--surface-inset); color: var(--text-muted); cursor: pointer; }
.tabs button.active { color: white; border-color: var(--accent); background: var(--accent-completed); }
.search-row { width: 100%; display: grid; grid-template-columns: 1fr 7rem; gap: .5rem; }
.search-row input, select { min-width: 0; padding: .65rem; color: white; background: var(--surface-inset); border: 1px solid var(--line); border-radius: .4rem; }
.cloud-row { gap: .6rem; cursor: pointer; }
.cloud-row > div { min-width: 0; flex: 1; display: flex; flex-direction: column; }
.cloud-row small { color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.hint, .disclosure { width: 100%; color: var(--text-muted); font-size: .76rem; }
.identity, .published { width: 100%; display: flex; align-items: center; gap: .6rem; padding: .7rem; background: var(--surface-inset); border-radius: .4rem; }
.identity span, .published small { color: var(--text-muted); font-size: .75rem; flex: 1; }
.identity button { background: none; border: 0; color: var(--text-muted); cursor: pointer; }
.published { flex-direction: column; align-items: flex-start; gap: .2rem; }
.upload-label { width: 100%; display: flex; flex-direction: column; gap: .35rem; color: var(--text-muted); font-size: .8rem; }
.danger-btn { width: 100%; padding: .65rem; border: 1px solid rgba(248,113,113,.4); border-radius: .4rem; background: rgba(248,113,113,.1); color: var(--text-failure); cursor: pointer; }
</style>

<script setup lang="ts">
// The shared tag database, always on screen rather than somewhere you navigate
// to: a searchable, scrolling list of what other people have published, each
// row showing the in-game tag, the start.gg account behind it, and an on-demand
// "what this tag changes" diff.

import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import TagDiff from './TagDiff.vue';

export interface SharedTag {
  name: string;
  author: string;
  file: string;
  startggSlug: string;
  startggTag: string;
}

defineProps<{
  /** Files currently selected, keyed by manifest file name. */
  selected: Set<string>;
  /** Downloaded copies, so a row can show its diff without re-fetching. */
  previewPaths?: Record<string, string>;
}>();

const emit = defineEmits<{
  toggle: [file: string];
  loaded: [tags: SharedTag[]];
  preview: [file: string];
}>();

const tags = ref<SharedTag[]>([]);
const loading = ref(false);
const errorMsg = ref('');
const query = ref('');

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return tags.value;
  return tags.value.filter(
    t =>
      t.name.toLowerCase().includes(q) ||
      t.startggTag.toLowerCase().includes(q) ||
      t.author.toLowerCase().includes(q),
  );
});

async function load() {
  loading.value = true;
  errorMsg.value = '';
  try {
    tags.value = await invoke<SharedTag[]>('fetch_shared_tags');
    emit('loaded', tags.value);
  } catch (err) {
    errorMsg.value = String(err);
  } finally {
    loading.value = false;
  }
}

onMounted(load);
defineExpose({ reload: load });
</script>

<template>
  <div class="browser">
    <div class="browser-head">
      <div class="browser-title">
        Shared database
        <span v-if="tags.length" class="browser-count">{{ tags.length }} tags</span>
      </div>
      <input
        v-model="query"
        class="browser-search"
        type="search"
        placeholder="Search tag or start.gg name"
      />
    </div>

    <p v-if="loading" class="browser-note">Loading published tags…</p>
    <p v-else-if="errorMsg" class="browser-note browser-note--error">{{ errorMsg }}</p>
    <p v-else-if="!filtered.length && query" class="browser-note">
      Nothing matches “{{ query }}”.
    </p>
    <p v-else-if="!filtered.length" class="browser-note">No tags published yet.</p>

    <ul v-else class="browser-list">
      <li v-for="t in filtered" :key="t.file" class="browser-row">
        <label class="browser-main">
          <input
            type="checkbox"
            :checked="selected.has(t.file)"
            @change="emit('toggle', t.file)"
          />
          <span class="browser-name">{{ t.name }}</span>
          <span v-if="t.startggTag" class="browser-startgg">{{ t.startggTag }}</span>
        </label>
        <TagDiff
          v-if="previewPaths && previewPaths[t.file]"
          :path="previewPaths[t.file]"
          class="browser-diff"
        />
        <button v-else class="browser-peek" @click="emit('preview', t.file)">
          What this tag changes
        </button>
      </li>
    </ul>
  </div>
</template>

<style scoped lang="scss">
.browser-head {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-bottom: 0.5rem;
}

.browser-title {
  font-weight: 500;
  white-space: nowrap;
}

.browser-count {
  margin-left: 0.4rem;
  font-size: 0.8rem;
  font-weight: 400;
  color: var(--text-muted);
}

.browser-search {
  flex: 1;
  min-width: 0;
}

.browser-note {
  margin: 0.5rem 0;
  font-size: 0.85rem;
  color: var(--text-muted);

  &--error {
    color: #e06c75;
  }
}

/* A sample of the database is always visible; the rest is a scroll away
   rather than another screen. */
.browser-list {
  list-style: none;
  margin: 0;
  padding: 0;
  max-height: 15rem;
  overflow-y: auto;
}

.browser-row {
  padding: 0.35rem 0;
  border-top: 1px solid var(--border, rgba(255, 255, 255, 0.08));

  &:first-child {
    border-top: none;
  }
}

.browser-main {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
  cursor: pointer;
}

.browser-name {
  font-weight: 500;
}

.browser-startgg {
  font-size: 0.8rem;
  color: var(--text-muted);
}

.browser-peek {
  margin-top: 0.15rem;
  padding: 0;
  border: none;
  background: none;
  font: inherit;
  font-size: 0.78rem;
  color: var(--text-muted);
  cursor: pointer;
  text-decoration: underline;
  text-underline-offset: 2px;

  &:hover {
    color: var(--text-primary, inherit);
  }
}

.browser-diff {
  margin-left: 1.4rem;
}
</style>

<script setup lang="ts">
// The shared tag database, always on screen rather than somewhere you navigate
// to: a searchable, scrolling list of what other people have published, each
// row showing the in-game tag, the start.gg account behind it, and an on-demand
// "what this tag changes" diff.

import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import TagDiff from './TagDiff.vue';

export interface SharedTag {
  name: string;
  author: string;
  file: string;
  startggSlug: string;
  startggTag: string;
}

const props = defineProps<{
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
  const base = !q
    ? tags.value
    : tags.value.filter(
        t =>
          t.name.toLowerCase().includes(q) ||
          t.startggTag.toLowerCase().includes(q) ||
          t.author.toLowerCase().includes(q),
      );
  // Selected rows float to the top so you can see what you've picked; a
  // stable sort keeps everything else in its existing relative order.
  return [...base].sort(
    (a, b) => Number(props.selected.has(b.file)) - Number(props.selected.has(a.file)),
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

/** Open a publisher's start.gg profile, the way the website's @handles do. */
function openProfile(slug: string) {
  if (slug) openUrl(`https://start.gg/${slug}`);
}

onMounted(load);
defineExpose({ reload: load });
</script>

<template>
  <div class="browser">
    <div class="browser-head">
      <span class="browser-title">Shared database</span>
      <span v-if="tags.length" class="browser-count">{{ tags.length }}</span>
      <input v-model="query" class="browser-search" type="search" placeholder="Search" />
    </div>

    <p v-if="loading" class="browser-note">Loading published tags…</p>
    <p v-else-if="errorMsg" class="browser-note browser-note--error">{{ errorMsg }}</p>
    <p v-else-if="!filtered.length && query" class="browser-note">
      Nothing matches “{{ query }}”.
    </p>
    <p v-else-if="!filtered.length" class="browser-note">No tags published yet.</p>

    <ul v-else class="browser-list">
      <li v-for="t in filtered" :key="t.file" class="browser-row">
        <div class="browser-line">
          <div class="browser-main" @click="emit('toggle', t.file)">
            <span
              class="tag-checkbox"
              :class="{ 'tag-checkbox--checked': selected.has(t.file) }"
              aria-hidden="true"
            >
              <svg v-if="selected.has(t.file)" xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24">
                <path fill="currentColor" d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
              </svg>
            </span>
            <span class="tag-name browser-name">{{ t.name }}</span>
            <button
              v-if="t.startggTag"
              class="browser-startgg"
              title="Open on start.gg"
              @click.stop="openProfile(t.startggSlug)"
            >@{{ t.startggTag }}</button>
          </div>
          <TagDiff
            v-if="previewPaths && previewPaths[t.file]"
            :path="previewPaths[t.file]"
            :default-open="true"
            :hide-count="true"
            class="browser-diff"
          />
          <button v-else class="browser-peek" @click="emit('preview', t.file)">
            <span class="browser-peek-arrow" aria-hidden="true">▸</span> changes
          </button>
        </div>
      </li>
    </ul>
  </div>
</template>

<style scoped lang="scss">
.browser {
  font-size: 1em;
}

.browser-head {
  display: flex;
  align-items: center;
  gap: 0.5em;
  margin-bottom: 0.4em;
}

.browser-title {
  text-transform: uppercase;
  letter-spacing: 0.1em;
  color: var(--text-muted);
  white-space: nowrap;
}

.browser-count {
  color: var(--text-muted);
  opacity: 0.7;
}

/* Inputs are styled per-view here; without this the browser default renders as
   a bright white box against the dark card. */
.browser-search {
  flex: 1;
  min-width: 0;
  font-family: inherit;
  font-size: 1em;
  color: var(--text-primary);
  background: rgba(0, 0, 0, 0.25);
  border: 1px solid var(--line);
  border-radius: var(--radius-button);
  padding: 0.3em 0.5em;

  &::placeholder {
    color: rgba(255, 255, 255, 0.35);
  }

  &:focus-visible {
    outline: 2px solid rgba(99, 102, 241, 0.6);
    outline-offset: 1px;
  }
}

.browser-note {
  margin: 0.4em 0;
  color: var(--text-muted);

  &--error {
    color: var(--text-failure);
  }
}

.browser-list {
  list-style: none;
  margin: 0;
  padding: 0;
  max-height: 12rem;
  overflow-y: auto;
}

.browser-row {
  padding: 0.3em 0;
  border-bottom: 1px solid var(--line-divider);
}

/* Name row and its "changes" expander share one line: align-items: flex-start
   keeps the name pinned to the top instead of drifting to the vertical
   centre once TagDiff expands. TagDiff/the peek button sit beside
   .browser-main rather than nested in it, so their clicks don't bubble into
   the row's own toggle handler. */
.browser-line {
  display: flex;
  align-items: flex-start;
  gap: 0.5em;
}

.browser-main {
  display: flex;
  align-items: center;
  gap: 0.4em;
  cursor: pointer;
}

/* Drawn, not native — a browser checkbox is a bright OS control on this card. */
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

.browser-startgg {
  padding: 0;
  border: none;
  background: none;
  font: inherit;
  color: var(--text-muted);
  cursor: pointer;

  &:hover {
    color: var(--text-primary);
    text-decoration: underline;
  }
}

.browser-peek-arrow {
  display: inline-block;
  font-size: 0.85em;
}

.browser-peek {
  flex-shrink: 0;
  padding: 0;
  border: none;
  background: none;
  font: inherit;
  font-size: 0.9em;
  color: var(--text-muted);
  cursor: pointer;

  &:hover {
    color: var(--text-primary);
  }
}

.browser-diff {
  flex: 1 1 auto;
  min-width: 0;
}
</style>

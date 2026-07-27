<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { StartggLinkValue } from '../types';

interface StartggPlayer {
  gamerTag: string;
  prefix: string;
  slug: string;
  image: string;
}

// `compact` drops the heading/hint and panel chrome so the linker can sit
// inline in a row (used per-tag in the Share view).
defineProps<{ compact?: boolean }>();

const model = defineModel<StartggLinkValue | null>({ default: null });

const root = ref<HTMLElement | null>(null);
const query = ref('');
const results = ref<StartggPlayer[]>([]);
const showResults = ref(false);
const searching = ref(false);
const errorMsg = ref('');

let debounce: number | undefined;
let seq = 0; // guards against out-of-order async responses

/** Pull a `user/<id>` slug out of a pasted profile URL or raw slug. */
function parseUserSlug(text: string): string | null {
  const m = text.trim().match(/user\/([a-z0-9]+)/i);
  return m ? `user/${m[1]}` : null;
}

async function runSearch(q: string) {
  const mySeq = ++seq;
  searching.value = true;
  errorMsg.value = '';
  try {
    const slug = parseUserSlug(q);
    if (slug) {
      // A pasted profile URL / slug resolves directly to one account.
      const player = await invoke<StartggPlayer>('startgg_user', { slug });
      if (mySeq !== seq) return;
      results.value = [player];
    } else {
      const players = await invoke<StartggPlayer[]>('startgg_search', { query: q });
      if (mySeq !== seq) return;
      results.value = players;
    }
    showResults.value = true;
  } catch (e) {
    if (mySeq !== seq) return;
    results.value = [];
    errorMsg.value = String(e);
    showResults.value = true;
  } finally {
    if (mySeq === seq) searching.value = false;
  }
}

watch(query, (q) => {
  window.clearTimeout(debounce);
  const trimmed = q.trim();
  if (trimmed.length < 2) {
    results.value = [];
    showResults.value = false;
    return;
  }
  debounce = window.setTimeout(() => runSearch(trimmed), 250);
});

function choose(p: StartggPlayer) {
  model.value = { slug: p.slug, tag: p.gamerTag };
  query.value = '';
  results.value = [];
  showResults.value = false;
}

function clearSelection() {
  model.value = null;
}

function label(p: StartggPlayer): string {
  return (p.prefix ? `${p.prefix} | ` : '') + (p.gamerTag || '(no tag)');
}

function initial(p: StartggPlayer): string {
  return (p.gamerTag || '?').slice(0, 1).toUpperCase();
}

function onDocMouseDown(e: MouseEvent) {
  if (root.value && !root.value.contains(e.target as Node)) {
    showResults.value = false;
  }
}

onMounted(() => document.addEventListener('mousedown', onDocMouseDown));
onBeforeUnmount(() => {
  document.removeEventListener('mousedown', onDocMouseDown);
  window.clearTimeout(debounce);
});
</script>

<template>
  <div ref="root" class="sgg" :class="{ 'sgg--compact': compact }">
    <template v-if="!compact">
      <span class="sgg-heading">start.gg account (if submitting)</span>
    </template>

    <!-- Selected -->
    <div v-if="model" class="sgg-chip">
      <span>Linked: <strong>@{{ model.tag || model.slug }}</strong></span>
      <button type="button" class="sgg-clear" title="Remove" @click="clearSelection">
        <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24">
          <path fill="currentColor" d="M13.46 12L19 17.54V19h-1.46L12 13.46L6.46 19H5v-1.46L10.54 12L5 6.46V5h1.46L12 10.54L17.54 5H19v1.46z"/>
        </svg>
      </button>
    </div>

    <!-- Search -->
    <div v-else class="sgg-search-wrap">
      <input
        v-model="query"
        type="search"
        class="sgg-input"
        placeholder="Search start.gg or paste a profile URL…"
        autocomplete="off"
        spellcheck="false"
        @focus="results.length && (showResults = true)"
      />
      <ul v-if="showResults" class="sgg-results">
        <li v-if="searching && !results.length" class="sgg-empty">Searching…</li>
        <li v-else-if="errorMsg" class="sgg-empty sgg-error">{{ errorMsg }}</li>
        <li v-else-if="!results.length" class="sgg-empty">No start.gg accounts found.</li>
        <li
          v-for="p in results"
          v-else
          :key="p.slug"
          class="sgg-result"
          @click="choose(p)"
        >
          <img v-if="p.image" class="sgg-avatar" :src="p.image" alt="" referrerpolicy="no-referrer" />
          <span v-else class="sgg-avatar sgg-avatar-empty">{{ initial(p) }}</span>
          <span class="sgg-result-text">
            <span class="sgg-result-tag">{{ label(p) }}</span>
            <span class="sgg-result-slug">{{ p.slug }}</span>
          </span>
        </li>
      </ul>
    </div>
  </div>
</template>

<style scoped lang="scss">
.sgg {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 0.35em;
  padding: 0.75rem 1rem;
  background: var(--surface-inset);
  border: 1px solid var(--line-subtle);
  border-radius: var(--radius-panel);

  &--compact {
    padding: 0;
    gap: 0;
    background: none;
    border: none;
  }
}

.sgg-heading {
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.1em;
  color: var(--text-muted);
}

.sgg-optional {
  color: var(--text-muted);
  letter-spacing: 0;
}

.sgg-search-wrap {
  position: relative;
  margin-top: 0.15em;
}

.sgg-input {
  width: 100%;
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

.sgg-results {
  position: absolute;
  z-index: 20;
  top: calc(100% + 0.25em);
  left: 0;
  right: 0;
  list-style: none;
  max-height: 15rem;
  overflow-y: auto;
  overscroll-behavior: contain;
  background: #161334;
  border: 1px solid var(--line);
  border-radius: var(--radius-panel);
  box-shadow: 0 8px 22px rgba(0, 0, 0, 0.45);
}

.sgg-empty {
  padding: 0.6em 0.7em;
  font-size: 0.8rem;
  color: var(--text-muted);
}

.sgg-error {
  color: var(--text-failure);
}

.sgg-result {
  display: flex;
  align-items: center;
  gap: 0.6em;
  padding: 0.45em 0.6em;
  cursor: pointer;
  border-bottom: 1px solid var(--line-divider);

  &:last-child {
    border-bottom: none;
  }

  &:hover {
    background: var(--surface-hover);
  }
}

.sgg-avatar {
  flex-shrink: 0;
  width: 30px;
  height: 30px;
  border-radius: 50%;
  object-fit: cover;
  background: var(--surface);
}

.sgg-avatar-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.8rem;
  font-weight: 700;
  color: var(--text-muted);
}

.sgg-result-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.sgg-result-tag {
  font-size: 0.85rem;
  color: var(--text-primary);
}

.sgg-result-slug {
  font-family: 'Ubuntu Sans Mono Variable', monospace;
  font-size: 0.7rem;
  color: var(--text-muted);
}

.sgg-chip {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5em;
  margin-top: 0.15em;
  padding: 0.45em 0.7em;
  font-size: 0.82rem;
  color: var(--text-primary);
  background: var(--accent-completed);
  border: 1px solid var(--accent);
  border-radius: var(--radius-button);

  strong {
    color: var(--text-primary);
  }
}

.sgg-clear {
  display: flex;
  align-items: center;
  justify-content: center;
  background: none;
  border: none;
  cursor: pointer;
  color: var(--text-muted);
  padding: 0.15em;
  border-radius: var(--radius-button);
  transition: color 0.15s;

  &:hover {
    color: var(--text-failure);
  }
}
</style>

<script setup lang="ts">
// "What does this tag change?" — a collapsed summary of how a tag's controls
// differ from the game's defaults, so you can see what you're installing
// before you write it into a save.
//
// The parse is done on demand (only when opened) because reading a .r2tag is
// a real file read, and a browse list can hold hundreds of them.

import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { diffTag, type TagDiff } from '../lib/tagDefaults';

const props = defineProps<{
  /** Path to a .r2tag file on disk. Mutually exclusive with savePath+tagName. */
  path?: string;
  /** A tag already inside a save: the save's path plus the tag's name. */
  savePath?: string;
  tagName?: string;
}>();

const diff = ref<TagDiff | null>(null);
const loading = ref(false);
const errorMsg = ref('');
const loaded = ref(false);

async function onToggle(ev: Event) {
  if (!(ev.target as HTMLDetailsElement).open || loaded.value || loading.value) return;
  loading.value = true;
  errorMsg.value = '';
  try {
    const root = props.path
      ? await invoke<unknown>('read_tag_json', { path: props.path })
      : await invoke<unknown>('read_tag_json_from_save', {
          savePath: props.savePath,
          tagName: props.tagName,
        });
    diff.value = diffTag(root);
    loaded.value = true;
  } catch (err) {
    errorMsg.value = String(err);
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <details class="tag-diff" @toggle="onToggle">
    <summary class="tag-diff-summary">
      <template v-if="diff">
        {{ diff.count === 0 ? 'stock controls' : `${diff.count} change${diff.count === 1 ? '' : 's'}` }}
      </template>
      <template v-else>{{ loading ? 'reading…' : 'changes' }}</template>
    </summary>

    <div class="tag-diff-body">
      <p v-if="loading" class="tag-diff-note">Reading tag…</p>
      <p v-else-if="errorMsg" class="tag-diff-note tag-diff-note--error">{{ errorMsg }}</p>
      <p v-else-if="diff && !diff.count" class="tag-diff-note">Same as the default controls.</p>

      <template v-else-if="diff">
        <div v-for="group in diff.groups" :key="group.scope" class="tag-diff-group">
          <div class="tag-diff-group-title">{{ group.scope }}</div>
          <ul class="tag-diff-list">
            <li v-for="item in group.items" :key="group.scope + item.label" class="tag-diff-item">
              <span class="tag-diff-key">{{ item.label }}</span>
              <span class="tag-diff-from">{{ item.from }}</span>
              <span class="tag-diff-arrow">→</span>
              <span class="tag-diff-to">{{ item.to }}</span>
            </li>
          </ul>
        </div>
      </template>
    </div>
  </details>
</template>

<style scoped lang="scss">
.tag-diff {
  margin-top: 0.15em;
}

.tag-diff-summary {
  cursor: pointer;
  font-size: 0.9em;
  color: var(--text-muted);
  user-select: none;

  &:hover {
    color: var(--text-primary);
  }
}

.tag-diff-body {
  padding: 0.35em 0 0.2em 1em;
}

.tag-diff-note {
  margin: 0;
  color: var(--text-muted);

  &--error {
    color: var(--text-failure);
  }
}

.tag-diff-group + .tag-diff-group {
  margin-top: 0.5em;
}

.tag-diff-group-title {
  text-transform: uppercase;
  letter-spacing: 0.1em;
  color: var(--text-muted);
  margin-bottom: 0.15em;
}

.tag-diff-list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.tag-diff-item {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 0.35em;
  padding: 0.1em 0;
}

.tag-diff-key {
  min-width: 8em;
  color: var(--text-muted);
}

.tag-diff-from {
  color: var(--text-muted);
  text-decoration: line-through;
}

.tag-diff-arrow {
  color: var(--text-muted);
}

.tag-diff-to {
  color: var(--text-primary);
}
</style>

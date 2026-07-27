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
      What this tag changes
      <span v-if="diff" class="tag-diff-count">
        {{ diff.count === 0 ? 'nothing' : `${diff.count} change${diff.count === 1 ? '' : 's'}` }}
      </span>
    </summary>

    <div class="tag-diff-body">
      <p v-if="loading" class="tag-diff-note">Reading tag…</p>
      <p v-else-if="errorMsg" class="tag-diff-note tag-diff-note--error">{{ errorMsg }}</p>
      <p v-else-if="diff && !diff.count" class="tag-diff-note">
        No differences from the default controls.
      </p>

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
  margin-top: 0.35rem;
}

.tag-diff-summary {
  cursor: pointer;
  font-size: 0.8rem;
  opacity: 0.75;
  user-select: none;

  &:hover {
    opacity: 1;
  }
}

.tag-diff-count {
  margin-left: 0.4rem;
  opacity: 0.7;
  font-variant-numeric: tabular-nums;
}

.tag-diff-body {
  padding: 0.4rem 0 0.2rem 0.9rem;
}

.tag-diff-note {
  margin: 0;
  font-size: 0.8rem;
  opacity: 0.7;

  &--error {
    opacity: 1;
    color: #e06c75;
  }
}

.tag-diff-group + .tag-diff-group {
  margin-top: 0.5rem;
}

.tag-diff-group-title {
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  opacity: 0.6;
  margin-bottom: 0.15rem;
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
  gap: 0.35rem;
  font-size: 0.8rem;
  padding: 0.1rem 0;
}

.tag-diff-key {
  min-width: 9rem;
  opacity: 0.85;
}

.tag-diff-from {
  opacity: 0.55;
  text-decoration: line-through;
}

.tag-diff-arrow {
  opacity: 0.5;
}

.tag-diff-to {
  font-weight: 600;
}
</style>

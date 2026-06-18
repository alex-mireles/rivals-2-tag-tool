<script setup lang="ts">
import { ref, reactive, computed, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import AnimatedCard from '../components/AnimatedCard.vue';
import SavePathBar from '../components/SavePathBar.vue';
import ViewHeader from '../components/ViewHeader.vue';
import StartggLink from '../components/StartggLink.vue';
import type { StartggLinkValue } from '../types';

const props = defineProps<{
  savePath: string;
  tagNames: string[];
}>();

const emit = defineEmits<{
  'go-back': [];
}>();

const included = ref<Set<string>>(new Set());
// Per-tag start.gg link, keyed by tag name.
const links = reactive<Record<string, StartggLinkValue | null>>({});
const isSharing = ref(false);
const result = ref<{ pr: string; number: number; count: number } | null>(null);
const errorMsg = ref('');

const includedNames = computed(() => props.tagNames.filter(n => included.value.has(n)));

// Every included tag must have a start.gg account linked before sharing.
const allLinked = computed(() =>
  includedNames.value.length > 0 && includedNames.value.every(n => !!links[n]?.slug)
);
const missingLinks = computed(() => includedNames.value.filter(n => !links[n]?.slug).length);

function toggle(name: string) {
  if (included.value.has(name)) {
    included.value.delete(name);
  } else {
    included.value.add(name);
    if (!(name in links)) links[name] = null;
  }
  included.value = new Set(included.value);
}

async function share() {
  errorMsg.value = '';
  isSharing.value = true;
  await nextTick();
  try {
    const items = includedNames.value.map(name => ({
      tagName: name,
      startgg: links[name] as StartggLinkValue,
    }));
    const res = await invoke<{ pr: string; number: number }>('share_tags_to_site', {
      savePath: props.savePath,
      items,
    });
    result.value = { ...res, count: items.length };
  } catch (err) {
    errorMsg.value = String(err);
  } finally {
    isSharing.value = false;
  }
}

async function openPr() {
  if (result.value?.pr) await openUrl(result.value.pr);
}

function reset() {
  result.value = null;
  errorMsg.value = '';
  included.value = new Set();
}
</script>

<template>
  <AnimatedCard>
    <ViewHeader title="Share to Site" @go-back="emit('go-back')" />

    <SavePathBar :label="savePath" />

    <Transition name="content-swap" mode="out-in">
      <!-- Success -->
      <div v-if="result" key="result" class="view-stack">
        <div class="result-panel result-panel--success">
          <span class="result-panel-msg">
            Submitted {{ result.count }} tag{{ result.count === 1 ? '' : 's' }} to the sharing site.
          </span>
          <span class="result-panel-note">
            A pull request was opened and auto-merges once it passes validation, then your
            tag appears on the site.
          </span>
        </div>
        <button v-if="result.pr" class="btn btn-primary" @click="openPr">
          View Pull Request #{{ result.number }}
        </button>
        <button class="btn btn-primary btn-primary-muted" @click="reset">Share More</button>
      </div>

      <!-- Loading -->
      <div v-else-if="isSharing" key="loading" class="loading-panel">Uploading to the site...</div>

      <!-- Selection + per-tag linking -->
      <div v-else key="select" class="view-stack">
        <p class="share-intro">
          Pick tags to publish on the tag-sharing site and link each to its start.gg account.
          Nothing is saved locally — the tags upload directly.
        </p>

        <div class="tag-panel">
          <div class="tag-panel-header">
            <span class="tag-panel-label">Tags to Share</span>
          </div>
          <ul class="tag-list share-list">
            <li v-for="name in tagNames" :key="name" class="share-row">
              <div
                class="share-head"
                :class="{ 'share-head--on': included.has(name) }"
                @click="toggle(name)"
              >
                <div class="tag-checkbox" :class="{ 'tag-checkbox--checked': included.has(name) }">
                  <svg v-if="included.has(name)" xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24">
                    <path fill="currentColor" d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
                  </svg>
                </div>
                <span class="tag-name">{{ name }}</span>
                <span v-if="included.has(name) && !links[name]?.slug" class="needs-link">needs start.gg</span>
                <span v-else-if="included.has(name)" class="linked-pill">@{{ links[name]?.tag || links[name]?.slug }}</span>
              </div>
              <div v-if="included.has(name)" class="share-link" @click.stop>
                <StartggLink v-model="links[name]" compact />
              </div>
            </li>
          </ul>
        </div>

        <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>

        <button
          class="btn btn-primary"
          :disabled="includedNames.length === 0 || !allLinked"
          @click="share"
        >
          {{
            includedNames.length === 0
              ? 'Select tags to share'
              : !allLinked
                ? `Link start.gg for ${missingLinks} tag${missingLinks === 1 ? '' : 's'}`
                : `Share ${includedNames.length} tag${includedNames.length === 1 ? '' : 's'}`
          }}
        </button>
      </div>
    </Transition>
  </AnimatedCard>
</template>

<style scoped lang="scss">
.share-intro {
  width: 100%;
  font-size: 0.8em;
  color: var(--text-muted);
  line-height: 1.4;
}

.share-list {
  max-height: 18rem;
}

.share-row {
  display: flex;
  flex-direction: column;
  border-bottom: 1px solid var(--line-divider);

  &:last-child {
    border-bottom: none;
  }
}

.share-head {
  display: flex;
  align-items: center;
  gap: 0.75em;
  padding: 0.5em 0.25em;
  cursor: pointer;

  &:hover {
    background: var(--surface-hover);
  }

  .tag-name {
    font-size: 1.2em;
    flex: 1;
    min-width: 0;
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

.needs-link {
  flex-shrink: 0;
  font-size: 0.75em;
  font-weight: 600;
  color: var(--text-warning);
  background: rgba(250, 204, 21, 0.1);
  border: 1px solid rgba(250, 204, 21, 0.25);
  border-radius: 0.4em;
  padding: 0.2em 0.5em;
}

.linked-pill {
  flex-shrink: 0;
  font-size: 0.75em;
  font-weight: 600;
  color: var(--text-success);
  background: rgba(0, 255, 170, 0.08);
  border: 1px solid rgba(0, 255, 170, 0.2);
  border-radius: 0.4em;
  padding: 0.2em 0.5em;
}

.share-link {
  padding: 0 0.25em 0.6em 1.85em;
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

  &-note {
    font-size: 0.78em;
    color: var(--text-muted);
    line-height: 1.4;
  }
}
</style>

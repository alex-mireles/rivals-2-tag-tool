<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import type { CloudTagMetadata } from '../types';

/**
 * `examples` replaces the static placeholder with a fading rotation of concrete
 * inputs — "slug" means nothing to most players, but `start.gg/user/a1b2c3d4`
 * does. The rotation is an overlay because `::placeholder` can't be animated.
 */
const props = defineProps<{
  placeholder: string;
  examples?: readonly string[];
  results: CloudTagMetadata[];
  selected: Set<string>;
  allSelected: boolean;
  isWorking: boolean;
  disabled: boolean;
}>();

const query = defineModel<string>('query', { required: true });

const emit = defineEmits<{ search: []; toggle: [id: string]; 'toggle-all': [] }>();

// Enter has to clear the same bar as the button beside it. Emitting regardless
// sends an empty or unconfigured search to the backend, which comes back as a
// URL-parse error in place of the message the view already put up.
function submit() {
  if (props.isWorking || props.disabled || !query.value.trim()) return;
  emit('search');
}

// The out-in swap leaves the field briefly empty, so keep the fades short
// relative to the hold — a long blank gap reads as flicker.
const EXAMPLE_INTERVAL_MS = 3600;

const index = ref(0);
const examples = computed(() => props.examples ?? []);
const showExamples = computed(() => examples.value.length > 0 && !query.value);
const currentExample = computed(() => examples.value[index.value] ?? '');

// Tab switches swap the example set out from under us; restart at the top so
// the first thing the user sees is the most common form of input.
watch(examples, () => {
  index.value = 0;
});

const timer = setInterval(() => {
  if (!showExamples.value) return;
  index.value = (index.value + 1) % examples.value.length;
}, EXAMPLE_INTERVAL_MS);

onBeforeUnmount(() => clearInterval(timer));
</script>

<template>
  <div class="search-row">
    <div class="search-field">
      <input
        v-model="query"
        :placeholder="showExamples ? '' : placeholder"
        :disabled="isWorking"
        @keyup.enter="submit"
      />
      <Transition name="example" mode="out-in">
        <span v-if="showExamples" :key="currentExample" class="search-example">
          e.g. {{ currentExample }}
        </span>
      </Transition>
    </div>
    <button
      class="btn btn-primary"
      :disabled="isWorking || !query.trim() || disabled"
      @click="submit"
    >
      <v-icon name="md-search-round" scale="0.85" />
      Search
    </button>
  </div>

  <div v-if="results.length" class="tag-panel">
    <div class="tag-panel-header">
      <span class="tag-panel-label">Available Tags</span>
      <button class="panel-btn" @click="$emit('toggle-all')">
        <v-icon name="md-doneall-round" scale="0.7" />
        {{ allSelected ? 'Deselect All' : 'Select All' }}
      </button>
    </div>
    <ul class="tag-list">
      <li
        v-for="tag in results"
        :key="tag.startggUserId"
        class="tag-row cloud-row"
        @click="$emit('toggle', tag.startggUserId)"
      >
        <div class="tag-checkbox" :class="{ 'tag-checkbox--checked': selected.has(tag.startggUserId) }">
          <v-icon v-if="selected.has(tag.startggUserId)" name="md-check-round" scale="0.7" />
        </div>
        <div class="cloud-row-detail">
          <strong>{{ tag.gamerTag }}</strong>
          <small>{{ tag.startggSlug }} · in-game: {{ tag.tagName }}</small>
        </div>
        <span v-if="tag.saveVersion !== null" class="cloud-row-version">v{{ tag.saveVersion }}</span>
      </li>
    </ul>
  </div>
</template>

<style scoped lang="scss">
.search-row {
  width: 100%;
  display: grid;
  grid-template-columns: 1fr 8rem;
  gap: 0.5rem;

  input {
    width: 100%;
    min-width: 0;
    padding: 0.75rem;
    color: var(--text-primary);
    background: var(--surface-inset);
    border: 1px solid var(--line);
    border-radius: 0.4rem;
    font-family: inherit;
    font-size: 0.8rem;
    font-weight: 500;

    &:disabled {
      opacity: 0.5;
    }
  }
}

.search-field {
  position: relative;
  min-width: 0;
}

// Sits exactly where the native placeholder would, so the swap between the two
// (examples vs. plain placeholder) is invisible.
.search-example {
  position: absolute;
  inset: 1px; // clear the input's border, so the text lines up with typed text
  padding: 0.75rem;
  display: flex;
  align-items: center;
  pointer-events: none;
  color: var(--text-muted);
  font-size: 0.8rem;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.example-enter-active,
.example-leave-active {
  transition: opacity 250ms ease, transform 250ms ease;
}

.example-enter-from,
.example-leave-to {
  opacity: 0;
  transform: translateY(0.35rem);
}

@media (prefers-reduced-motion: reduce) {
  .example-enter-active,
  .example-leave-active {
    transition: none;
  }

  .example-enter-from,
  .example-leave-to {
    transform: none;
  }
}

.cloud-row {
  gap: 0.6rem;
  cursor: pointer;

  &:hover {
    background: var(--surface-hover);
  }

  &-detail {
    min-width: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  &-version {
    flex-shrink: 0;
    font-size: 0.78rem;
    color: var(--text-muted);
  }

  strong {
    font-size: 0.95rem;
  }

  small {
    font-size: 0.78rem;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}
</style>

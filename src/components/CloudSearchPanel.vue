<script setup lang="ts">
import type { CloudTagMetadata } from '../types';

defineProps<{
  placeholder: string;
  results: CloudTagMetadata[];
  selected: Set<string>;
  allSelected: boolean;
  isWorking: boolean;
  disabled: boolean;
}>();

const query = defineModel<string>('query', { required: true });

defineEmits<{ search: []; toggle: [id: string]; 'toggle-all': [] }>();
</script>

<template>
  <div class="search-row">
    <input
      v-model="query"
      :placeholder="placeholder"
      :disabled="isWorking"
      @keyup.enter="$emit('search')"
    />
    <button
      class="btn btn-primary"
      :disabled="isWorking || !query.trim() || disabled"
      @click="$emit('search')"
    >
      Search
    </button>
  </div>

  <div v-if="results.length" class="tag-panel">
    <div class="tag-panel-header">
      <span class="tag-panel-label">Available Tags</span>
      <button class="small-btn" @click="$emit('toggle-all')">
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
  grid-template-columns: 1fr 7rem;
  gap: 0.5rem;

  input {
    min-width: 0;
    padding: 0.65rem;
    color: var(--text-primary);
    background: var(--surface-inset);
    border: 1px solid var(--line);
    border-radius: 0.4rem;
    font-family: inherit;

    &:disabled {
      opacity: 0.5;
    }
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
    color: var(--text-muted);
  }

  small {
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

.small-btn {
  background: none;
  border: 1px solid var(--line-subtle);
  color: var(--text-muted);
  font-size: 0.85em;
  padding: 0.25em 0.6em;
  border-radius: var(--radius-button);
  cursor: pointer;
  transition: color 0.15s, border-color 0.15s;

  &:hover {
    color: var(--text-primary);
    border-color: var(--accent);
  }
}
</style>

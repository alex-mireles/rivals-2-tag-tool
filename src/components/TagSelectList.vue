<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{ label: string; tagNames: readonly string[] }>();

const selected = defineModel<Set<string>>({ required: true });

const allSelected = computed(
  () => props.tagNames.length > 0 && selected.value.size === props.tagNames.length,
);

function toggle(name: string) {
  const next = new Set(selected.value);
  if (next.has(name)) next.delete(name);
  else next.add(name);
  selected.value = next;
}

function toggleAll() {
  selected.value = allSelected.value ? new Set() : new Set(props.tagNames);
}
</script>

<template>
  <div class="tag-panel">
    <div class="tag-panel-header">
      <span class="tag-panel-label">{{ label }}</span>
      <button class="select-all-btn" @click="toggleAll">
        {{ allSelected ? 'Deselect All' : 'Select All' }}
      </button>
    </div>
    <ul class="tag-list">
      <li
        v-for="name in tagNames"
        :key="name"
        class="tag-row tag-row--selectable"
        @click="toggle(name)"
      >
        <div class="tag-checkbox" :class="{ 'tag-checkbox--checked': selected.has(name) }">
          <v-icon v-if="selected.has(name)" name="md-check-round" scale="0.7" />
        </div>
        <span class="tag-name">{{ name }}</span>
      </li>
    </ul>
  </div>
</template>

<style scoped lang="scss">
.select-all-btn {
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

.tag-row {
  gap: 0.75em;

  &--selectable {
    cursor: pointer;

    &:hover {
      background: var(--surface-hover);
    }
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
</style>

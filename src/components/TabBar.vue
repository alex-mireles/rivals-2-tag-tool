<script setup lang="ts" generic="T extends string">
defineProps<{
  tabs: readonly { id: T; label: string }[];
  modelValue: T;
}>();

defineEmits<{ 'update:modelValue': [value: T] }>();
</script>

<template>
  <div class="tabs">
    <button
      v-for="tab in tabs"
      :key="tab.id"
      :class="{ active: modelValue === tab.id }"
      @click="$emit('update:modelValue', tab.id)"
    >
      {{ tab.label }}
    </button>
  </div>
</template>

<style scoped lang="scss">
.tabs {
  width: 100%;
  display: flex;
  gap: 0.35rem;

  button {
    flex: 1;
    border: 1px solid var(--line);
    border-radius: 0.4rem;
    padding: 0.45rem;
    background: var(--surface-inset);
    color: var(--text-muted);
    cursor: pointer;
    transition: color 0.15s, border-color 0.15s, background 0.15s;

    &.active {
      color: var(--text-primary);
      border-color: var(--accent);
      background: var(--accent-completed);
    }
  }
}
</style>

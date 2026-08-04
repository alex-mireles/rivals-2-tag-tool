<script setup lang="ts" generic="T extends string">
defineProps<{
  tabs: readonly { id: T; label: string; icon?: string }[];
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
      <v-icon v-if="tab.icon" :name="tab.icon" scale="0.8" />
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
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
    border: 1px solid var(--line);
    border-radius: 0.4rem;
    padding: 0.45rem;
    background: var(--surface-inset);
    color: var(--text-muted);
    cursor: pointer;
    transition: color 500ms, border-color 500ms, background 500ms, transform 500ms;

    &:hover {
      color: var(--text-primary);
      background: var(--surface-hover);
      transform: translateY(-0.15em);
    }

    &.active {
      color: var(--text-primary);
      border-color: var(--accent);
      background: var(--accent-completed);

      &:hover {
        background: var(--accent-completed);
        border-color: var(--accent-hover);
      }
    }
  }
}
</style>

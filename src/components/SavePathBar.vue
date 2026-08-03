<script setup lang="ts">
defineProps<{
  label: string;
  status?: 'idle' | 'error' | 'success';
}>();
</script>

<template>
  <div class="save-path-bar">
    <v-icon name="md-folderopen-round" scale="0.9" class="save-path-bar-icon" />

    <span class="save-path-bar-label" :class="`save-path-bar-label--${status ?? 'success'}`">{{ label }}</span>
    <!-- Reload / change controls; omitted on read-only bars. -->
    <slot name="actions" />
  </div>
</template>

<style scoped lang="scss">
.save-path-bar {
  width: 100%;
  display: flex;
  align-items: center;
  background: var(--surface-inset);
  border: 1px solid var(--line-subtle);
  border-radius: var(--radius-button);
  padding: 0.5em 0.85em;
  gap: 0.6em;

  &-icon {
    flex-shrink: 0;
    color: rgba(200, 180, 230, 0.35);
  }

  &-label {
    flex: 1;
    font-family: 'Ubuntu Sans Mono Variable', monospace;
    font-size: 0.7em;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;

    &--idle    { color: var(--text-muted); }
    &--error   { color: var(--text-failure); }
    &--success { color: var(--text-success); }
  }
}
</style>

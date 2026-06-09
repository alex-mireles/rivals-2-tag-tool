<script setup lang="ts">
defineProps<{
  label: string;
  status?: 'idle' | 'error' | 'success';
}>();
</script>

<template>
  <div class="save-path-bar">
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" class="save-path-bar-icon" aria-hidden="true">
      <path d="M2 2h5l1.5 2H14v10H2V2z" stroke="rgba(200,180,230,0.35)" stroke-width="1.5" />
    </svg>
    <span class="save-path-bar-label" :class="`save-path-bar-label--${status ?? 'success'}`">{{ label }}</span>
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
    opacity: 0.7;
  }

  &-label {
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

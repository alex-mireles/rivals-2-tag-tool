<script setup lang="ts">
import {
  SelectContent,
  SelectIcon,
  SelectItem,
  SelectItemIndicator,
  SelectItemText,
  SelectPortal,
  SelectRoot,
  SelectScrollDownButton,
  SelectScrollUpButton,
  SelectTrigger,
  SelectValue,
  SelectViewport,
} from 'reka-ui';

defineProps<{ options: readonly string[]; disabled?: boolean }>();

const model = defineModel<string>({ required: true });
</script>

<template>
  <SelectRoot v-model="model" :disabled="disabled || !options.length">
    <SelectTrigger class="tag-select-trigger">
      <SelectValue />
      <SelectIcon class="tag-select-chevron">
        <v-icon name="md-expandmore-round" scale="0.9" />
      </SelectIcon>
    </SelectTrigger>

    <SelectPortal>
      <SelectContent class="tag-select-content" position="popper" :side-offset="6">
        <SelectScrollUpButton class="tag-select-scroll">
          <v-icon name="md-expandless-round" scale="0.8" />
        </SelectScrollUpButton>

        <SelectViewport class="tag-select-viewport">
          <SelectItem
            v-for="name in options"
            :key="name"
            :value="name"
            class="tag-select-item"
          >
            <SelectItemText>{{ name }}</SelectItemText>
            <SelectItemIndicator class="tag-select-indicator">
              <v-icon name="md-check-round" scale="0.7" />
            </SelectItemIndicator>
          </SelectItem>
        </SelectViewport>

        <SelectScrollDownButton class="tag-select-scroll">
          <v-icon name="md-expandmore-round" scale="0.8" />
        </SelectScrollDownButton>
      </SelectContent>
    </SelectPortal>
  </SelectRoot>
</template>

<style scoped lang="scss">
// Outline only, transparent until hovered — deliberately muted so it doesn't
// compete with the tag name it holds. Font is left to inherit (same as
// .identity-name / .published-name) rather than forced to a monospace face.
.tag-select-trigger {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.8rem 0.9rem;
  color: var(--text-primary);
  font-size: 0.95rem;
  font-weight: 600;
  background: transparent;
  border: 1px solid var(--line);
  border-radius: var(--radius-button);
  cursor: pointer;
  transition: background 500ms, border-color 500ms;

  &:hover:not([data-disabled]) {
    background: var(--surface-hover);
  }

  &:focus-visible {
    outline: 2px solid rgba(99, 102, 241, 0.6);
    outline-offset: 2px;
  }

  &[data-disabled] {
    opacity: 0.4;
    cursor: not-allowed;
  }
}

.tag-select-chevron {
  display: flex;
  flex-shrink: 0;
  color: rgba(255, 255, 255, 0.85);
}

// Rendered in a Portal (teleported to <body>), so it escapes .card-content's
// overflow: hidden and can float above the rest of the window. Also outside
// this component's DOM tree, so scoped CSS's [data-v-xxx] attribute never
// reaches it — every rule touching it must be wrapped in :global().
:global(.tag-select-content) {
  width: var(--reka-select-trigger-width);
  max-height: min(var(--reka-select-content-available-height), 16rem);
  overflow: hidden;
  background: var(--color-bg);
  border: 1px solid var(--line);
  border-radius: var(--radius-panel);
  box-shadow: 0 0.5rem 1.5rem rgba(0, 0, 0, 0.4);
  z-index: 100;
}

:global(.tag-select-viewport) {
  padding: 0.3rem;
}

:global(.tag-select-scroll) {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0.2rem;
  color: var(--text-muted);
  background: var(--color-bg);
  cursor: default;
}

:global(.tag-select-item) {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.55rem 0.7rem;
  border-radius: var(--radius-button);
  font-size: 0.9rem;
  font-weight: 500;
  color: var(--text-primary);
  cursor: pointer;
  user-select: none;
  outline: none;
}

:global(.tag-select-item[data-highlighted]) {
  background: var(--surface-hover);
}

:global(.tag-select-item[data-state='checked']) {
  color: var(--text-primary);
  font-weight: 700;
}

:global(.tag-select-indicator) {
  display: flex;
  flex-shrink: 0;
  color: var(--accent-hover);
}
</style>

<script setup lang="ts">
import { computed } from 'vue';
import { useAppUpdate } from '../composables/useAppUpdate';

/**
 * Announces a newer release and, where the app can install one, does it.
 * Dismissible and non-blocking: importing one tag shouldn't require updating.
 */

const update = useAppUpdate();

const headline = computed(() => {
  switch (update.phase.value) {
    case 'downloading':
      return update.fraction.value === null
        ? `Downloading version ${update.info.value?.version}…`
        : `Downloading version ${update.info.value?.version} — ${Math.round(
            update.fraction.value * 100,
          )}%`;
    case 'installing':
      return 'Installing — the app will restart…';
    case 'failed':
      return update.error.value;
    default:
      return `Version ${update.info.value?.version} is available.`;
  }
});
</script>

<template>
  <div
    v-if="update.showNotice.value"
    class="update-notice"
    :class="{ 'update-notice--failed': update.phase.value === 'failed' }"
    role="status"
  >
    <div class="update-notice-body">
      <span class="update-notice-text">{{ headline }}</span>

      <!-- If we don't know the total size, show no bar rather than a fake one. -->
      <div
        v-if="update.phase.value === 'downloading' && update.fraction.value !== null"
        class="update-notice-progress"
        role="progressbar"
        aria-label="Update download progress"
        :aria-valuenow="Math.round(update.fraction.value * 100)"
        aria-valuemin="0"
        aria-valuemax="100"
      >
        <div class="update-notice-progress-fill" :style="{ width: `${update.fraction.value * 100}%` }" />
      </div>
    </div>

    <div v-if="!update.isBusy.value" class="update-notice-actions">
      <button
        v-if="update.info.value?.canSelfInstall"
        class="update-notice-action update-notice-action--primary"
        @click="update.install()"
      >
        <v-icon name="md-download-round" scale="0.75" />
        {{ update.phase.value === 'failed' ? 'Try again' : 'Update' }}
      </button>
      <button v-else class="update-notice-action" @click="update.openReleasePage()">
        <v-icon name="md-openinnew-round" scale="0.75" />
        Download
      </button>

      <button
        class="update-notice-action"
        aria-label="Dismiss the update notice"
        @click="update.dismiss()"
      >
        Later
      </button>
    </div>
  </div>
</template>

<style scoped lang="scss">
.update-notice {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.6rem;
  padding: 0.6em 0.8em;
  background: rgba(99, 102, 241, 0.1);
  border: 1px solid var(--accent);
  border-radius: var(--radius-button);

  &--failed {
    background: rgba(248, 113, 113, 0.08);
    border-color: rgba(248, 113, 113, 0.35);
  }

  &-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  &-text {
    font-size: 0.78rem;
    color: var(--text-primary);
  }

  &--failed &-text {
    color: var(--text-failure);
  }

  &-progress {
    height: 0.25rem;
    background: var(--surface-inset);
    border-radius: 0.125rem;
    overflow: hidden;

    &-fill {
      height: 100%;
      background: var(--accent);
      border-radius: inherit;
      transition: width 200ms ease;
    }
  }

  &-actions {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  &-action {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    border: 1px solid var(--line);
    background: var(--surface-hover);
    color: var(--text-primary);
    border-radius: 0.4rem;
    padding: 0.3rem 0.6rem;
    font-size: 0.75rem;
    cursor: pointer;
    transition: border-color 500ms, background 500ms;

    &:hover {
      border-color: var(--accent);
    }

    &--primary {
      background: var(--accent);

      &:hover {
        background: var(--accent-hover);
      }
    }
  }
}
</style>

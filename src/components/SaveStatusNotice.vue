<script setup lang="ts">
import { computed } from 'vue';
import { useSaveFile } from '../composables/useSaveFile';

/**
 * Explains a non-ready save file and offers the matching recovery action.
 * `context` tailors the headline: on the get/share screens the save is a
 * prerequisite for one action, not for the screen as a whole.
 */
const props = defineProps<{ context?: 'home' | 'download' | 'import' | 'share' }>();

const save = useSaveFile();

const detail = computed(() => {
  switch (save.status.value) {
    case 'missing':
      return save.source.value === 'saved'
        ? { text: 'The save file you chose is no longer there.', action: 'reset' as const }
        : {
            text:
              props.context === 'download'
                ? 'No Rivals 2 save found on this PC — you can still download tags and save them as a .r2pack.'
                : 'No Rivals 2 save found in the usual location.',
            action: 'choose' as const,
          };
    case 'unreadable':
      return {
        text: 'Couldn’t read the save file. Is Rivals 2 running?',
        action: 'retry' as const,
      };
    case 'unsupported':
      return {
        text: 'This doesn’t look like a Rivals 2 tag save.',
        action: 'choose' as const,
      };
    default:
      return null;
  }
});
</script>

<template>
  <div v-if="detail" class="save-notice">
    <span class="save-notice-text">{{ detail.text }}</span>
    <button v-if="detail.action === 'retry'" class="save-notice-action" @click="save.reload()">
      <v-icon name="md-refresh-round" scale="0.75" />
      Retry
    </button>
    <button
      v-else-if="detail.action === 'reset'"
      class="save-notice-action"
      @click="save.resetToDefault()"
    >
      <v-icon name="md-restartalt-round" scale="0.75" />
      Reset to default
    </button>
    <button v-else class="save-notice-action" @click="save.choose()">
      <v-icon name="md-folderopen-round" scale="0.75" />
      Choose a save file…
    </button>
  </div>
</template>

<style scoped lang="scss">
.save-notice {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.6rem;
  padding: 0.6em 0.8em;
  background: rgba(250, 204, 21, 0.08);
  border: 1px solid rgba(250, 204, 21, 0.25);
  border-radius: var(--radius-button);

  &-text {
    flex: 1;
    min-width: 0;
    font-size: 0.78rem;
    color: var(--text-warning);
  }

  &-action {
    flex-shrink: 0;
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
    transition: border-color 500ms;

    &:hover {
      border-color: var(--accent);
    }
  }
}
</style>

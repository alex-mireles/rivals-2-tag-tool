<script setup lang="ts">
import { computed } from 'vue';
import AnimatedCard from '../components/AnimatedCard.vue';
import SavePathBar from '../components/SavePathBar.vue';
import SaveStatusNotice from '../components/SaveStatusNotice.vue';
import { EXPECTED_SAVE_FILE_NAME, useSaveFile } from '../composables/useSaveFile';

const appVersion = APP_VERSION;

defineEmits<{ navigate: [view: 'get' | 'share'] }>();

const save = useSaveFile();

const isResolving = computed(() => save.status.value === 'resolving');

const savePathDisplay = computed(() => {
  if (isResolving.value) return 'Looking for your save file…';
  if (!save.path.value) return `${EXPECTED_SAVE_FILE_NAME} not found`;
  return save.path.value;
});

const savePathStatus = computed(() => {
  if (isResolving.value) return 'idle' as const;
  if (save.canWriteSave.value) return 'success' as const;
  return 'error' as const;
});
</script>

<template>
  <AnimatedCard>
    <div class="card-header">
      <h1 class="app-title">Rivals II Tag Tool</h1>
      <span class="app-version">v{{ appVersion }}</span>
    </div>

    <SavePathBar :label="savePathDisplay" :status="savePathStatus">
      <template #actions>
        <button
          class="path-action"
          title="Reload tags from the save file"
          :disabled="isResolving"
          @click="save.reload()"
        >
          <v-icon name="md-refresh-round" scale="0.8" />
        </button>
        <button class="path-action" title="Choose a different save file" @click="save.choose()">
          <v-icon name="md-fileopen-round" scale="0.8" />
        </button>
      </template>
    </SavePathBar>

    <SaveStatusNotice context="home" />

    <Transition name="content-swap" mode="out-in">
      <div v-if="isResolving" key="loading" class="loading-panel">Reading save file...</div>

      <div v-else key="panel" class="tag-panel">
        <div class="tag-panel-header">
          <span class="tag-panel-label">Player Tags</span>
          <Transition name="tag-count-fade">
            <span v-if="save.canWriteSave.value" class="tag-panel-count">
              {{ save.tagNames.value.length }} tags found
            </span>
          </Transition>
        </div>

        <Transition name="expand" mode="out-in">
          <template v-if="save.canWriteSave.value">
            <div v-if="!save.hasTags.value" class="tag-panel-empty">
              <span class="tag-panel-empty-message">No custom tags found in file</span>
            </div>
            <ul v-else class="tag-list">
              <li v-for="name in save.tagNames.value" :key="name" class="tag-row">
                <span class="tag-name">{{ name }}</span>
              </li>
            </ul>
          </template>

          <div v-else class="tag-panel-empty">
            <span class="tag-panel-empty-message">no player tags currently loaded</span>
          </div>
        </Transition>
      </div>
    </Transition>

    <Transition name="fade">
      <div v-if="!isResolving" class="action-row">
        <button class="btn btn-primary" @click="$emit('navigate', 'get')">
          <v-icon name="md-download-round" scale="0.85" />
          Get Tags
        </button>
        <button class="btn btn-primary" @click="$emit('navigate', 'share')">
          <v-icon name="md-upload-round" scale="0.85" />
          Share Tags
        </button>
      </div>
    </Transition>
  </AnimatedCard>
</template>

<style scoped lang="scss">
.card-header {
  text-align: center;
}

.app-title {
  font-size: 2em;
  font-weight: 800;
}

.app-version {
  font-size: 0.75em;
  letter-spacing: 0.1em;
  color: var(--text-muted);
}

.path-action {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 1.6rem;
  height: 1.6rem;
  background: none;
  border: none;
  border-radius: var(--radius-button);
  color: var(--text-muted);
  cursor: pointer;
  transition: color 0.15s, background 0.15s;

  &:hover:not(:disabled) {
    color: var(--text-primary);
    background: var(--surface-hover);
  }

  &:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
}

.tag-panel {
  min-height: 8rem;

  &-header {
    padding-bottom: 0.2em;
    margin-bottom: 0;
  }

  &-count {
    font-size: 1em;
    font-weight: 600;
    color: var(--text-success);
    background: rgba(99, 102, 241, 0.15);
    border-radius: 0.5em;
    padding: 0.4em 0.8em;
  }

  &-empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;

    &-message {
      font-size: 1em;
      color: var(--text-muted);
    }
  }
}

.tag {
  &-list {
    max-height: 10rem;
  }

  &-row {
    justify-content: center;
    padding: 0.5em;
  }
}

.action-row {
  width: 100%;
  display: flex;
  gap: 0.625rem;

  .btn {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5em;
  }
}

// Opacity only — the card itself animates the height change.
.expand-enter-active,
.expand-leave-active {
  transition: opacity 0.25s ease;
}

.expand-enter-from,
.expand-leave-to {
  opacity: 0;
}

.tag-count-fade-enter-active {
  transition: opacity 0.3s ease;
  transition-delay: 0.5s;
}

.tag-count-fade-leave-active {
  transition: opacity 0.3s ease;
}

.tag-count-fade-enter-from,
.tag-count-fade-leave-to {
  opacity: 0;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>

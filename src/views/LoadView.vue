<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

const EXPECTED_SAVE_FILE_NAME = 'Rivals2_PlayerTagSaveSlot.sav';
const appVersion = APP_VERSION;

interface LoadViewState {
  savePath: string;
  savePathError: boolean;
  tagNames: string[];
  hasLoaded: boolean;
}

const props = defineProps<{ initialState: LoadViewState }>();

const emit = defineEmits<{
  navigate: [view: 'export' | 'import'];
  stateChange: [state: LoadViewState];
}>();

const errorMsg = ref('');
const tagNames = ref<string[]>(props.initialState.tagNames);
const hasLoaded = ref(props.initialState.hasLoaded);
const isLoading = ref(false);

const savePath = ref(props.initialState.savePath);
const savePathError = ref(props.initialState.savePathError);

watch([savePath, savePathError, tagNames, hasLoaded], () => {
  emit('stateChange', {
    savePath: savePath.value,
    savePathError: savePathError.value,
    tagNames: tagNames.value,
    hasLoaded: hasLoaded.value,
  });
});

function clearTagNames() {
  tagNames.value = [];
  hasLoaded.value = false;
}

async function chooseSaveFile() {
  const defaultPath = await invoke<string>('get_default_save_path');

  const filePath = await open({
    multiple: false,
    title: 'Choose a Save File',
    filters: [{ name: '.sav file', extensions: ['sav'] }],
    ...(defaultPath ? { defaultPath } : {}),
  });

  if (!filePath) return;

  clearTagNames();

  const fileName = filePath.split(/[\\/]/).pop() ?? '';

  if (fileName !== EXPECTED_SAVE_FILE_NAME) {
    savePath.value = filePath;
    savePathError.value = true;
    return;
  }

  savePathError.value = false;
  savePath.value = filePath;
}

const savePathDisplay = computed(() => {
  if (!savePath.value) return `${EXPECTED_SAVE_FILE_NAME} not selected`;
  if (savePathError.value) return `Expected ${EXPECTED_SAVE_FILE_NAME}`;
  return savePath.value;
});

async function loadTagNames() {
  errorMsg.value = '';
  tagNames.value = [];
  hasLoaded.value = false;
  isLoading.value = true;
  await nextTick();

  try {
    tagNames.value = await invoke<string[]>('get_tag_names', {
      savePath: savePath.value,
    });
    hasLoaded.value = true;
  } catch (error) {
    errorMsg.value = String(error);
  } finally {
    isLoading.value = false;
  }
}
</script>

<template>
  <div class="card">
    <div class="card-header">
      <h1 class="app-title">Rivals II Tag Tool</h1>
      <span class="app-version">v{{ appVersion }}</span>
    </div>

    <div class="save-path-row">
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none" class="save-path-icon">
        <path d="M2 2h5l1.5 2H14v10H2V2z" stroke="rgba(200,180,230,0.35)" stroke-width="1.5" />
      </svg>
      <div
        class="save-path-label"
        :class="{ 'save-path-label-error': savePathError, 'save-path-label-success': savePath && !savePathError }"
      >
        {{ savePathDisplay }}
      </div>
    </div>

    <div class="button-row">
      <button
        class="btn btn-primary"
        :class="{ 'btn-primary-muted': savePath && !savePathError }"
        :disabled="isLoading"
        @click="chooseSaveFile"
      >
        Choose a Save File
      </button>

      <Transition name="slide-in">
        <div v-if="savePath && !savePathError" class="load-tags-wrapper">
          <button
            class="btn btn-primary"
            :class="{ 'btn-primary-muted': hasLoaded }"
            :disabled="isLoading"
            @click="loadTagNames"
          >
            Load Tags
          </button>
        </div>
      </Transition>
    </div>

    <div v-if="isLoading" class="loading-panel">Reading save file...</div>

    <div v-else class="tag-panel">
      <div class="tag-panel-header">
        <span class="tag-panel-label">Player Tags</span>
        <Transition name="tag-count-fade">
          <span v-if="hasLoaded" class="tag-panel-count">{{ tagNames.length }} tags found</span>
        </Transition>
      </div>

      <Transition name="expand" mode="out-in">
        <template v-if="hasLoaded">
          <div v-if="tagNames.length === 0" class="tag-panel-empty">
            <span class="tag-panel-empty-message">No custom tags found in file</span>
          </div>
          <ul v-else class="tag-list">
            <li v-for="name in tagNames" :key="name" class="tag-row">
              <span class="tag-name">{{ name }}</span>
            </li>
          </ul>
        </template>

        <div v-else class="tag-panel-empty">
          <span class="tag-panel-empty-message">no player tags currently loaded</span>
        </div>
      </Transition>
    </div>

    <Transition name="fade">
      <div v-if="hasLoaded && tagNames.length !== 0" class="action-row">
        <button class="btn btn-primary" @click="emit('navigate', 'import')">
          Import or Overwrite Tags
        </button>
        <button class="btn btn-primary" @click="emit('navigate', 'export')">
          Export Tags
        </button>
      </div>
    </Transition>
  </div>
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

.save-path-row {
  width: 100%;
  display: flex;
  align-items: center;
  background: var(--surface-inset);
  border: 1px solid var(--line-subtle);
  border-radius: var(--radius-button);
  padding: 0.7em 1em;
  gap: 0.7em;
}

.save-path-icon {
  flex-shrink: 0;
}

.save-path-label {
  font-family: 'Source Code Pro', monospace;
  font-size: 0.9em;
  min-width: 0;
  color: var(--text-muted);
  word-break: break-all;

  &-success {
    color: var(--text-success);
  }

  &-error {
    color: var(--text-failure);
  }
}

.tag-panel {
  display: flex;
  flex-direction: column;
  width: 100%;
  font-size: 0.75rem;
  padding: 0.75rem 1rem 0.25rem;
  min-height: 8rem;
  background: var(--surface-inset);
  border: 1px solid var(--line-subtle);
  border-radius: var(--radius-panel);

  &-header {
    flex-shrink: 0;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--line-divider);
    padding-bottom: 0.2em;
    min-height: 2.5em;
  }

  &-label {
    font-size: 1em;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    font-weight: 400;
    color: var(--text-muted);
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
    list-style: none;
    max-height: 10rem;
    overflow-y: auto;
  }

  &-row {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0.5em;
    border-bottom: 1px solid var(--line-divider);

    &:last-child {
      border-bottom: none;
    }
  }

}

.action-row {
  width: 100%;
  display: flex;
  gap: 0.625rem;

  .btn {
    flex: 1;
  }
}

.button-row {
  width: 100%;
  display: flex;
  align-items: center;

  .btn {
    flex: 1;
  }
}

.load-tags-wrapper {
  flex: 1;
  min-width: 0;
  margin-left: 0.625rem;

  .btn {
    width: 100%;
    overflow: hidden;
    white-space: nowrap;
  }
}

.expand-enter-active,
.expand-leave-active {
  transition: opacity 0.3s ease, max-height 0.5s ease;
  overflow: hidden;
  flex: 1;
}

.expand-enter-from,
.expand-leave-to {
  opacity: 0;
  max-height: 0;
}

.expand-enter-to,
.expand-leave-from {
  opacity: 1;
  max-height: 10rem;
  flex: 1;
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

.slide-in-enter-active {
  transition: opacity 0.3s ease 0.15s, max-width 0.5s ease, margin-left 0.5s ease;
}

.slide-in-leave-active {
  transition: opacity 0.3s ease, max-width 0.5s ease 0.1s, margin-left 0.5s ease 0.1s;
}

.slide-in-enter-from,
.slide-in-leave-to {
  opacity: 0;
  max-width: 0;
  margin-left: 0;
}

.slide-in-enter-to,
.slide-in-leave-from {
  opacity: 1;
  max-width: 300px;
  margin-left: 0.625rem;
}

.fade-enter-active {
  transition: opacity 0.3s ease, max-height 0.5s ease, margin-top 0.5s ease, padding-top 0.5s ease;
  overflow: hidden;
}

.fade-leave-active {
  transition: opacity 0.3s ease, max-height 0.5s ease, margin-top 0.5s ease, padding-top 0.5s ease;
  overflow: hidden;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
  max-height: 0;
  margin-top: 0;
  padding-top: 0;
}

.fade-enter-to,
.fade-leave-from {
  opacity: 1;
  max-height: 3rem;
}
</style>

<script setup lang="ts">
import { ref } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import HomeView from './views/HomeView.vue';
import ExportView from './views/ExportView.vue';
import ShareView from './views/ShareView.vue';
import type { SaveFileState } from './types';

const appWindow = getCurrentWindow();

// Home does the everyday work (see your tags, get new ones). Sharing and
// exporting to file are real but secondary, so they stay as their own screens
// reached from a tag's row rather than as top-level choices.
type ViewName = 'home' | 'share' | 'export';

const currentView = ref<ViewName>('home');
const transitionName = ref('slide-forward');

const saveFileState = ref<SaveFileState>({
  savePath: '',
  savePathError: false,
  tagNames: [],
  hasLoaded: false,
});

function navigateTo(view: ViewName) {
  transitionName.value = 'slide-forward';
  currentView.value = view;
}

function goBack() {
  transitionName.value = 'slide-back';
  currentView.value = 'home';
}
</script>

<template>
  <div class="titlebar">
    <div data-tauri-drag-region></div>
    <div class="controls">
      <button @click="appWindow.close()">
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24">
          <path fill="currentColor" d="M13.46 12L19 17.54V19h-1.46L12 13.46L6.46 19H5v-1.46L10.54 12L5 6.46V5h1.46L12 10.54L17.54 5H19v1.46z"/>
        </svg>
      </button>
    </div>
  </div>

  <div class="bg" aria-hidden="true">
    <div class="bloom bloom--a"></div>
    <div class="bloom bloom--b"></div>
  </div>

  <div class="viewport">
    <Transition :name="transitionName" mode="out-in">
      <HomeView
        v-if="currentView === 'home'"
        key="home"
        @state-change="(s: SaveFileState) => (saveFileState = s)"
        @share="navigateTo('share')"
        @export="navigateTo('export')"
      />
      <ShareView
        v-else-if="currentView === 'share'"
        key="share"
        :save-path="saveFileState.savePath"
        :tag-names="saveFileState.tagNames"
        @go-back="goBack"
      />
      <ExportView
        v-else
        key="export"
        :save-path="saveFileState.savePath"
        :tag-names="saveFileState.tagNames"
        @go-back="goBack"
      />
    </Transition>
  </div>
</template>

<style scoped lang="scss">
.slide-forward-enter-active,
.slide-forward-leave-active,
.slide-back-enter-active,
.slide-back-leave-active {
  transition: opacity 0.25s ease, transform 0.3s ease;
}

.slide-forward-enter-from {
  opacity: 0;
  transform: translateX(40px);
}

.slide-forward-leave-to {
  opacity: 0;
  transform: translateX(-40px);
}

.slide-back-enter-from {
  opacity: 0;
  transform: translateX(-40px);
}

.slide-back-leave-to {
  opacity: 0;
  transform: translateX(40px);
}
</style>

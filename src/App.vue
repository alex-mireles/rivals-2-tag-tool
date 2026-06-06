<script setup lang="ts">
import { ref } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import LoadView from './views/LoadView.vue';
import ExportView from './views/ExportView.vue';
import ImportView from './views/ImportView.vue';

const appWindow = getCurrentWindow();

type ViewName = 'load' | 'export' | 'import';

const currentView = ref<ViewName>('load');
const transitionName = ref('slide-forward');

interface LoadViewState {
  savePath: string;
  savePathError: boolean;
  tagNames: string[];
  hasLoaded: boolean;
}

const loadViewState = ref<LoadViewState>({
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
  currentView.value = 'load';
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
      <LoadView
        v-if="currentView === 'load'"
        key="load"
        :initial-state="loadViewState"
        @state-change="(s: LoadViewState) => (loadViewState = s)"
        @navigate="navigateTo"
      />
      <ExportView
        v-else-if="currentView === 'export'"
        key="export"
        :save-path="loadViewState.savePath"
        :tag-names="loadViewState.tagNames"
        @go-back="goBack"
      />
      <ImportView
        v-else
        key="import"
        :save-path="loadViewState.savePath"
        :tag-names="loadViewState.tagNames"
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

.slide-forward-enter-to,
.slide-forward-leave-from,
.slide-back-enter-to,
.slide-back-leave-from {
  opacity: 1;
  transform: translateX(0);
}
</style>

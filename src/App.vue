<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import HomeView from './views/HomeView.vue';
import GetTagsView from './views/GetTagsView.vue';
import ShareTagsView from './views/ShareTagsView.vue';
import { useSaveFile } from './composables/useSaveFile';
import { useAppUpdate } from './composables/useAppUpdate';

const appWindow = getCurrentWindow();

type ViewName = 'home' | 'get' | 'share';

const currentView = ref<ViewName>('home');
const transitionName = ref('slide-forward');

const save = useSaveFile();
const update = useAppUpdate();

onMounted(() => {
  // Resolve and read the save up front — the user shouldn't have to point the
  // app at a file that lives in a fixed, known location.
  void save.reload();
  void invoke('cleanup_stale_cloud_files').catch(() => undefined);
  // Nothing waits on this, and it handles its own errors.
  void update.check();
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
      <button aria-label="Close the app" @click="appWindow.close()">
        <v-icon name="md-close-round" scale="0.85" />
      </button>
    </div>
  </div>

  <div class="bg" aria-hidden="true">
    <div class="bloom bloom--a"></div>
    <div class="bloom bloom--b"></div>
  </div>

  <div class="viewport">
    <Transition :name="transitionName" mode="out-in">
      <HomeView v-if="currentView === 'home'" key="home" @navigate="navigateTo" />
      <GetTagsView v-else-if="currentView === 'get'" key="get" @go-back="goBack" />
      <ShareTagsView v-else key="share" @go-back="goBack" />
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

<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window';
import HomeView from './views/HomeView.vue';

const appWindow = getCurrentWindow();

// Everything happens on one screen now — submitting your tags and installing
// other people's are two columns, not two destinations — so there is no view
// switching left to do.
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
    <HomeView />
  </div>
</template>

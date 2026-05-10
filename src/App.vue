<script setup lang="ts">
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const appVersion = APP_VERSION;
const tagNames = ref<string[]>([]);
const errorMsg = ref('');
const hasLoaded = ref(false);

const TEST_SAVE_PATH =
  '/home/hyper/code/projects/rivals-2-tag-tool/test-data/Rivals2_PlayerTagSaveSlot.sav';

async function loadTagNames() {
  errorMsg.value = '';
  tagNames.value = [];
  hasLoaded.value = false;

  try {
    tagNames.value = await invoke<string[]>('get_tag_names', {
      savePath: TEST_SAVE_PATH,
    });
    hasLoaded.value = true;
  } catch (error) {
    errorMsg.value = String(error);
  }
}
</script>

<template>
  <main class="container">
    <h1>Rivals 2 Tag Tool</h1>
    <p>Version {{ appVersion }}</p>

    <button @click="loadTagNames">Load Tag Names</button>

    <p v-if="errorMsg" class="error">Error: {{ errorMsg }}</p>

    <div v-if="hasLoaded">
      <p>Found {{ tagNames.length }} tag(s):</p>
      <ul>
        <li v-for="name in tagNames" :key="name">{{ name }}</li>
      </ul>
    </div>
  </main>
</template>

<style scoped>
.error {
  color: #ff6b6b;
}

ul {
  list-style: none;
  padding: 0;
}

li {
  padding: 0.3em 0;
}
</style>

<style>
:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 16px;
  line-height: 24px;
  font-weight: 400;

  color: #f6f6f6;
  background-color: #2f2f2f;

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

.container {
  margin: 0;
  padding-top: 10vh;
  display: flex;
  flex-direction: column;
  justify-content: center;
  text-align: center;
}

button {
  border-radius: 8px;
  border: 1px solid transparent;
  padding: 0.6em 1.2em;
  font-size: 1em;
  font-weight: 500;
  font-family: inherit;
  color: #ffffff;
  background-color: #0f0f0f98;
  transition: border-color 0.25s;
  box-shadow: 0 2px 2px rgba(0, 0, 0, 0.2);
  cursor: pointer;
}

button:hover {
  border-color: #396cd8;
}

button:active {
  border-color: #396cd8;
  background-color: #0f0f0f69;
}

button {
  outline: none;
}

h1 {
  text-align: center;
}
</style>

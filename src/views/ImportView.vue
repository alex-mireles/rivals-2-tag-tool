<script setup lang="ts">
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import AnimatedCard from '../components/AnimatedCard.vue';
import ImportReview from '../components/ImportReview.vue';
import SavePathBar from '../components/SavePathBar.vue';
import ViewHeader from '../components/ViewHeader.vue';
import type { PreviewResult, TagPreview } from '../types';

const props = defineProps<{ savePath: string; tagNames: string[] }>();
const emit = defineEmits<{ 'go-back': []; 'tags-changed': [names: string[]] }>();
const previews = ref<TagPreview[]>([]);
const saveVersion = ref<number | null>(null);
const isLoading = ref(false);
const errorMsg = ref('');

async function chooseFiles() {
  errorMsg.value = '';
  isLoading.value = true;
  try {
    const paths = await open({ multiple: true, title: 'Choose .r2tag Files', filters: [{ name: 'Tag file', extensions: ['r2tag'] }] });
    if (!paths || (Array.isArray(paths) && paths.length === 0)) return;
    const result = await invoke<PreviewResult>('get_tag_previews', {
      r2tagPaths: Array.isArray(paths) ? paths : [paths], savePath: props.savePath,
    });
    previews.value = result.previews;
    saveVersion.value = result.save_version;
  } catch (error) {
    errorMsg.value = String(error);
  } finally {
    isLoading.value = false;
  }
}

async function finished() {
  emit('tags-changed', await invoke<string[]>('get_tag_names', { savePath: props.savePath }));
}

function reset() { previews.value = []; saveVersion.value = null; errorMsg.value = ''; }
</script>

<template>
  <AnimatedCard>
    <ViewHeader title="Import Tags" @go-back="emit('go-back')" />
    <SavePathBar :label="savePath" />
    <div v-if="isLoading" class="loading-panel">Reading tag files...</div>
    <ImportReview v-else-if="previews.length" :save-path="savePath" :tag-names="tagNames" :previews="previews" :save-version="saveVersion" @reset="reset" @finished="finished" />
    <div v-else class="view-stack">
      <button class="btn btn-primary" @click="chooseFiles">Choose .r2tag Files</button>
      <p class="empty-hint">Choose one or more <code>.r2tag</code> files to import into your save.</p>
      <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>
    </div>
  </AnimatedCard>
</template>

<style scoped>.empty-hint { color: var(--text-muted); font-size: .85rem; text-align: center; }</style>

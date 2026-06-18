<script setup lang="ts">
import { ref } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import AnimatedCard from '../components/AnimatedCard.vue';
import SavePathBar from '../components/SavePathBar.vue';
import ViewHeader from '../components/ViewHeader.vue';
import TagImportPanel from '../components/TagImportPanel.vue';

defineProps<{
  savePath: string;
  tagNames: string[];
}>();

const emit = defineEmits<{
  'go-back': [];
}>();

const paths = ref<string[]>([]);
const errorMsg = ref('');

async function chooseFiles() {
  errorMsg.value = '';
  try {
    const picked = await open({
      multiple: true,
      title: 'Choose .r2tag Files',
      filters: [{ name: 'Tag file', extensions: ['r2tag'] }],
    });
    if (!picked || (Array.isArray(picked) && picked.length === 0)) return;
    paths.value = Array.isArray(picked) ? picked : [picked];
  } catch (err) {
    errorMsg.value = String(err);
  }
}
</script>

<template>
  <AnimatedCard>
    <ViewHeader title="Import Tags" @go-back="emit('go-back')" />

    <SavePathBar :label="savePath" />

    <div class="view-stack">
      <button class="btn btn-primary" @click="chooseFiles">Choose .r2tag Files</button>

      <div v-if="paths.length === 0 && !errorMsg" class="empty-hint">
        Choose one or more <code>.r2tag</code> files to import into your save.
      </div>

      <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>

      <TagImportPanel
        :save-path="savePath"
        :existing-tag-names="tagNames"
        :paths="paths"
        @restart="paths = []"
      />
    </div>
  </AnimatedCard>
</template>

<style scoped lang="scss">
.empty-hint {
  font-size: 0.85em;
  color: var(--text-muted);
  text-align: center;
  padding: 1em;

  code {
    font-family: 'Ubuntu Sans Mono Variable', monospace;
    background: var(--surface-inset);
    padding: 0.1em 0.3em;
    border-radius: 3px;
  }
}
</style>

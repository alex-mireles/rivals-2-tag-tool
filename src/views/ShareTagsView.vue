<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import AnimatedCard from '../components/AnimatedCard.vue';
import SaveStatusNotice from '../components/SaveStatusNotice.vue';
import TabBar from '../components/TabBar.vue';
import TagSelectList from '../components/TagSelectList.vue';
import ViewHeader from '../components/ViewHeader.vue';
import { apiBaseUrl, cloudConfigured, CLOUD_UNCONFIGURED_MESSAGE } from '../cloud';
import { useCloudAuth } from '../composables/useCloudAuth';
import { useSaveFile } from '../composables/useSaveFile';
import type { CloudTagMetadata, PackSummary } from '../types';

/**
 * Everything that sends tags *off* this PC. Publishing to the cloud is the
 * default; exporting to files covers setups with no internet.
 *
 * The two halves are deliberately asymmetric: the cloud holds exactly one tag
 * per start.gg user, while a local export can carry any number.
 */

type TabName = 'publish' | 'export';
type ExportTarget = 'folder' | 'pack';

const TABS = [
  { id: 'publish', label: 'Publish to Cloud' },
  { id: 'export', label: 'Export to Files' },
] as const satisfies readonly { id: TabName; label: string }[];

const emit = defineEmits<{ 'go-back': [] }>();

const save = useSaveFile();
const auth = useCloudAuth();

const tab = ref<TabName>('publish');
const errorMsg = ref(cloudConfigured ? '' : CLOUD_UNCONFIGURED_MESSAGE);
const progress = ref('');
const isWorking = ref(false);
const confirmingDelete = ref(false);

const uploadTagName = ref(save.tagNames.value[0] ?? '');
const exportTarget = ref<ExportTarget>('folder');
const selected = ref<Set<string>>(new Set());
const exportResult = ref<{ count: number; location: string; isPack: boolean } | null>(null);

// The save can be reloaded from another screen; keep the picker on a real tag.
watch(
  save.tagNames,
  (names) => {
    if (!names.includes(uploadTagName.value)) uploadTagName.value = names[0] ?? '';
    selected.value = new Set([...selected.value].filter((name) => names.includes(name)));
  },
  { immediate: true },
);

const canPublish = computed(
  () => cloudConfigured && save.canWriteSave.value && save.hasTags.value,
);

function switchTab(next: TabName) {
  tab.value = next;
  errorMsg.value = cloudConfigured ? '' : CLOUD_UNCONFIGURED_MESSAGE;
  progress.value = '';
  exportResult.value = null;
  confirmingDelete.value = false;
}

async function signIn() {
  errorMsg.value = '';
  isWorking.value = true;
  try {
    await auth.signIn((message) => (progress.value = message));
  } catch (error) {
    errorMsg.value = String(error);
    progress.value = '';
  } finally {
    isWorking.value = false;
  }
}

async function uploadTag() {
  if (!auth.sessionToken.value || !uploadTagName.value) return;
  errorMsg.value = '';
  isWorking.value = true;
  try {
    auth.publishedTag.value = await invoke<CloudTagMetadata>('cloud_upload_tag', {
      apiBaseUrl,
      sessionToken: auth.sessionToken.value,
      savePath: save.path.value,
      tagName: uploadTagName.value,
    });
    progress.value = 'Cloud tag published successfully.';
  } catch (error) {
    errorMsg.value = String(error);
  } finally {
    isWorking.value = false;
  }
}

async function deleteTag() {
  if (!auth.sessionToken.value || !auth.publishedTag.value) return;
  confirmingDelete.value = false;
  isWorking.value = true;
  try {
    await invoke('cloud_delete_tag', { apiBaseUrl, sessionToken: auth.sessionToken.value });
    auth.publishedTag.value = null;
    progress.value = 'Cloud tag removed.';
  } catch (error) {
    errorMsg.value = String(error);
  } finally {
    isWorking.value = false;
  }
}

async function exportSelected() {
  if (!selected.value.size) return;
  errorMsg.value = '';
  exportResult.value = null;
  const tagNames = [...selected.value];

  // The destination toggle picks both the dialog and the command.
  const isPack = exportTarget.value === 'pack';
  const destination = isPack
    ? await saveDialog({
        title: 'Save Tag Pack',
        defaultPath: 'my-tags.r2pack',
        filters: [{ name: 'Tag pack', extensions: ['r2pack'] }],
      })
    : await open({ directory: true, title: 'Choose Export Folder' });
  if (!destination) return;

  isWorking.value = true;
  try {
    if (isPack) {
      const summary = await invoke<PackSummary>('pack_tags_from_save', {
        savePath: save.path.value,
        tagNames,
        outputPath: destination,
        label: null,
      });
      exportResult.value = { count: summary.entryCount, location: summary.outputPath, isPack };
    } else {
      const written = await invoke<string[]>('export_tags', {
        savePath: save.path.value,
        tagNames,
        outputDir: destination,
      });
      exportResult.value = { count: written.length, location: destination, isPack };
    }
    selected.value = new Set();
  } catch (error) {
    errorMsg.value = String(error);
  } finally {
    isWorking.value = false;
  }
}
</script>

<template>
  <AnimatedCard>
    <ViewHeader title="Share Tags" @go-back="emit('go-back')" />

    <TabBar :tabs="TABS" :model-value="tab" @update:model-value="switchTab" />

    <SaveStatusNotice v-if="!save.canWriteSave.value" context="share" />

    <!-- Publish to Cloud -->
    <div v-if="tab === 'publish'" class="view-stack">
      <div v-if="isWorking" class="loading-panel">Working with the cloud service…</div>

      <template v-else>
        <button
          v-if="!auth.signedInUser.value"
          class="btn btn-primary"
          :disabled="!cloudConfigured"
          @click="signIn"
        >
          Sign in with start.gg
        </button>

        <template v-else>
          <div class="identity">
            <strong>{{ auth.signedInUser.value.gamerTag }}</strong>
            <span>{{ auth.signedInUser.value.slug }}</span>
            <button class="link-btn" @click="auth.signOut()">Sign out</button>
          </div>

          <div v-if="auth.publishedTag.value" class="published">
            <span>Published tag</span>
            <strong>{{ auth.publishedTag.value.tagName }}</strong>
            <small>Updated {{ new Date(auth.publishedTag.value.updatedAt).toLocaleString() }}</small>
          </div>

          <label class="upload-label">
            Tag from loaded save
            <select v-model="uploadTagName" :disabled="!save.hasTags.value">
              <option v-for="name in save.tagNames.value" :key="name" :value="name">
                {{ name }}
              </option>
            </select>
          </label>

          <p class="disclosure">
            Publishing makes your start.gg gamer tag, profile slug, in-game tag name, and controls
            file publicly downloadable, and may be saved offline by others.
          </p>

          <button class="btn btn-primary" :disabled="!canPublish || !uploadTagName" @click="uploadTag">
            {{ auth.publishedTag.value ? 'Replace Published Tag' : 'Publish Tag' }}
          </button>

          <template v-if="auth.publishedTag.value">
            <div v-if="confirmingDelete" class="confirm">
              <span class="confirm-text">Remove your published cloud tag?</span>
              <button class="confirm-btn" @click="confirmingDelete = false">Cancel</button>
              <button class="confirm-btn confirm-btn--danger" @click="deleteTag">Remove</button>
            </div>
            <button v-else class="danger-btn" @click="confirmingDelete = true">
              Delete Published Tag
            </button>
          </template>
        </template>

        <p v-if="progress" class="hint">{{ progress }}</p>
      </template>
    </div>

    <!-- Export to Files -->
    <div v-else class="view-stack">
      <div v-if="isWorking" class="loading-panel">Writing tag files...</div>

      <template v-else-if="exportResult">
        <div class="result-panel result-panel--success">
          <span class="result-panel-msg">
            Exported {{ exportResult.count }} tag{{ exportResult.count === 1 ? '' : 's' }} to
            <span class="result-panel-path">{{ exportResult.location }}</span>
          </span>
        </div>
        <div class="action-row">
          <button class="btn btn-primary" @click="exportResult = null">Export More</button>
          <button
            class="btn btn-primary btn-primary-muted"
            @click="revealItemInDir(exportResult!.location)"
          >
            Show in Folder
          </button>
        </div>
      </template>

      <template v-else>
        <TagSelectList
          v-model="selected"
          label="Select Tags to Export"
          :tag-names="save.tagNames.value"
        />

        <div class="target-row">
          <span class="target-label">Save as</span>
          <button
            class="target-btn"
            :class="{ active: exportTarget === 'folder' }"
            @click="exportTarget = 'folder'"
          >
            Folder of .r2tag files
          </button>
          <button
            class="target-btn"
            :class="{ active: exportTarget === 'pack' }"
            @click="exportTarget = 'pack'"
          >
            Single .r2pack
          </button>
        </div>

        <button
          class="btn btn-primary"
          :disabled="!selected.size || !save.canWriteSave.value"
          @click="exportSelected"
        >
          Export {{ selected.size || '' }} Selected Tag{{ selected.size === 1 ? '' : 's' }}
        </button>
      </template>
    </div>

    <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>
  </AnimatedCard>
</template>

<style scoped lang="scss">
.identity,
.published {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.6rem;
  padding: 0.7rem;
  background: var(--surface-inset);
  border-radius: 0.4rem;
}

.identity span,
.published small {
  color: var(--text-muted);
  font-size: 0.75rem;
  flex: 1;
}

.published {
  flex-direction: column;
  align-items: flex-start;
  gap: 0.2rem;
}

.link-btn {
  background: none;
  border: 0;
  color: var(--text-muted);
  cursor: pointer;

  &:hover {
    color: var(--text-primary);
  }
}

.upload-label {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  color: var(--text-muted);
  font-size: 0.8rem;

  select {
    padding: 0.65rem;
    color: var(--text-primary);
    background: var(--surface-inset);
    border: 1px solid var(--line);
    border-radius: 0.4rem;
    font-family: inherit;
  }
}

.hint,
.disclosure {
  width: 100%;
  color: var(--text-muted);
  font-size: 0.76rem;
}

.danger-btn {
  width: 100%;
  padding: 0.65rem;
  border: 1px solid rgba(248, 113, 113, 0.4);
  border-radius: 0.4rem;
  background: rgba(248, 113, 113, 0.1);
  color: var(--text-failure);
  cursor: pointer;
}

.confirm {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.6rem 0.7rem;
  border: 1px solid rgba(248, 113, 113, 0.4);
  border-radius: 0.4rem;
  background: rgba(248, 113, 113, 0.1);

  &-text {
    flex: 1;
    min-width: 0;
    font-size: 0.78rem;
    color: var(--text-failure);
  }

  &-btn {
    flex-shrink: 0;
    border: 1px solid var(--line);
    background: var(--surface-hover);
    color: var(--text-primary);
    border-radius: 0.4rem;
    padding: 0.3rem 0.6rem;
    font-size: 0.75rem;
    cursor: pointer;

    &--danger {
      border-color: rgba(248, 113, 113, 0.5);
      color: var(--text-failure);
    }
  }
}

.target-row {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.35rem;

  .target-label {
    font-size: 0.76rem;
    color: var(--text-muted);
    margin-right: 0.15rem;
  }

  .target-btn {
    flex: 1;
    border: 1px solid var(--line);
    border-radius: 0.4rem;
    padding: 0.4rem;
    font-size: 0.75rem;
    background: var(--surface-inset);
    color: var(--text-muted);
    cursor: pointer;
    transition: color 0.15s, border-color 0.15s, background 0.15s;

    &.active {
      color: var(--text-primary);
      border-color: var(--accent);
      background: var(--accent-completed);
    }
  }
}

.action-row {
  width: 100%;
  display: flex;
  gap: 0.625rem;

  .btn {
    flex: 1;
    font-size: 0.82em;
  }
}

.result-panel {
  width: 100%;
  padding: 1em;
  border-radius: var(--radius-panel);
  display: flex;
  flex-direction: column;
  gap: 0.5em;

  &--success {
    background: rgba(0, 255, 170, 0.06);
    border: 1px solid rgba(0, 255, 170, 0.2);
  }

  &-msg {
    font-size: 0.85em;
    color: var(--text-success);
  }

  &-path {
    font-family: 'Ubuntu Sans Mono Variable', monospace;
    word-break: break-all;
  }
}
</style>

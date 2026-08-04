<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import AnimatedCard from '../components/AnimatedCard.vue';
import SaveStatusNotice from '../components/SaveStatusNotice.vue';
import TabBar from '../components/TabBar.vue';
import TagSelect from '../components/TagSelect.vue';
import TagSelectList from '../components/TagSelectList.vue';
import ViewHeader from '../components/ViewHeader.vue';
import startggIcon from '../assets/startgg.svg';
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
  { id: 'publish', label: 'Publish to Cloud', icon: 'md-cloudupload-round' },
  { id: 'export', label: 'Export to Files', icon: 'md-folder-round' },
] as const satisfies readonly { id: TabName; label: string; icon: string }[];

const emit = defineEmits<{ 'go-back': [] }>();

const save = useSaveFile();
const auth = useCloudAuth();

const tab = ref<TabName>('publish');
const errorMsg = ref(cloudConfigured ? '' : CLOUD_UNCONFIGURED_MESSAGE);
const progress = ref('');
const isWorking = ref(false);
const confirmingDelete = ref(false);
// Overwriting a published tag is deliberate: pick a tag, then confirm.
const isReplacing = ref(false);

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
  isReplacing.value = false;
}

function startReplace() {
  confirmingDelete.value = false;
  progress.value = '';
  // Re-publishing the same tag is a refresh, so start the picker there.
  const published = auth.publishedTag.value?.tagName;
  if (published && save.tagNames.value.includes(published)) uploadTagName.value = published;
  isReplacing.value = true;
}

// The backend flattens HTTP failures into strings, so a dead session is only
// recognizable by its status line. When it happens, drop the stale identity
// instead of showing "Signed in as ..." next to a raw 401.
function handleCloudError(error: unknown) {
  const message = String(error);
  if (message.includes('Cloud API returned 401')) {
    void auth.signOut();
    isReplacing.value = false;
    confirmingDelete.value = false;
    errorMsg.value = 'Your session expired — please sign in again.';
  } else {
    errorMsg.value = message;
  }
}

// The composable owns the in-flight state, so this only has to report failure.
// Cancelling is a non-event: the user already knows they pressed the button.
async function signIn() {
  errorMsg.value = '';
  progress.value = '';
  try {
    await auth.signIn();
  } catch (error) {
    errorMsg.value = String(error);
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
    isReplacing.value = false;
  } catch (error) {
    handleCloudError(error);
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
    handleCloudError(error);
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
      <!-- The browser round trip gets its own panel: it is the one wait the app
           cannot end on its own, so it says what it is waiting for and always
           offers a way out. -->
      <template v-if="auth.isSigningIn.value">
        <div class="loading-panel">{{ auth.signInStatus.value }}</div>
        <p class="sign-in-hint">
          A <strong>start.gg</strong> page opened in your browser — approve access there.
        </p>
        <button class="btn btn-primary btn-primary-muted" @click="auth.cancelSignIn()">
          <v-icon name="md-close-round" scale="0.85" />
          Cancel Sign-in
        </button>
      </template>

      <div v-else-if="isWorking" class="loading-panel">Working with the cloud service…</div>

      <template v-else>
        <button
          v-if="!auth.signedInUser.value"
          class="btn btn-primary"
          :disabled="!cloudConfigured"
          @click="signIn"
        >
          <img :src="startggIcon" alt="" class="startgg-icon" />
          Sign in with start.gg
        </button>

        <template v-else>
          <div class="identity">
            <div class="identity-header">
              <span class="panel-label">Signed in as</span>
              <button class="link-btn" @click="auth.signOut()">
                <v-icon name="md-logout-round" scale="0.75" />
                Sign out
              </button>
            </div>
            <strong class="identity-name">{{ auth.signedInUser.value.gamerTag }}</strong>
            <span class="identity-slug">{{ auth.signedInUser.value.slug }}</span>
          </div>

          <div v-if="auth.publishedTag.value" class="published">
            <span class="panel-label">Published Tag</span>
            <strong class="published-name">{{ auth.publishedTag.value.tagName }}</strong>
            <small>Updated {{ new Date(auth.publishedTag.value.updatedAt).toLocaleString() }}</small>
          </div>

          <template v-if="!auth.publishedTag.value || isReplacing">
            <div class="tag-panel publish-panel">
              <span class="tag-panel-label">
                {{ isReplacing ? 'Select a Replacement Tag' : 'Tag to Publish' }}
              </span>
              <TagSelect
                v-model="uploadTagName"
                :options="save.tagNames.value"
                :disabled="!save.hasTags.value"
              />
            </div>

            <p class="disclosure">
              Publishing makes your start.gg username and in-game tag public.
            </p>
          </template>

          <!-- Step 2 of a replace: confirm the swap, or back out of it. -->
          <div v-if="isReplacing" class="action-row">
            <button class="btn btn-primary btn-primary-muted" @click="isReplacing = false">
              <v-icon name="md-close-round" scale="0.85" />
              Cancel
            </button>
            <button
              class="btn btn-primary"
              :disabled="!canPublish || !uploadTagName"
              @click="uploadTag"
            >
              <v-icon name="md-cloudupload-round" scale="0.85" />
              Replace Published Tag
            </button>
          </div>

          <button
            v-else-if="!auth.publishedTag.value"
            class="btn btn-primary"
            :disabled="!canPublish || !uploadTagName"
            @click="uploadTag"
          >
            <v-icon name="md-cloudupload-round" scale="0.85" />
            Publish Tag
          </button>

          <template v-else>
            <!-- Step 1 of a replace: opt in before the tag picker appears. -->
            <button class="btn btn-primary" :disabled="!canPublish" @click="startReplace">
              <v-icon name="md-swaphoriz-round" scale="0.85" />
              Replace Published Tag
            </button>

            <div v-if="confirmingDelete" class="confirm">
              <span class="confirm-text">Remove your published cloud tag?</span>
              <button class="confirm-btn" @click="confirmingDelete = false">
                <v-icon name="md-close-round" scale="0.7" />
                Cancel
              </button>
              <button class="confirm-btn confirm-btn--danger" @click="deleteTag">
                <v-icon name="md-delete-round" scale="0.7" />
                Remove
              </button>
            </div>
            <button v-else class="btn danger-btn" @click="confirmingDelete = true">
              <v-icon name="md-delete-round" scale="0.85" />
              Delete Published Tag
            </button>
          </template>
        </template>

        <p v-if="progress" class="progress-msg">
          <v-icon name="md-checkcircle-round" scale="0.8" />
          {{ progress }}
        </p>
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
          <button class="btn btn-primary" @click="exportResult = null">
            <v-icon name="md-refresh-round" scale="0.85" />
            Export More
          </button>
          <button
            class="btn btn-primary btn-primary-muted"
            @click="revealItemInDir(exportResult!.location)"
          >
            <v-icon name="md-folderopen-round" scale="0.85" />
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
            <v-icon name="md-folder-round" scale="0.75" />
            Folder of .r2tag files
          </button>
          <button
            class="target-btn"
            :class="{ active: exportTarget === 'pack' }"
            @click="exportTarget = 'pack'"
          >
            <v-icon name="md-archive-round" scale="0.75" />
            Single .r2pack
          </button>
        </div>

        <button
          class="btn btn-primary"
          :disabled="!selected.size || !save.canWriteSave.value"
          @click="exportSelected"
        >
          <v-icon name="md-upload-round" scale="0.85" />
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
  flex-direction: column;
  align-items: flex-start;
  gap: 0.25rem;
  padding: 0.8rem 0.9rem;
  background: var(--surface-inset);
  border: 1px solid var(--line-subtle);
  border-radius: var(--radius-panel);
}

.panel-label {
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.1em;
  font-weight: 600;
  color: var(--text-muted);
}

.identity-header {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.identity-name,
.published-name {
  font-size: 1.05rem;
}

.identity-slug {
  color: var(--text-muted);
  font-size: 0.75rem;
  font-family: 'Ubuntu Sans Mono Variable', monospace;
}

.published small {
  color: var(--text-muted);
  font-size: 0.72rem;
  margin-top: 0.1rem;
}

// Outlined, matching .panel-btn: the same kind of small secondary control
// tucked into a panel header, and it should read as a button rather than as
// part of the label beside it.
.link-btn {
  display: flex;
  align-items: center;
  gap: 0.3em;
  background: none;
  border: 1px solid var(--line-subtle);
  border-radius: var(--radius-button);
  color: var(--text-muted);
  font-size: 0.78rem;
  padding: 0.3em 0.6em;
  cursor: pointer;
  transition: color 500ms, border-color 500ms;

  &:hover {
    color: var(--text-primary);
    border-color: var(--accent);
  }
}

.startgg-icon {
  width: 1em;
  height: 1em;
  flex-shrink: 0;
}

.publish-panel {
  padding-bottom: 0.75rem;

  .tag-panel-label {
    margin-bottom: 0.5rem;
  }
}

.disclosure {
  width: 100%;
  color: var(--text-muted);
  font-size: 0.76rem;
}

.sign-in-hint {
  width: 100%;
  color: var(--text-muted);
  font-size: 0.78rem;
  line-height: 1.45;
  text-align: center;

  strong {
    color: var(--text-primary);
    font-weight: 600;
  }
}

.progress-msg {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 0.45rem;
  padding: 0.65rem 0.8rem;
  background: rgba(0, 255, 170, 0.06);
  border: 1px solid rgba(0, 255, 170, 0.2);
  border-radius: var(--radius-panel);
  color: var(--text-success);
  font-size: 0.85em;
  font-weight: 600;
}

// Sizing comes from .btn; this only recolors it. Restating the box here is what
// let it drift out of step with every other full-width button.
.danger-btn {
  border-color: rgba(248, 113, 113, 0.4);
  background: rgba(248, 113, 113, 0.1);
  color: var(--text-failure);

  &:hover {
    background: rgba(248, 113, 113, 0.18);
    border-color: rgba(248, 113, 113, 0.65);
  }
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
    display: flex;
    align-items: center;
    gap: 0.3rem;
    border: 1px solid var(--line);
    background: var(--surface-hover);
    color: var(--text-primary);
    border-radius: 0.4rem;
    padding: 0.3rem 0.6rem;
    font-size: 0.75rem;
    cursor: pointer;
    transition: border-color 500ms, background 500ms, transform 500ms;

    &:hover {
      background: rgba(255, 255, 255, 0.22);
      border-color: var(--accent);
      transform: translateY(-0.15em);
    }

    &--danger {
      border-color: rgba(248, 113, 113, 0.5);
      color: var(--text-failure);

      &:hover {
        background: rgba(248, 113, 113, 0.2);
        border-color: rgba(248, 113, 113, 0.9);
      }
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
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
    border: 1px solid var(--line);
    border-radius: 0.4rem;
    padding: 0.4rem;
    font-size: 0.75rem;
    background: var(--surface-inset);
    color: var(--text-muted);
    cursor: pointer;
    transition: color 500ms, border-color 500ms, background 500ms, transform 500ms;

    &:hover {
      color: var(--text-primary);
      background: var(--surface-hover);
      transform: translateY(-0.15em);
    }

    &.active {
      color: var(--text-primary);
      border-color: var(--accent);
      background: var(--accent-completed);

      &:hover {
        background: var(--accent-completed);
        border-color: var(--accent-hover);
      }
    }
  }
}

.action-row {
  width: 100%;
  display: flex;
  gap: 0.625rem;

  .btn {
    flex: 1;
    font-size: 0.9em;
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

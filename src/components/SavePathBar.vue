<script setup lang="ts">
import { computed } from 'vue';

/**
 * The one bar that says which file the whole app is pointed at.
 *
 * Save paths are routinely longer than the card is wide (the Windows default
 * runs past 80 characters), so the label truncates in the middle rather than at
 * the end: the head identifies *whose* save it is — which is the question worth
 * answering on a shared tournament PC — and the tail is the filename. Plain
 * `text-overflow: ellipsis` would drop exactly those two and keep the middle.
 */

const props = defineProps<{
  label: string;
  /** Truncate as a path. Off for status text like "…not found". */
  isPath?: boolean;
  status?: 'idle' | 'error' | 'success';
}>();

/**
 * Everything before the last separator; empty for non-paths. The separator
 * itself goes to the tail, so the join reads `…\Rivals2_PlayerTagSaveSlot.sav`
 * — the ellipsis lands where a directory name would, instead of floating
 * between two words.
 */
const head = computed(() => {
  if (!props.isPath) return '';
  const cut = Math.max(props.label.lastIndexOf('\\'), props.label.lastIndexOf('/'));
  return cut < 0 ? '' : props.label.slice(0, cut);
});

const tail = computed(() => props.label.slice(head.value.length));
</script>

<template>
  <div class="save-path-bar">
    <v-icon name="md-folderopen-round" scale="0.9" class="save-path-bar-icon" />
    <span class="save-path-bar-caption">Save file</span>

    <!-- The tooltip carries the untruncated path, and sits on the label rather
         than the bar so it can't fire alongside an action button's own. -->
    <span
      class="save-path-bar-label"
      :class="`save-path-bar-label--${status ?? 'success'}`"
      :data-tooltip="isPath ? label : undefined"
    >
      <span v-if="head" class="save-path-bar-head">{{ head }}</span>
      <span class="save-path-bar-tail">{{ tail }}</span>
    </span>
    <!-- Reload / change controls; omitted on read-only bars. -->
    <slot name="actions" />
  </div>
</template>

<style scoped lang="scss">
.save-path-bar {
  width: 100%;
  display: flex;
  align-items: center;
  background: var(--surface-inset);
  border: 1px solid var(--line-subtle);
  border-radius: var(--radius-button);
  padding: 0.5em 0.85em;
  gap: 0.6em;

  &-icon {
    flex-shrink: 0;
    color: rgba(200, 180, 230, 0.35);
  }

  &-caption {
    flex-shrink: 0;
    font-size: 0.62em;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.09em;
    color: var(--text-muted);
  }

  &-label {
    flex: 1;
    display: flex;
    align-items: baseline;
    min-width: 0;
    // No `overflow: hidden` here, however tempting: it would clip the tooltip
    // this element anchors. The two spans below keep themselves in bounds.
    font-family: 'Ubuntu Sans Mono Variable', monospace;
    font-size: 0.7em;

    &--idle    { color: var(--text-muted); }
    &--error   { color: var(--text-failure); }
    &--success { color: var(--text-success); }
  }

  // The head absorbs all the shrinking; the filename is never elided.
  &-head {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  // Held at full width until it alone would overrun the bar; only then does it
  // give up its own end, which is the one case where nothing better exists.
  &-tail {
    flex-shrink: 0;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}
</style>

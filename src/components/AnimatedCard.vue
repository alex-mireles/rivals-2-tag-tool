<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';

/** The home screen puts two columns side by side and needs more room than the
 *  single-column flows the 500px card was sized for. */
/** `static` opts out of the height-tracking animation entirely: no inline
 *  height is set and no ResizeObserver is attached, so the card just sizes
 *  to its content. Meant for callers that never swap between sibling views
 *  (the animation exists to smooth transitions between those), where a
 *  pinned height only risks clipping content that grows after mount. */
const props = defineProps<{ wide?: boolean; static?: boolean }>();

const shell = ref<HTMLDivElement | null>(null);
const content = ref<HTMLDivElement | null>(null);

let observer: ResizeObserver | null = null;

onMounted(() => {
  if (props.static) return;

  const shellEl = shell.value;
  const contentEl = content.value;
  if (!shellEl || !contentEl) return;

  // height is border-box, but the measured content sits inside the border.
  const styles = getComputedStyle(shellEl);
  const borderY = parseFloat(styles.borderTopWidth) + parseFloat(styles.borderBottomWidth);

  const syncHeight = () => {
    shellEl.style.height = `${contentEl.getBoundingClientRect().height + borderY}px`;
  };

  // Runs before first paint, so mounting never animates from an empty card.
  syncHeight();

  observer = new ResizeObserver(syncHeight);
  observer.observe(contentEl);
});

onBeforeUnmount(() => {
  observer?.disconnect();
});
</script>

<template>
  <div ref="shell" class="card" :class="{ 'card--wide': wide, 'card--static': static }">
    <div ref="content" class="card-content">
      <slot />
    </div>
  </div>
</template>

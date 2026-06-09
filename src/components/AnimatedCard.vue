<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';

const shell = ref<HTMLDivElement | null>(null);
const content = ref<HTMLDivElement | null>(null);

let observer: ResizeObserver | null = null;

onMounted(() => {
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
  <div ref="shell" class="card">
    <div ref="content" class="card-content">
      <slot />
    </div>
  </div>
</template>

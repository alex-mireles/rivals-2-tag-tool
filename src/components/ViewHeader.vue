<script setup lang="ts">
defineProps<{ title: string }>();

defineEmits<{
  'go-back': [];
}>();
</script>

<template>
  <div class="card-view-header">
    <!-- `data-tooltip` renders through a ::after pseudo-element, which is not an
         accessible name; an icon-only button still needs `aria-label`. -->
    <button
      class="back-btn"
      aria-label="Back to the home screen"
      data-tooltip="Back to the home screen"
      @click="$emit('go-back')"
    >
      <v-icon name="md-arrowback-round" scale="1.1" />
    </button>
    <span class="card-view-header-title">{{ title }}</span>
  </div>
</template>

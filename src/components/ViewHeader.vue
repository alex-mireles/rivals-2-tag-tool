<script setup lang="ts">
defineProps<{ title: string }>();

defineEmits<{
  'go-back': [];
}>();
</script>

<template>
  <div class="card-view-header">
    <button class="back-btn" title="Back" @click="$emit('go-back')">
      <v-icon name="md-arrowback-round" scale="1.1" />
    </button>
    <span class="card-view-header-title">{{ title }}</span>
  </div>
</template>

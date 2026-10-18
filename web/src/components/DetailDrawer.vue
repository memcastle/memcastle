<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue"

const props = defineProps<{ title: string; selectionKey: string }>()
const emit = defineEmits<{ close: [] }>()
const panel = ref<HTMLElement>()
const heading = ref<HTMLElement>()
// Remember the row that opened the panel so closing it does not strand a keyboard reader.
const origin = document.activeElement instanceof HTMLElement ? document.activeElement : null

function close(): void {
  emit("close")
  void nextTick(() => { if (origin?.isConnected) origin.focus() })
}
function onKeydown(event: KeyboardEvent): void {
  // A launch dialog owns Escape while it is on top of the resource details.
  if (document.querySelector('.p-dialog[aria-modal="true"]')) return
  if (event.key === "Escape") { event.preventDefault(); close() }
}
onMounted(() => { window.addEventListener("keydown", onKeydown); heading.value?.focus() })
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown)
  // Browser Back also closes a linked detail; return focus if it was still inside the departing panel.
  if (panel.value?.contains(document.activeElement)) void nextTick(() => { if (origin?.isConnected) origin.focus() })
})
watch(() => props.selectionKey, () => void nextTick(() => heading.value?.focus()))
</script>

<template>
  <aside ref="panel" class="detail-drawer panel" role="region" :aria-label="title">
    <header class="detail-drawer-header">
      <h2 ref="heading" tabindex="-1">{{ title }}</h2>
      <button type="button" class="detail-close" aria-label="Close details" @click="close">Close</button>
    </header>
    <div class="detail-drawer-body"><slot /></div>
  </aside>
</template>

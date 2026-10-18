<script setup lang="ts">
import ProgressBar from "openvue/progressbar"
import { computed } from "vue"
import type { Job } from "../api/types.ts"

const props = defineProps<{ job: Job }>()

const known = computed(() => props.job.progress.total !== null && props.job.progress.total > 0)
// Clamp only the bar: the daemon's exact counts must remain visible even when work discovers extra items.
const percent = computed(() => known.value ? Math.max(0, Math.min(100, Math.round(props.job.progress.current / props.job.progress.total! * 100))) : null)
const count = computed(() => known.value ? `${props.job.progress.current} / ${props.job.progress.total}` : `${props.job.progress.current} processed · total unknown`)
const description = computed(() => known.value ? `${percent.value}% · ${count.value}` : count.value)
const runningUnknown = computed(() => !known.value && props.job.status === "running")
</script>

<template>
  <div class="job-progress" :title="description" tabindex="0">
    <ProgressBar
      v-if="known || runningUnknown"
      :value="percent ?? undefined"
      :mode="runningUnknown ? 'indeterminate' : 'determinate'"
      :aria-label="`Job progress: ${description}`"
      :aria-valuetext="description"
    />
    <span class="job-progress-text muted">{{ description }}<template v-if="job.progress.message"> · {{ job.progress.message }}</template></span>
  </div>
</template>

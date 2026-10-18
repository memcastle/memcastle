<script setup lang="ts">
import Button from "openvue/button"
import Checkbox from "openvue/checkbox"
import Dialog from "openvue/dialog"
import InputText from "openvue/inputtext"
import Message from "openvue/message"
import { useToast } from "openvue/usetoast"
import { computed, reactive, ref, watch } from "vue"
import type { AdapterInfo, Job, JobRequest, Miner, SourceConfiguration } from "../api/types.ts"
import { asApiError } from "../composables/useLoad.ts"
import { shortId } from "../format.ts"
import { mineRequest } from "../mine.ts"
import { useSession } from "../session.ts"

const props = defineProps<{ visible: boolean }>()
const emit = defineEmits<{ close: []; submitted: [job: Job] }>()
const { client, state: session } = useSession()
const toast = useToast()
type LaunchType = "mine" | "extract" | "embed" | "audit" | "repair"
const type = ref<LaunchType>("mine")
const mineKind = ref<"directory" | "source" | "miner">("directory")
const path = ref("")
const source = ref("")
const minerId = ref("")
const locator = ref("")
const wing = ref("")
const full = ref(false)
const dryRun = ref(true)
const profile = ref("")
const refresh = ref<"missing" | "stale" | "all">("missing")
const options = reactive<Record<string, string>>({})
const adapters = ref<AdapterInfo[]>([])
const miners = ref<Miner[]>([])
const configs = ref<SourceConfiguration[]>([])
const discoveryError = ref("")
const error = ref("")
const busy = ref(false)
const writable = computed(() => session.mode === "full")
const availableSources = computed(() => adapters.value.filter((adapter) => adapter.state === "enabled"))
const selectedMiner = computed(() => miners.value.find((miner) => miner.name === minerId.value))
const selectedSource = computed(() => availableSources.value.find((adapter) => adapter.name === (mineKind.value === "directory" ? "directory" : source.value)))
const sourceConfig = computed(() => configs.value.find((config) => config.id === source.value))
const sourceReady = computed(() => !!selectedSource.value && !(sourceConfig.value?.credential_kind && !sourceConfig.value.credential_available))
const acceptedOptions = computed(() => mineKind.value === "miner" ? [] : selectedSource.value?.options ?? [])
const selectedConfig = computed(() => configs.value.find((config) => config.id === (selectedMiner.value?.source_config ?? selectedMiner.value?.source)))
const canSubmit = computed(() => {
  if (busy.value || (!writable.value && !(type.value === "audit" || (type.value === "repair" && dryRun.value)))) return false
  if (type.value !== "mine") return true
  if (mineKind.value === "miner") return selectedMiner.value?.state === "ready" && selectedMiner.value.enabled
  if (mineKind.value === "directory") return path.value.trim().startsWith("/")
  return sourceReady.value
})

// Discovery is only needed for installed sources and saved miners; an absolute directory still runs without it.
watch(() => props.visible, async (visible) => {
  if (!visible) return
  error.value = ""
  discoveryError.value = ""
  const results = await Promise.allSettled([client.sources(), client.miners(), client.sourceConfigs()])
  if (!props.visible) return
  adapters.value = results[0].status === "fulfilled" ? results[0].value.adapters : []
  miners.value = results[1].status === "fulfilled" ? results[1].value.miners : []
  configs.value = results[2].status === "fulfilled" ? results[2].value : []
  if (results.some((result) => result.status === "rejected")) discoveryError.value = "Some sources or miners could not be loaded. Directory mining and direct jobs are still available."
})
watch([type, mineKind, source], () => {
  error.value = ""
  // Do not submit a source's old option keys after the user switches to another launch.
  for (const key of Object.keys(options)) delete options[key]
})
const orUndefined = (text: string) => text.trim() || undefined

async function submit(): Promise<void> {
  if (!canSubmit.value) return
  if (type.value === "repair" && !dryRun.value && !window.confirm("Remove the orphaned drawers an audit finds? This cannot be undone. A dry run shows what would go.")) return
  busy.value = true
  error.value = ""
  try {
    let job: Job
    if (type.value === "mine" && mineKind.value === "miner") {
      job = await client.runMiner(minerId.value, full.value)
    } else {
      let request: JobRequest
      switch (type.value) {
        case "mine": request = mineRequest({ kind: mineKind.value === "directory" ? "directory" : "source", path: path.value, source: source.value, locator: locator.value, wing: wing.value, full: full.value, options }, acceptedOptions.value); break
        case "audit": request = { type: "audit", wing: orUndefined(wing.value) }; break
        case "repair": request = { type: "repair", dry_run: dryRun.value }; break
        case "extract": request = { type: "extract", wing: orUndefined(wing.value) }; break
        case "embed": request = { type: "embed", wing: orUndefined(wing.value), profile: orUndefined(profile.value), refresh: refresh.value }; break
      }
      job = await client.submitJob(request)
    }
    toast.add({ severity: "success", summary: "Job submitted", detail: `Job ${shortId(job.id)}`, life: 4000 })
    emit("submitted", job)
  } catch (caught) {
    const refused = asApiError(caught)
    error.value = refused.help ? `${refused.message} ${refused.help}` : refused.message
  } finally { busy.value = false }
}
</script>

<template>
  <Dialog :visible="visible" modal append-to="self" header="Launch work" :style="{ width: 'min(42rem, 95vw)' }" @update:visible="emit('close')">
    <div class="field"><label for="launch-type">Job type</label><select id="launch-type" v-model="type">
      <option value="mine">Mine source or run miner</option><option value="extract">Extract entities</option><option value="embed">Embed drawers</option><option value="audit">Audit palace</option><option value="repair">Repair palace</option>
    </select></div>
    <Message v-if="discoveryError && type === 'mine'" severity="warn" :closable="false">{{ discoveryError }}</Message>
    <template v-if="type === 'mine'">
      <p class="muted">An ad-hoc source run uses its default source configuration. A configured miner selects a source configuration and its own destination and options; schedules and triggers run separately.</p>
      <div class="field"><label for="mine-kind">Mining run</label><select id="mine-kind" v-model="mineKind">
        <option value="directory">Directory</option><option value="source" :disabled="!availableSources.length">Installed source</option><option value="miner" :disabled="!miners.length">Configured miner</option>
      </select></div>
      <div v-if="mineKind === 'directory'" class="field"><label for="mine-path">Directory (absolute path on the daemon's machine)</label><InputText id="mine-path" v-model="path" fluid /><small v-if="path && !path.trim().startsWith('/')" role="alert">Enter an absolute path.</small></div>
      <template v-else-if="mineKind === 'source'">
        <div class="field"><label for="mine-source">Enabled source</label><select id="mine-source" v-model="source"><option value="">Choose a source</option><option v-for="adapter in availableSources" :key="adapter.name" :value="adapter.name">{{ adapter.name }}</option></select></div>
        <p v-if="source" class="muted">Default source configuration: {{ source }}{{ sourceConfig?.credential_kind ? ` · ${sourceConfig.credential_available ? 'credential available' : 'credential unavailable; sign in before running'}` : '' }}.</p>
        <div class="field"><label for="mine-locator">Place to read (source default when empty)</label><InputText id="mine-locator" v-model="locator" fluid /></div>
      </template>
      <template v-else>
        <div class="field"><label for="mine-miner">Ready miner</label><select id="mine-miner" v-model="minerId"><option value="">Choose a miner</option><option v-for="miner in miners" :key="miner.name" :value="miner.name" :disabled="miner.state !== 'ready' || !miner.enabled">{{ miner.name }} — {{ miner.state }}{{ miner.reason ? `: ${miner.reason}` : '' }}</option></select></div>
        <p v-if="selectedMiner" class="muted">Source {{ selectedMiner.source }} · configuration {{ selectedConfig?.id ?? selectedMiner.source_config ?? selectedMiner.source }}{{ selectedConfig?.credential_kind ? ` · ${selectedConfig.credential_available ? 'credential available' : 'credential unavailable'}` : '' }} · preset destination and options. {{ selectedMiner.schedule?.enabled ? 'Schedule enabled' : 'No active schedule' }}.</p>
      </template>
      <div v-for="spec in acceptedOptions" :key="spec.name" class="field"><label :for="`mine-option-${spec.name}`">{{ spec.name }}: {{ spec.description }}</label><InputText :id="`mine-option-${spec.name}`" v-model="options[spec.name]" fluid /></div>
      <div v-if="mineKind !== 'miner'" class="field"><label for="mine-wing">Wing (source default when empty)</label><InputText id="mine-wing" v-model="wing" fluid /></div>
      <label class="row"><Checkbox v-model="full" binary /> Read everything again, ignoring the cursor</label>
    </template>
    <template v-else-if="type !== 'repair'">
      <p v-if="type === 'audit'" class="muted">Read-only consistency report. Orphan and provenance checks are palace-wide even when a wing is selected.</p>
      <p v-else-if="type === 'extract'" class="muted">Add entities and relationships from mined drawers; needs an extraction provider.</p>
      <p v-else class="muted">Compute search vectors for drawers; needs an embedding provider.</p>
      <div class="field"><label for="launch-wing">Wing (whole palace when empty)</label><InputText id="launch-wing" v-model="wing" fluid /></div>
      <template v-if="type === 'embed'">
        <div class="field"><label for="embed-profile">Embedding profile (default when empty)</label><InputText id="embed-profile" v-model="profile" fluid /></div>
        <div class="field"><label for="embed-refresh">Refresh</label><select id="embed-refresh" v-model="refresh"><option value="missing">Missing only</option><option value="stale">Missing and stale</option><option value="all">All</option></select></div>
      </template>
    </template>
    <template v-else><p class="muted">A dry run shows orphaned drawers without changing anything. Applying a repair removes them.</p><label class="row"><Checkbox v-model="dryRun" binary /> Dry run (recommended)</label></template>
    <Message v-if="!writable && !canSubmit && !busy" severity="warn" :closable="false">This run needs full access. Audit and repair dry runs are available in read-only mode.</Message>
    <Message v-if="error" severity="error" :closable="false" role="alert">{{ error }}</Message>
    <div class="row" style="margin-top: 1rem"><Button label="Launch job" :loading="busy" :disabled="!canSubmit" @click="submit" /><Button label="Cancel" severity="secondary" text @click="emit('close')" /></div>
  </Dialog>
</template>

<script setup lang="ts">
import Button from "openvue/button"
import InputText from "openvue/inputtext"
import Message from "openvue/message"
import { computed, onMounted, reactive, ref, watch } from "vue"
import { RouterLink, useRoute, useRouter } from "vue-router"
import type { MinerPatch, SourceConfiguration, Trigger } from "../api/types.ts"
import PageHeader from "../components/PageHeader.vue"
import { asApiError } from "../composables/useLoad.ts"
import { formatOptions, optionPatch } from "../optionEditor.ts"
import { useSession } from "../session.ts"

const { client } = useSession()
const route = useRoute()
const router = useRouter()
const report = ref<Awaited<ReturnType<typeof client.miners>> | null>(null)
const configs = ref<SourceConfiguration[]>([])
const triggers = ref<Trigger[]>([])
const error = ref("")
const id = ref("")
const allowMinerBroaden = ref(false)
const edit = reactive({ source: "", sourceConfig: "", locator: "", wing: "", room: "", wingFrom: "", roomFrom: "", every: "", at: "", maxDocuments: "", embeddingProfile: "", options: "" })
const selected = computed(() => report.value?.miners.find((miner) => miner.name === id.value))
const minerTriggers = (name: string) => triggers.value.filter((trigger) => trigger.miner === name)

async function refresh(): Promise<void> {
  try {
    const [miners, sourceConfigs, triggerReport] = await Promise.all([client.miners(), client.sourceConfigs(), client.triggers()])
    report.value = miners
    configs.value = sourceConfigs
    triggers.value = triggerReport.triggers ?? []
    error.value = ""
  } catch (caught) { error.value = asApiError(caught).message }
}
onMounted(refresh)
watch([() => route.query.miner, report], ([name]) => { if (typeof name === "string") select(name) }, { immediate: true })

function select(name: string): void {
  const miner = report.value?.miners.find((item) => item.name === name)
  if (!miner) return
  id.value = miner.name
  edit.source = miner.source
  edit.sourceConfig = miner.source_config ?? ""
  edit.locator = miner.locator ?? ""
  edit.wing = miner.wing ?? ""
  edit.room = miner.room ?? ""
  edit.wingFrom = miner.mapping?.wing_from ?? ""
  edit.roomFrom = miner.mapping?.room_from ?? ""
  edit.every = miner.schedule?.every ?? ""
  edit.at = miner.schedule?.at ?? ""
  edit.maxDocuments = miner.max_documents?.toString() ?? ""
  edit.embeddingProfile = miner.embedding_profile ?? ""
  edit.options = formatOptions(miner.options ?? {})
}

async function save(): Promise<void> {
  try {
    const { options, unset_options } = optionPatch(edit.options, selected.value?.options ?? {})
    const patch: MinerPatch = { source: edit.source, options, unset_options, allow_broaden: allowMinerBroaden.value }
    const unset: string[] = []
    for (const [field, value] of [["source_config", edit.sourceConfig], ["locator", edit.locator], ["wing", edit.wing], ["room", edit.room], ["embedding_profile", edit.embeddingProfile]] as const) {
      if (value.trim()) patch[field] = value.trim()
      else if (report.value?.miners.some((miner) => miner.name === id.value)) unset.push(field)
    }
    if (edit.every.trim()) patch.schedule = { every: edit.every.trim(), at: edit.at.trim() || undefined, enabled: report.value?.miners.find((miner) => miner.name === id.value)?.schedule?.enabled ?? false }
    else if (report.value?.miners.find((miner) => miner.name === id.value)?.schedule) unset.push("schedule")
    if (edit.wingFrom.trim() || edit.roomFrom.trim()) patch.mapping = { wing_from: edit.wingFrom.trim() || undefined, room_from: edit.roomFrom.trim() || undefined }
    else if (report.value?.miners.find((miner) => miner.name === id.value)?.mapping) unset.push("mapping")
    if (edit.maxDocuments.trim()) {
      const limit = Number(edit.maxDocuments)
      if (!Number.isSafeInteger(limit) || limit < 1) throw new Error("Max documents must be a positive integer")
      patch.max_documents = limit
    } else if (report.value?.miners.find((miner) => miner.name === id.value)?.max_documents) unset.push("max_documents")
    if (unset.length) patch.unset = unset
    await client.setMiner(id.value, patch)
    allowMinerBroaden.value = false
    await refresh()
    await router.replace({ name: "miners", query: { miner: id.value } })
  } catch (caught) { error.value = asApiError(caught).message }
}

async function schedule(name: string, enabled: boolean): Promise<void> {
  try { await client.scheduleMiner(name, enabled); await refresh() }
  catch (caught) { error.value = asApiError(caught).message }
}
async function enable(name: string, enabled: boolean): Promise<void> {
  try { await client.enableMiner(name, enabled); await refresh() }
  catch (caught) { error.value = asApiError(caught).message }
}
async function remove(name: string): Promise<void> {
  try { await client.removeMiner(name); await refresh() }
  catch (caught) { error.value = asApiError(caught).message }
}

</script>

<template>
  <PageHeader title="Miners" subtitle="Input scope and destination presets. Each miner may have several triggers and an optional schedule." />
  <Message v-if="error" severity="error" :closable="false">{{ error }}</Message>
  <Message v-if="report?.error" severity="warn" :closable="false">{{ report.error }}</Message>
  <section class="panel">
    <h2>Configured miners</h2>
    <div v-for="miner in report?.miners ?? []" :key="miner.name" class="row">
      <Button :label="miner.name" text @click="router.push({ name: 'miners', query: { miner: miner.name } })" />
      <span>{{ miner.source }} · {{ miner.source_config ?? miner.source }} · {{ miner.state }}</span>
      <Button :label="miner.enabled ? 'Disable miner' : 'Enable miner'" size="small" severity="secondary" outlined @click="enable(miner.name, !miner.enabled)" />
      <span>{{ minerTriggers(miner.name).length }} trigger{{ minerTriggers(miner.name).length === 1 ? '' : 's' }}</span>
      <span v-if="miner.reason" class="muted">{{ miner.reason }}</span>
      <span v-if="miner.schedule">every {{ miner.schedule.every }} {{ miner.schedule.enabled ? "(on)" : "(off)" }}</span>
      <Button v-if="miner.schedule" :label="miner.schedule.enabled ? 'Stop schedule' : 'Enable schedule'" size="small" @click="schedule(miner.name, !miner.schedule!.enabled)" />
      <Button label="Edit" size="small" outlined @click="router.push({ name: 'miners', query: { miner: miner.name } })" />
      <Button label="Remove" size="small" severity="danger" outlined @click="remove(miner.name)" />
    </div>
    <p v-if="!report?.miners.length" class="muted">No miners configured yet.</p>
  </section>
  <section v-if="selected" class="panel">
    <h2>Runs for {{ selected.name }}</h2>
    <p>Source <strong>{{ selected.source }}</strong> using configuration <RouterLink :to="{ name: 'source-configs', query: { config: selected.source_config ?? selected.source } }">{{ selected.source_config ?? selected.source }}</RouterLink>.</p>
    <p v-if="selected.schedule">Miner-owned schedule: {{ selected.schedule.every }} ({{ selected.schedule.enabled ? 'on' : 'off' }}).</p>
    <h3>Independent triggers</h3>
    <p v-if="!minerTriggers(selected.name).length" class="muted">No independent triggers for this miner.</p>
    <div v-for="trigger in minerTriggers(selected.name)" :key="trigger.name" class="row"><RouterLink :to="{ name: 'triggers', query: { miner: selected.name } }">{{ trigger.name }}</RouterLink><span>{{ trigger.type }} · {{ trigger.status }}</span><span v-if="trigger.reason">{{ trigger.reason }}</span></div>
    <p class="muted">Triggers are configured with <code>memcastle trigger</code>. Manual runs are launched from Jobs.</p>
  </section>
  <section class="panel">
    <h2>{{ report?.miners.some((miner) => miner.name === id) ? 'Edit' : 'Create' }} miner</h2>
    <p class="muted">A new miner ID is a lowercase slug. Source credentials and defaults belong to a separate source configuration.</p>
    <div class="field-grid">
      <div class="field"><label for="miner-id">Miner ID</label><InputText id="miner-id" v-model="id" fluid /></div>
      <div class="field"><label for="miner-source">Source</label><InputText id="miner-source" v-model="edit.source" fluid /></div>
      <div class="field"><label for="miner-config">Source configuration (optional)</label><InputText id="miner-config" v-model="edit.sourceConfig" fluid /><small>Available: {{ configs.map((c) => c.id).join(', ') }}</small></div>
      <div class="field"><label for="miner-locator">Locator</label><InputText id="miner-locator" v-model="edit.locator" fluid /></div>
      <div class="field"><label for="miner-wing">Wing</label><InputText id="miner-wing" v-model="edit.wing" fluid /></div>
      <div class="field"><label for="miner-room">Room</label><InputText id="miner-room" v-model="edit.room" fluid /></div>
      <div class="field"><label for="miner-wing-from">Wing from metadata field (optional)</label><InputText id="miner-wing-from" v-model="edit.wingFrom" fluid /></div>
      <div class="field"><label for="miner-room-from">Room from metadata field (optional)</label><InputText id="miner-room-from" v-model="edit.roomFrom" fluid /></div>
      <div class="field"><label for="miner-limit">Max documents per job (empty inherits common default)</label><InputText id="miner-limit" v-model="edit.maxDocuments" fluid /></div>
      <div class="field"><label for="miner-profile">Embedding profile (optional)</label><InputText id="miner-profile" v-model="edit.embeddingProfile" fluid /></div>
      <div class="field"><label for="miner-every">Schedule interval (e.g. 1d)</label><InputText id="miner-every" v-model="edit.every" fluid /></div>
      <div class="field"><label for="miner-at">Schedule UTC time (e.g. 03:30)</label><InputText id="miner-at" v-model="edit.at" fluid /></div>
      <div class="field"><label for="miner-options">Source options (one KEY=VALUE per line)</label><textarea id="miner-options" v-model="edit.options" rows="4" /></div>
    </div>
    <label><input v-model="allowMinerBroaden" type="checkbox" /> Allow a broader input selection when changing this miner</label>
    <Button label="Save miner" :disabled="!id.trim() || !edit.source.trim()" @click="save" />
  </section>
</template>

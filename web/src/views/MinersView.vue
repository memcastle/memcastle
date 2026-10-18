<script setup lang="ts">
import Button from "openvue/button"
import InputText from "openvue/inputtext"
import Message from "openvue/message"
import { computed, onMounted, reactive, ref } from "vue"
import type { MinerPatch, SourceConfiguration } from "../api/types.ts"
import PageHeader from "../components/PageHeader.vue"
import { asApiError } from "../composables/useLoad.ts"
import { useSession } from "../session.ts"

const { client, state } = useSession()
const writable = computed(() => state.mode === "full")
const report = ref<Awaited<ReturnType<typeof client.miners>> | null>(null)
const configs = ref<SourceConfiguration[]>([])
const error = ref("")
const id = ref("")
const configId = ref("")
const configSource = ref("")
const configOptions = ref("")
const credentialKind = ref<"" | "oauth" | "env" | "file">("")
const credentialLocation = ref("")
const allowConfigBroaden = ref(false)
const allowMinerBroaden = ref(false)
const edit = reactive({ source: "", sourceConfig: "", locator: "", wing: "", room: "", wingFrom: "", roomFrom: "", every: "", at: "", maxDocuments: "", options: "" })

async function refresh(): Promise<void> {
  try {
    [report.value, configs.value] = await Promise.all([client.miners(), client.sourceConfigs()])
    error.value = ""
  } catch (caught) { error.value = asApiError(caught).message }
}
onMounted(refresh)

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
  edit.options = Object.entries(miner.options ?? {}).map(([key, value]) => `${key}=${Array.isArray(value) ? value.join(",") : value}`).join("\n")
}

async function save(): Promise<void> {
  try {
    const options: Record<string, string> = {}
    for (const line of edit.options.split("\n").filter((value) => value.trim())) {
      const index = line.indexOf("=")
      if (index <= 0) throw new Error(`Use KEY=VALUE for each option: ${line}`)
      options[line.slice(0, index).trim()] = line.slice(index + 1).trim()
    }
    const patch: MinerPatch = { source: edit.source, options, allow_broaden: allowMinerBroaden.value }
    const unset: string[] = []
    for (const [field, value] of [["source_config", edit.sourceConfig], ["locator", edit.locator], ["wing", edit.wing], ["room", edit.room]] as const) {
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
  } catch (caught) { error.value = asApiError(caught).message }
}

async function schedule(name: string, enabled: boolean): Promise<void> {
  try { await client.scheduleMiner(name, enabled); await refresh() }
  catch (caught) { error.value = asApiError(caught).message }
}
async function remove(name: string): Promise<void> {
  try { await client.removeMiner(name); await refresh() }
  catch (caught) { error.value = asApiError(caught).message }
}

async function saveConfig(): Promise<void> {
  try {
    const options: Record<string, string> = {}
    for (const line of configOptions.value.split("\n").filter(Boolean)) {
      const index = line.indexOf("=")
      if (index < 1) throw new Error(`Give KEY=VALUE: ${line}`)
      options[line.slice(0, index).trim()] = line.slice(index + 1).trim()
    }
    const credential = credentialKind.value === "oauth" ? { type: "oauth" as const }
      : credentialKind.value === "env" ? { type: "env" as const, name: credentialLocation.value }
      : credentialKind.value === "file" ? { type: "file" as const, path: credentialLocation.value }
      : undefined
    await client.setSourceConfig(configId.value, { source: configSource.value, options, credential, allow_broaden: allowConfigBroaden.value })
    allowConfigBroaden.value = false
    await refresh()
  } catch (caught) { error.value = asApiError(caught).message }
}

</script>

<template>
  <PageHeader title="Miners" subtitle="Reusable source and destination presets. Schedules start disabled until explicitly enabled." />
  <Message v-if="error" severity="error" :closable="false">{{ error }}</Message>
  <Message v-if="report?.error" severity="warn" :closable="false">{{ report.error }}</Message>
  <section class="panel">
    <h2>Source configurations</h2>
    <p class="muted">A configuration holds reusable source-native defaults and sign-in status; it does not schedule or map content.</p>
    <div v-for="config in configs" :key="config.id" class="row">
      <Button :label="config.id" text @click="configId = config.id; configSource = config.source; configOptions = Object.entries(config.options).map(([key, value]) => `${key}=${value}`).join('\n'); credentialKind = config.credential_kind === 'oauth' ? 'oauth' : ''; credentialLocation = ''" />
      <span>{{ config.source }} · {{ config.credential_kind ?? 'no credential' }} {{ config.credential_available ? '(available)' : '' }}</span>
    </div>
    <div class="field-grid">
      <div class="field"><label for="config-id">Configuration ID</label><InputText id="config-id" v-model="configId" fluid /></div>
      <div class="field"><label for="config-source">Source</label><InputText id="config-source" v-model="configSource" fluid /></div>
      <div class="field"><label for="config-options">Source defaults (KEY=VALUE per line)</label><textarea id="config-options" v-model="configOptions" rows="3" /></div>
      <div class="field"><label for="config-credential">Credential reference (optional)</label><select id="config-credential" v-model="credentialKind"><option value="">Keep current</option><option value="oauth">OAuth</option><option value="env">Environment</option><option value="file">File</option></select></div>
      <div v-if="credentialKind === 'env' || credentialKind === 'file'" class="field"><label for="config-credential-location">{{ credentialKind === 'env' ? 'Environment variable name' : 'File path' }}</label><InputText id="config-credential-location" v-model="credentialLocation" fluid /></div>
      <label><input v-model="allowConfigBroaden" type="checkbox" /> Allow a broader source selection for every miner using this configuration</label>
    </div>
    <Button label="Save configuration" :disabled="!writable || !configId.trim() || !configSource.trim()" @click="saveConfig" />
    <p class="muted">OAuth sign-in is done locally with <code>memcastle source config auth &lt;id&gt;</code>; this page only shows its availability.</p>
  </section>
  <section class="panel">
    <h2>Configured miners</h2>
    <div v-for="miner in report?.miners ?? []" :key="miner.name" class="row">
      <Button :label="miner.name" text @click="select(miner.name)" />
      <span>{{ miner.source }} · {{ miner.state }}</span>
      <span v-if="miner.reason" class="muted">{{ miner.reason }}</span>
      <span v-if="miner.schedule">every {{ miner.schedule.every }} {{ miner.schedule.enabled ? "(on)" : "(off)" }}</span>
      <Button v-if="miner.schedule" :label="miner.schedule.enabled ? 'Stop schedule' : 'Enable schedule'" size="small" :disabled="!writable" @click="schedule(miner.name, !miner.schedule!.enabled)" />
      <Button label="Edit" size="small" outlined @click="select(miner.name)" />
      <Button label="Remove" size="small" severity="danger" outlined :disabled="!writable" @click="remove(miner.name)" />
    </div>
    <p v-if="!report?.miners.length" class="muted">No miners configured yet.</p>
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
      <div class="field"><label for="miner-every">Schedule interval (e.g. 1d)</label><InputText id="miner-every" v-model="edit.every" fluid /></div>
      <div class="field"><label for="miner-at">Schedule UTC time (e.g. 03:30)</label><InputText id="miner-at" v-model="edit.at" fluid /></div>
      <div class="field"><label for="miner-options">Source options (one KEY=VALUE per line)</label><textarea id="miner-options" v-model="edit.options" rows="4" /></div>
    </div>
    <label><input v-model="allowMinerBroaden" type="checkbox" /> Allow a broader input selection when changing this miner</label>
    <Button label="Save miner" :disabled="!writable || !id.trim() || !edit.source.trim()" @click="save" />
  </section>
</template>

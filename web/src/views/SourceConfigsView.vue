<script setup lang="ts">
import Button from "openvue/button"
import InputText from "openvue/inputtext"
import Message from "openvue/message"
import { onMounted, ref, watch } from "vue"
import { useRoute, useRouter } from "vue-router"
import type { SourceConfiguration, SourceConfigPatch } from "../api/types.ts"
import PageHeader from "../components/PageHeader.vue"
import { asApiError } from "../composables/useLoad.ts"
import { formatOptions, optionPatch } from "../optionEditor.ts"
import { useSession } from "../session.ts"

const { client } = useSession()
const route = useRoute()
const router = useRouter()
const configs = ref<SourceConfiguration[]>([])
const error = ref("")
const configId = ref("")
const source = ref("")
const optionsText = ref("")
const credentialKind = ref<"" | "oauth" | "env" | "file">("")
const credentialLocation = ref("")
const allowBroaden = ref(false)
const current = () => configs.value.find((config) => config.id === configId.value)

function createNew(): void {
  void router.push({ name: "source-configs" })
  configId.value = ""
  source.value = ""
  optionsText.value = ""
  credentialKind.value = ""
  credentialLocation.value = ""
}

function select(id: string): void {
  const config = configs.value.find((entry) => entry.id === id)
  if (!config) return
  configId.value = config.id
  source.value = config.source
  optionsText.value = formatOptions(config.options)
  // No credential location is sent by the daemon; leaving this empty preserves the existing reference.
  credentialKind.value = ""
  credentialLocation.value = ""
}

async function refresh(): Promise<void> {
  try {
    configs.value = await client.sourceConfigs()
    error.value = ""
    if (typeof route.query.config === "string") select(route.query.config)
  }
  catch (caught) { error.value = asApiError(caught).message }
}
onMounted(refresh)
watch(() => route.query.config, (id) => {
  if (typeof id === "string") select(id)
  else if (configId.value && configs.value.some((entry) => entry.id === configId.value)) {
    configId.value = ""; source.value = ""; optionsText.value = ""; credentialKind.value = ""; credentialLocation.value = ""
  }
})

async function save(): Promise<void> {
  try {
    const { options, unset_options } = optionPatch(optionsText.value, current()?.options ?? {})
    const patch: SourceConfigPatch = { source: source.value, options, unset_options, allow_broaden: allowBroaden.value }
    if (credentialKind.value === "oauth") patch.credential = { type: "oauth" }
    if (credentialKind.value === "env" && credentialLocation.value.trim()) patch.credential = { type: "env", name: credentialLocation.value.trim() }
    if (credentialKind.value === "file" && credentialLocation.value.trim()) patch.credential = { type: "file", path: credentialLocation.value.trim() }
    if ((credentialKind.value === "env" || credentialKind.value === "file") && !patch.credential) throw new Error("Give the credential reference location")
    const savedId = configId.value
    await client.setSourceConfig(savedId, patch)
    allowBroaden.value = false
    await refresh()
    await router.replace({ name: "source-configs", query: { config: savedId } })
  } catch (caught) { error.value = asApiError(caught).message }
}
</script>

<template>
  <PageHeader title="Source configurations" subtitle="Reusable source-native settings and authentication shared by miners." />
  <Message v-if="error" severity="error" :closable="false">{{ error }}</Message>
  <section class="panel">
    <h2>Named configurations</h2>
    <Button label="New configuration" size="small" severity="secondary" outlined @click="createNew" />
    <p v-if="!configs.length" class="muted">No named configurations yet. A source's default configuration uses its source ID.</p>
    <button v-for="config in configs" :key="config.id" type="button" class="list-item" :class="{ active: route.query.config === config.id }" :aria-current="route.query.config === config.id ? 'location' : undefined" @click="router.push({ name: 'source-configs', query: { config: config.id } })">
      {{ config.id }} <small>{{ config.source }} · {{ config.credential_kind ?? 'no credential' }} {{ config.credential_available ? '(available)' : '' }}</small>
    </button>
  </section>
  <section class="panel">
    <h2>{{ current() ? 'Edit' : 'Create' }} source configuration</h2>
    <div class="field-grid">
      <div class="field"><label for="config-id">Configuration ID</label><InputText id="config-id" v-model="configId" :readonly="!!current()" fluid /></div>
      <div class="field"><label for="config-source">Source</label><InputText id="config-source" v-model="source" fluid /></div>
      <div class="field"><label for="config-options">Source defaults (KEY=VALUE per line)</label><textarea id="config-options" v-model="optionsText" rows="3" /></div>
      <div class="field"><label for="config-credential">Credential reference</label><select id="config-credential" v-model="credentialKind"><option value="">Keep current</option><option value="oauth">OAuth</option><option value="env">Environment</option><option value="file">File</option></select></div>
      <div v-if="credentialKind === 'env' || credentialKind === 'file'" class="field"><label for="config-credential-location">{{ credentialKind === 'env' ? 'Environment variable name' : 'File path' }}</label><InputText id="config-credential-location" v-model="credentialLocation" fluid /></div>
      <label><input v-model="allowBroaden" type="checkbox" /> Allow a broader source selection for every miner using this configuration</label>
    </div>
    <Button label="Save configuration" :disabled="!configId.trim() || !source.trim()" @click="save" />
    <p class="muted">OAuth sign-in is done locally with <code>memcastle source config auth &lt;id&gt;</code>; this page only shows availability.</p>
  </section>
</template>

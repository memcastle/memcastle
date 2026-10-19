<script setup lang="ts">
import cytoscape, { type Core } from "cytoscape"
import Button from "openvue/button"
import InputText from "openvue/inputtext"
import Message from "openvue/message"
import SelectButton from "openvue/selectbutton"
import Tag from "openvue/tag"
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue"
import { useRouter } from "vue-router"
import type { Entity, GraphView, Mention } from "../api/types.ts"
import EmptyState from "../components/EmptyState.vue"
import ErrorNotice from "../components/ErrorNotice.vue"
import PageHeader from "../components/PageHeader.vue"
import { asApiError } from "../composables/useLoad.ts"
import { useEvents } from "../events.ts"
import { shortId, utc } from "../format.ts"
import { useSession } from "../session.ts"

// The knowledge graph: entities and the facts between them, each with where it was read from. One request per view
// of the graph (`GET /api/graph`), not one per entity.
const { client } = useSession()
const router = useRouter()
const events = useEvents()
let unsubscribe: (() => void) | undefined
let loadRequest = 0
let selectionRequest = 0
let searchRequest = 0

const canvas = ref<HTMLDivElement>()
const cy = shallowRef<Core>()
const view = ref<GraphView>()
const error = ref<ReturnType<typeof asApiError> | null>(null)
const busy = ref(false)

const search = ref("")
const matches = ref<Entity[]>([])
const searched = ref(false)
const center = ref<Entity>()
const depth = ref(1)
const selected = ref<Entity>()
const mentions = ref<Mention[]>()

const names = computed(() => new Map((view.value?.nodes ?? []).map((node) => [node.id, node.name])))
const selectedEdges = computed(() => (view.value?.edges ?? []).filter((edge) => edge.from === selected.value?.id || edge.to === selected.value?.id))

/** The same kind always gets the same colour, without a table to keep in step with the daemon's vocabulary. */
function colour(kind: string): string {
  let hash = 0
  for (const char of kind) hash = (hash * 31 + char.charCodeAt(0)) % 360
  return `hsl(${hash}, 55%, 52%)`
}

async function load(): Promise<void> {
  const request = ++loadRequest
  busy.value = true
  error.value = null
  try {
    const result = await client.graph({ entity: center.value?.id, depth: center.value ? depth.value : undefined, limit: 80 })
    if (request !== loadRequest) return
    view.value = result
    selected.value = undefined
    mentions.value = undefined
    ++selectionRequest
    await nextTick()
    if (request !== loadRequest) return
    draw()
  } catch (caught) {
    if (request === loadRequest) error.value = asApiError(caught)
  } finally {
    if (request === loadRequest) busy.value = false
  }
}

function draw(): void {
  if (!canvas.value || !view.value) return
  const text = getComputedStyle(document.documentElement).getPropertyValue("--p-text-color").trim() || "#444"
  cy.value?.destroy()
  cy.value = cytoscape({
    container: canvas.value,
    elements: [
      ...view.value.nodes.map((node) => ({ data: { id: node.id, label: node.name, colour: colour(node.kind), centre: node.id === center.value?.id ? 1 : 0 } })),
      ...view.value.edges.map((edge) => ({ data: { id: edge.id, source: edge.from, target: edge.to, label: edge.predicate } })),
    ],
    style: [
      { selector: "node", style: { label: "data(label)", "background-color": "data(colour)", color: text, "font-size": 11, "text-valign": "bottom", "text-margin-y": 4 } },
      { selector: "node[centre = 1]", style: { "border-width": 3, "border-color": text } },
      { selector: "edge", style: { label: "data(label)", "font-size": 9, color: text, width: 1.5, "line-color": "#999", "target-arrow-color": "#999", "target-arrow-shape": "triangle", "curve-style": "bezier" } },
    ],
    layout: { name: "concentric", animate: false },
  })
  cy.value.on("tap", "node", (event) => {
    const node = view.value?.nodes.find((candidate) => candidate.id === event.target.id())
    if (node) void select(node)
  })
}

async function select(node: Entity): Promise<void> {
  const request = ++selectionRequest
  selected.value = node
  mentions.value = undefined
  try {
    const result = await client.mentions(node.id)
    if (request === selectionRequest) mentions.value = result
  } catch (caught) {
    if (request === selectionRequest) error.value = asApiError(caught)
  }
}

async function find(): Promise<void> {
  const request = ++searchRequest
  error.value = null
  searched.value = false
  try {
    const result = await client.entities({ name: search.value.trim() || undefined, limit: 10 })
    if (request === searchRequest) { matches.value = result; searched.value = true }
  } catch (caught) {
    if (request === searchRequest) error.value = asApiError(caught)
  }
}

function focus(entity?: Entity): void {
  center.value = entity
  matches.value = []
  searched.value = false
  void load()
}

async function openEvidence(id: string): Promise<void> {
  try {
    const location = await client.drawerLocation(id)
    await router.push({ name: "palace", query: location })
  } catch (caught) { error.value = asApiError(caught) }
}

watch(depth, () => center.value && void load())
onMounted(() => { void load(); unsubscribe = events?.subscribe(["entity"], () => void load()) })
onBeforeUnmount(() => { ++loadRequest; ++selectionRequest; ++searchRequest; unsubscribe?.(); cy.value?.destroy() })
</script>

<template>
  <PageHeader title="Graph" subtitle="Entities and the facts between them, with where each one was read from.">
    <Button label="Refresh graph" size="small" severity="secondary" :loading="busy" @click="load" />
  </PageHeader>
  <form class="toolbar" @submit.prevent="find">
    <InputText v-model="search" placeholder="Find an entity by name" aria-label="Entity name" class="graph-search" />
    <Button type="submit" label="Find" severity="secondary" size="small" />
    <template v-if="center">
      <span class="muted">Around <strong>{{ center.name }}</strong>, hops</span>
      <SelectButton v-model="depth" :options="[1, 2, 3]" :allow-empty="false" size="small" />
      <Button label="Show overview" size="small" text @click="focus(undefined)" />
    </template>
  </form>
  <div v-if="matches.length" class="panel">
    <button v-for="match in matches" :key="match.id" type="button" class="list-item" @click="focus(match)">
      {{ match.name }} <small>{{ match.kind }}</small>
    </button>
  </div>
  <p v-else-if="searched" role="status">No entities match that name.</p>
  <ErrorNotice :error="error" @retry="load" />
  <p v-if="busy && !view" role="status">Loading graph…</p>
  <Message v-if="view?.truncated" severity="info" :closable="false">The overview shows the first {{ view.nodes.length }} entities by name. Find an entity to see its neighbourhood.</Message>
  <EmptyState v-if="view && !view.nodes.length" title="The knowledge graph is empty" hint="Entities appear once an extraction job has read mined drawers, or a checkpoint has recorded facts." />

  <div class="split graph-split" :style="{ display: view?.nodes.length ? undefined : 'none' }">
    <div ref="canvas" class="graph-canvas" role="img" aria-label="Knowledge graph" />
    <section class="panel">
      <h2>Entities in view</h2>
      <div class="graph-entities" role="group" aria-label="Entities in view">
        <button v-for="node in view?.nodes ?? []" :key="node.id" type="button" class="list-item" :class="{ active: selected?.id === node.id }" :aria-pressed="selected?.id === node.id" @click="select(node)">{{ node.name }} <small>{{ node.kind }}</small></button>
      </div>
      <h2>{{ selected?.name ?? "Select an entity" }}</h2>
      <template v-if="selected">
        <p class="muted"><Tag :value="selected.kind" severity="secondary" /> <span v-if="selected.aliases.length">also {{ selected.aliases.join(", ") }}</span></p>
        <Button label="Centre the graph here" size="small" severity="secondary" outlined @click="focus(selected)" />
        <h2 style="margin-top: 1rem">Facts</h2>
        <p v-if="!selectedEdges.length" class="muted">None in view.</p>
        <div v-for="edge in selectedEdges" :key="edge.id" class="hit-meta">
          <span>{{ names.get(edge.from) ?? shortId(edge.from) }}</span><Tag :value="edge.predicate" /><span>{{ names.get(edge.to) ?? shortId(edge.to) }}</span>
          <span v-if="edge.provenance">read by {{ edge.provenance.extractor }} · {{ utc(edge.provenance.extracted_at) }}</span>
          <button v-if="edge.provenance?.drawer" type="button" class="evidence-link" @click="openEvidence(edge.provenance.drawer)">Read evidence {{ shortId(edge.provenance.drawer) }}</button>
        </div>
        <h2 style="margin-top: 1rem">Mentioned in</h2>
        <p v-if="mentions && !mentions.length" class="muted">No drawer.</p>
        <div v-for="mention in mentions" :key="mention.drawer" class="hit-meta"><button type="button" class="evidence-link" @click="openEvidence(mention.drawer)">Read {{ shortId(mention.drawer) }}</button><span>{{ utc(mention.created_at) }}</span></div>
      </template>
      <p v-else class="muted">Select an entity in the list or graph to see its facts and evidence.</p>
    </section>
  </div>
</template>

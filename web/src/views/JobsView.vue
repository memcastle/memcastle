<script setup lang="ts">
import Button from "openvue/button"
import Column from "openvue/column"
import DataTable from "openvue/datatable"
import SelectButton from "openvue/selectbutton"
import { useToast } from "openvue/usetoast"
import { computed, ref, watch } from "vue"
import { useRoute, useRouter } from "vue-router"
import type { Job, JobControl, JobStatus } from "../api/types.ts"
import EmptyState from "../components/EmptyState.vue"
import DetailDrawer from "../components/DetailDrawer.vue"
import ErrorNotice from "../components/ErrorNotice.vue"
import JobProgress from "../components/JobProgress.vue"
import RefreshButton from "../components/RefreshButton.vue"
import PageHeader from "../components/PageHeader.vue"
import StatusTag from "../components/StatusTag.vue"
import LaunchDialog from "../components/LaunchDialog.vue"
import { asApiError, useLoad, type Loaded } from "../composables/useLoad.ts"
import { ago, jobDuration, jobTarget, shortId, utc } from "../format.ts"
import { useSession } from "../session.ts"

const { client } = useSession()
const toast = useToast()
const route = useRoute()
const router = useRouter()

const ALL = "all"
const states = [ALL, "queued", "running", "paused", "completed", "failed", "cancelled"]
const state = ref<string>(ALL)
const kind = ref<string>(ALL)
const selected = ref<Job>()
const detailError = ref("")
const launching = computed(() => route.query.launch === "1")
const selectedId = computed(() => typeof route.query.job === "string" ? route.query.job : undefined)
const jobs: Loaded<Job[]> = useLoad(() => client.jobs({ status: state.value === ALL ? undefined : (state.value as JobStatus), limit: 200 }), { on: ["job"] })
watch(state, () => void jobs.refresh())
// The URL identifies the inspected job even when it is filtered out or older than the first page.
let detailRequest = 0
watch([selectedId, jobs.data], async ([id, list]) => {
  const request = ++detailRequest
  detailError.value = ""
  if (!id) { selected.value = undefined; return }
  const fresh = list?.find((job) => job.id === id)
  if (fresh) { selected.value = fresh; return }
  if (selected.value?.id !== id) selected.value = undefined
  try {
    const job = await client.job(id)
    if (request === detailRequest) selected.value = job
  } catch (caught) {
    if (request === detailRequest) detailError.value = asApiError(caught).message
  }
}, { immediate: true })

function queryWith(patch: Record<string, string | undefined>): void {
  const query = { ...route.query, ...patch }
  for (const key of Object.keys(query)) if (query[key] === undefined) delete query[key]
  void router.push({ name: "jobs", query })
}
function selectJob(job: Job | null): void { if (job) queryWith({ job: job.id }) }

const kinds = computed(() => [ALL, ...new Set((jobs.data.value ?? []).map((job) => job.kind.type))])
const visible = computed(() => (jobs.data.value ?? []).filter((job) => kind.value === ALL || job.kind.type === kind.value))
const active = computed(() => visible.value.filter((job) => ["queued", "running", "paused"].includes(job.status)))
const history = computed(() => visible.value.filter((job) => !["queued", "running", "paused"].includes(job.status)))

/** What a job's state allows, so the buttons are the transitions the daemon would accept. */
function actions(job: Job): JobControl[] {
  switch (job.status) {
    case "queued":
      return ["cancel"]
    case "running":
      return ["pause", "cancel"]
    case "paused":
      return ["resume", "cancel"]
    case "failed":
      return ["retry"]
    default:
      return []
  }
}

async function control(job: Job, action: JobControl): Promise<void> {
  try {
    await client.controlJob(job.id, action)
    toast.add({ severity: "success", summary: `Job ${shortId(job.id)}: ${action} requested`, life: 3000 })
    await jobs.refresh()
  } catch (caught) {
    const error = asApiError(caught)
    toast.add({ severity: "error", summary: error.message, detail: error.help, life: 6000 })
  }
}
</script>

<template>
  <PageHeader title="Jobs" subtitle="Durable jobs and their progress. They survive a daemon restart.">
    <Button label="Launch" @click="queryWith({ launch: '1' })" />
    <RefreshButton :loading="jobs.loading.value" :updated-at="jobs.updatedAt.value" @refresh="jobs.refresh()" />
  </PageHeader>
  <div class="resource-layout" :class="{ 'has-detail': !!selectedId }">
    <div class="resource-list">
  <div class="toolbar"><span class="muted">State</span><SelectButton v-model="state" :options="states" :allow-empty="false" size="small" /></div>
  <div class="toolbar"><span class="muted">Kind</span><SelectButton v-model="kind" :options="kinds" :allow-empty="false" size="small" /></div>
  <ErrorNotice :error="jobs.error.value" @retry="jobs.refresh()" />

  <h2>Active <span class="muted">{{ active.length }}</span></h2>
  <div class="table-scroll">
    <DataTable :value="active" data-key="id" size="small" class="panel-table" selection-mode="single" :selection="selected" @update:selection="selectJob">
      <template #empty><EmptyState title="No active jobs" /></template>
      <Column header="ID"><template #body="{ data }"><code>{{ shortId(data.id) }}</code></template></Column>
      <Column header="Kind"><template #body="{ data }">{{ data.kind.type }}</template></Column>
      <Column header="State"><template #body="{ data }"><StatusTag :status="data.status" /></template></Column>
      <Column header="Target"><template #body="{ data }">{{ jobTarget(data.kind) }}</template></Column>
      <Column header="Progress"><template #body="{ data }"><JobProgress :job="data" /></template></Column>
      <Column header="Created"><template #body="{ data }">{{ ago(data.created_at) }}</template></Column>
      <Column header="">
        <template #body="{ data }">
          <div class="row">
            <Button
              v-for="action in actions(data)"
              :key="action"
              :label="action"
              size="small"
              :severity="action === 'cancel' ? 'danger' : 'secondary'"
              outlined
              @click.stop="control(data, action)"
            />
          </div>
        </template>
      </Column>
    </DataTable>
  </div>

  <h2 style="margin-top: 1.5rem">History <span class="muted">{{ history.length }}</span></h2>
  <div class="table-scroll">
    <DataTable :value="history" data-key="id" size="small" paginator :rows="15" :rows-per-page-options="[15, 50, 100]" selection-mode="single" :selection="selected" @update:selection="selectJob">
      <template #empty><EmptyState title="No finished jobs yet" /></template>
      <Column header="ID"><template #body="{ data }"><code>{{ shortId(data.id) }}</code></template></Column>
      <Column header="Kind"><template #body="{ data }">{{ data.kind.type }}</template></Column>
      <Column header="State"><template #body="{ data }"><StatusTag :status="data.status" /></template></Column>
      <Column header="Target"><template #body="{ data }">{{ jobTarget(data.kind) }}</template></Column>
      <Column header="Progress"><template #body="{ data }"><JobProgress :job="data" /></template></Column>
      <Column header="Created"><template #body="{ data }">{{ ago(data.created_at) }}</template></Column>
      <Column header="Duration"><template #body="{ data }">{{ jobDuration(data) }}</template></Column>
      <Column header="Attempts" field="attempt" />
      <Column header="">
        <template #body="{ data }">
          <div class="row">
            <Button v-for="action in actions(data)" :key="action" :label="action" size="small" severity="secondary" outlined @click.stop="control(data, action)" />
          </div>
        </template>
      </Column>
    </DataTable>
  </div>
    </div>
  <DetailDrawer v-if="selectedId" :title="`Job ${shortId(selectedId)}`" :selection-key="selectedId" @close="queryWith({ job: undefined })">
    <p v-if="detailError" role="alert">{{ detailError }}</p>
    <p v-else-if="!selected" role="status">Loading job details…</p>
    <template v-else>
      <dl class="facts" style="margin-bottom: 1rem">
        <div><dt>Id</dt><dd class="mono">{{ selected.id }}</dd></div>
        <div><dt>State</dt><dd><StatusTag :status="selected.status" /></dd></div>
        <div><dt>Requested by</dt><dd>{{ selected.requested_by }}</dd></div>
        <div><dt>Created</dt><dd>{{ utc(selected.created_at) }}</dd></div>
        <div><dt>Started</dt><dd>{{ utc(selected.started_at) }}</dd></div>
        <div><dt>Completed</dt><dd>{{ utc(selected.completed_at) }}</dd></div>
      </dl>
      <h2>Progress</h2>
      <JobProgress :job="selected" />
      <p v-if="selected.error" class="error-body"><strong>Failed:</strong> {{ selected.error }}</p>
      <h2>Parameters</h2>
      <pre class="drawer-content">{{ JSON.stringify(selected.kind, null, 2) }}</pre>
      <template v-if="selected.result">
        <h2 style="margin-top: 1rem">Result</h2>
        <pre class="drawer-content">{{ JSON.stringify(selected.result, null, 2) }}</pre>
      </template>
    </template>
  </DetailDrawer>
  </div>
  <LaunchDialog :visible="launching" @close="queryWith({ launch: undefined })" @submitted="(job: Job) => { queryWith({ launch: undefined, job: job.id }); void jobs.refresh() }" />
</template>

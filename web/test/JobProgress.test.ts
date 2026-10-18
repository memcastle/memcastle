import { mount } from "@vue/test-utils"
import { describe, expect, it } from "vitest"
import type { Job } from "../src/api/types.ts"
import JobProgress from "../src/components/JobProgress.vue"

const job = (current: number, total: number | null, status: Job["status"] = "running", message: string | null = null) => ({
  status,
  progress: { current, total, message },
}) as Job

describe("job progress", () => {
  it("shows an exact count and percentage to sighted and screen reader users", async () => {
    const wrapper = mount(JobProgress, { props: { job: job(128, 500, "running", "Reading sources") } })
    const bar = wrapper.get('[role="progressbar"]')

    expect(bar.attributes("aria-valuenow")).toBe("26")
    expect(bar.attributes("aria-valuetext")).toBe("26% · 128 / 500")
    expect(wrapper.get(".job-progress").attributes("title")).toBe("26% · 128 / 500")
    expect(wrapper.get(".job-progress").attributes("tabindex")).toBe("0")
    expect(wrapper.text()).toContain("128 / 500")
    expect(wrapper.text()).toContain("Reading sources")

    await wrapper.setProps({ job: job(400, 500) })
    expect(wrapper.get('[role="progressbar"]').attributes("aria-valuenow")).toBe("80")
    expect(wrapper.get(".job-progress").attributes("title")).toBe("80% · 400 / 500")
  })

  it("clamps only the bar when a job discovers more work than estimated", () => {
    const wrapper = mount(JobProgress, { props: { job: job(7, 5) } })

    expect(wrapper.get('[role="progressbar"]').attributes("aria-valuenow")).toBe("100")
    expect(wrapper.get('[role="progressbar"]').attributes("aria-valuetext")).toBe("100% · 7 / 5")
  })

  it.each([null, 0])("keeps a running job with total %s indeterminate", (total) => {
    const wrapper = mount(JobProgress, { props: { job: job(3, total) } })
    const bar = wrapper.get('[role="progressbar"]')

    expect(bar.attributes("aria-valuenow")).toBeUndefined()
    expect(bar.attributes("aria-valuetext")).toContain("total unknown")
    expect(wrapper.text()).not.toContain("%")
    expect(wrapper.text()).toContain("3 processed · total unknown")
  })

  it.each(["queued", "paused", "completed", "failed", "cancelled"] as Job["status"][])("does not animate an unknown-total %s job", (status) => {
    const wrapper = mount(JobProgress, { props: { job: job(3, null, status, "Last known step") } })

    expect(wrapper.find('[role="progressbar"]').exists()).toBe(false)
    expect(wrapper.text()).toContain("3 processed · total unknown")
    expect(wrapper.text()).toContain("Last known step")
  })
})

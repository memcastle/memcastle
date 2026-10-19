import { spawnSync } from "node:child_process"
import { existsSync, readFileSync } from "node:fs"
import { describe, expect, it } from "vitest"

const chromium = ["/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/google-chrome"].find(existsSync)
// Hosted CI's installed Chromium can hang on startup under the coverage runner; run the browser check explicitly there.
const browserAvailable = !!chromium && (!process.env.CI || process.env.MEMCASTLE_BROWSER_TESTS === "1")

// Happy DOM cannot evaluate viewport media rules; check the shipped CSS in a real rendering engine when available.
describe.skipIf(!browserAvailable)("the details panel at browser viewport widths", () => {
  const stylesheet = readFileSync("src/styles.css", "utf8")
  function layout(width: number): { columns: string; list: number; panel: number; listTop: number; panelTop: number } {
    const page = `<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><style>${stylesheet}</style>
      <div class="resource-layout has-detail"><div class="resource-list">Jobs list</div><aside class="detail-drawer panel">Details <button>Close</button></aside></div>
      <output id="layout"></output><script>
      const list = document.querySelector('.resource-list').getBoundingClientRect();
      const panel = document.querySelector('.detail-drawer').getBoundingClientRect();
      document.querySelector('#layout').textContent = JSON.stringify({
        columns: getComputedStyle(document.querySelector('.resource-layout')).gridTemplateColumns,
        list: list.width, panel: panel.width, listTop: list.top, panelTop: panel.top
      });</script>`
    const result = spawnSync(chromium!, ["--headless", "--no-sandbox", "--disable-gpu", `--window-size=${width},800`, "--dump-dom", `data:text/html;charset=utf-8,${encodeURIComponent(page)}`], { encoding: "utf8", timeout: 15_000 })
    if (result.error) throw result.error
    expect(result.status).toBe(0)
    const match = result.stdout.match(/<output id="layout">([^<]+)<\/output>/)
    expect(match).not.toBeNull()
    return JSON.parse(match![1]!.replaceAll("&quot;", '"'))
  }

  it("keeps a usable list beside details on desktop and stacks details above it on a phone", () => {
    const desktop = layout(1440)
    expect(desktop.columns.split(" ")).toHaveLength(2)
    expect(desktop.list).toBeGreaterThan(200)
    expect(desktop.panel).toBeGreaterThan(200)
    const phone = layout(600)
    expect(phone.columns.split(" ")).toHaveLength(1)
    expect(phone.list).toBeGreaterThan(200)
    expect(phone.panelTop).toBeLessThan(phone.listTop)
  }, 35_000)

  it("bounds the Palace tree and drawer list while keeping the graph canvas visible", () => {
    const page = `<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><style>${stylesheet}</style>
      <div class="split"><section class="panel"><div class="palace-tree">${'<button class="list-item">Room</button>'.repeat(150)}</div></section>
      <section class="panel"><div class="palace-drawers">${'<div class="hit">Drawer</div>'.repeat(150)}</div></section></div>
      <div class="split graph-split"><div class="graph-canvas"></div><section class="panel">Entity facts</section></div>
      <output id="layout"></output><script>
      const tree = document.querySelector('.palace-tree'); const drawers = document.querySelector('.palace-drawers');
      const canvas = document.querySelector('.graph-canvas');
      document.querySelector('#layout').textContent = JSON.stringify({ tree: tree.clientHeight, treeScroll: tree.scrollHeight,
        drawers: drawers.clientHeight, drawerScroll: drawers.scrollHeight, canvas: canvas.getBoundingClientRect().width });</script>`
    const result = spawnSync(chromium!, ["--headless", "--no-sandbox", "--disable-gpu", "--window-size=650,800", "--dump-dom", `data:text/html;charset=utf-8,${encodeURIComponent(page)}`], { encoding: "utf8", timeout: 15_000 })
    expect(result.status).toBe(0)
    const match = result.stdout.match(/<output id="layout">([^<]+)<\/output>/)
    expect(match).not.toBeNull()
    const measured = JSON.parse(match![1]!.replaceAll("&quot;", '"'))
    expect(measured.treeScroll).toBeGreaterThan(measured.tree)
    expect(measured.drawerScroll).toBeGreaterThan(measured.drawers)
    expect(measured.canvas).toBeGreaterThan(250)
  }, 20_000)
})

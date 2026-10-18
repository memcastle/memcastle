import { readFileSync } from "node:fs"
import { describe, expect, it } from "vitest"

const styles = readFileSync("src/styles.css", "utf8")

describe("fluid layout contracts", () => {
  it("lets the shell and main content shrink and fill wide viewports without a global cap", () => {
    expect(styles).toMatch(/\.shell\s*\{[^}]*grid-template-columns:\s*15rem minmax\(0, 1fr\)/)
    expect(styles).toMatch(/\.content\s*\{[^}]*min-width:\s*0;[^}]*width:\s*100%/)
    expect(styles.match(/\.content\s*\{[^}]*\}/)?.[0]).not.toContain("max-width")
  })

  it("stacks both splits and narrows the padding at phone widths", () => {
    expect(styles).toMatch(/@media \(max-width: 52rem\)\s*\{[^}]*\.shell[^}]*\}[^}]*\.sidebar[^}]*\}[^}]*\.split, \.graph-split\s*\{\s*grid-template-columns:\s*minmax\(0, 1fr\)/)
    expect(styles).toMatch(/@media \(max-width: 36rem\)\s*\{\s*\.content\s*\{\s*padding:\s*1rem 0\.75rem 2rem/)
  })

  it("keeps wide tables scrolling locally and cards and form fields within their container", () => {
    expect(styles).toMatch(/\.table-scroll\s*\{[^}]*overflow-x:\s*auto/)
    expect(styles).toMatch(/\.maintenance-cards\s*\{[^}]*minmax\(min\(100%, 22rem\), 1fr\)/)
    expect(styles).toMatch(/\.field-grid\s*\{[^}]*minmax\(min\(100%, 16rem\), 1fr\)/)
  })
})

export type SourceOption = string | number | boolean | string[]

/** Existing typed options stay typed; new lines are source-native strings unless explicitly JSON-typed. */
export function formatOptions(options: Record<string, SourceOption>): string {
  return Object.entries(options).map(([key, value]) => `${key}=${typeof value === "string" ? value : JSON.stringify(value)}`).join("\n")
}

export function optionPatch(text: string, previous: Record<string, SourceOption>): { options: Record<string, SourceOption>; unset_options: string[] } {
  const options: Record<string, SourceOption> = {}
  const present = new Set<string>()
  for (const line of text.split("\n").filter((value) => value.trim())) {
    const index = line.indexOf("=")
    if (index < 1 || !line.slice(0, index).trim()) throw new Error(`Use KEY=VALUE for each option: ${line}`)
    const key = line.slice(0, index).trim()
    if (present.has(key)) throw new Error(`Option ${key} appears twice`)
    present.add(key)
    const value = line.slice(index + 1).trim()
    const old = previous[key]
    if (old !== undefined && formatOptions({ [key]: old }) === `${key}=${value}`) continue
    if (typeof old === "boolean" || typeof old === "number" || Array.isArray(old)) {
      let parsed: unknown
      try { parsed = JSON.parse(value) }
      catch { throw new Error(`Option ${key} needs a valid ${Array.isArray(old) ? "list" : typeof old} value`) }
      if (Array.isArray(old) ? !Array.isArray(parsed) || !parsed.every((entry) => typeof entry === "string")
        : typeof parsed !== typeof old || typeof parsed === "number" && !Number.isFinite(parsed)) {
        throw new Error(`Option ${key} needs a ${Array.isArray(old) ? "list of strings" : typeof old} value`)
      }
      options[key] = parsed as SourceOption
    } else options[key] = value
  }
  return { options, unset_options: Object.keys(previous).filter((key) => !present.has(key)) }
}

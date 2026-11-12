// A page file as it runs: its types taken out, as scripts/build-pages.mjs
// does (Node's own stripping: lines and columns kept). Node says each time
// that this is experimental; said once here, not by every test.
import { readFileSync } from 'node:fs'
import { stripTypeScriptTypes } from 'node:module'
import path from 'node:path'

export function stripped(code: string): string {
  const said = process.emitWarning
  process.emitWarning = () => {}
  try {
    return stripTypeScriptTypes(code, { mode: 'strip' })
  } finally {
    process.emitWarning = said
  }
}

// src/<parts...> as it runs, its lines ending in \n.
export const pageCode = (...parts: string[]) => stripped(readFileSync(path.join(import.meta.dirname, '..', 'src', ...parts), 'utf8')).replace(/\r\n/g, '\n')

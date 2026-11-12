// What the pages' entries (main.ts, desk-main.ts) share.

// Each part of a page made apart, as each was a script of its own: one that
// fails says so (an error the page did not catch, in the console and
// debug.log) and is none; the rest are made all the same, without it.
export function made<T>(make: () => T): T | null {
  try {
    return make()
  } catch (err) {
    reportError(err)
    return null
  }
}

// What a page's parts hand back, put on window for what looks in from
// outside alone (the preview's scenes and playground, /debug/eval, the test
// VM's scripts); the pages' own scripts never read them there
// (test/scripts.test.ts).
export const expose = (faces: Record<string, unknown>) => Object.assign(window, faces)

// What the page still has entered and the pointer is no longer on, for
// bridge.ts to play its mouseleave. The page hears mouseenter and mouseleave
// two ways: WebView2's own (the window caught) and played by bridge.ts from
// where the Rust side says the pointer is (the window letting clicks
// through, pointer.rs). Entered one way, the leave may never come the other:
// a mouseenter played, WebView2 had seen no move over her, and its first
// move was already off her, so it had nothing to leave (the taskbar's island
// stayed open after a hover, 2026-10-16). Imported by bridge.ts and the
// tests.
// Caught, WebView2 hit-tests the pointer itself, a pixel or so from where
// the Rust side read it a moment before: an element is left only once the
// pointer is this far (CSS px) outside its box too, never at its edge,
// where the two could disagree and the window's mouse mode bounce.
const EDGE_PX = 2

// entered: elements in the order they were entered; under: the chain of
// elements at the pointer (outermost first; [] once it has gone); at: where
// it is ({ x, y, caught }, null once gone); boxOf: an element's box
// ({ left, top, right, bottom }). The elements to leave, innermost first
// (as a browser leaves them).
function stale(entered, under, at, boxOf) {
  const out = []
  for (const el of entered) {
    if (under.includes(el)) continue
    if (at?.caught) {
      const r = boxOf(el)
      if (r && at.x >= r.left - EDGE_PX && at.x <= r.right + EDGE_PX && at.y >= r.top - EDGE_PX && at.y <= r.bottom + EDGE_PX) continue
    }
    out.push(el)
  }
  return out.reverse()
}

export { stale, EDGE_PX }

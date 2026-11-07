// Where her island sits at the top of the screen (zhoupengjie/Wakuwaku#59):
// the top centre, as ever, or a top corner (the islandPos setting: left,
// center, right). At a corner it keeps as far from the screen's side as from
// its top (island.js TOP, 8 px), and opens downward and toward the screen's
// middle: from the left corner it grows to the right, from the right corner
// to the left. What hangs under it follows it (the picker, the capsule's
// panel, the toasts: picker.css, capsule-panel.js), the ring and the dot are
// laid over it anyway (style.css #flow), and it slides up out of the way of
// a window under it wherever it is.
//
// Or along the foot of the screen (the islandEdge setting: top, bottom;
// zhoupengjie/Wakuwaku#105): laid over Windows' own taskbar, which stays as
// it is, the closed island centred in its strip (foot), and it opens upward;
// what hung under it stands above it. With no taskbar along the foot, SIDE
// up from the work area's foot, as from the top. Or, there, the closed
// capsule inside the taskbar itself, in a window of its own taking one end
// of it (embed.rs, islandEmbed): as wide as set while its room (a quarter
// of the strip) allows, else narrow, else her portrait alone (capsuleWidth).
//
// The page lays the island out in its window by the anchor island.rs placed
// that window at (pet:update's islandAt, islandEdge): the window at the
// screen's top left, top right, or top centre, as wide as ever (760,
// island.rs ISLAND), the island in it at that side; or at the foot, as tall
// as the taskbar's strip and room (island.rs ISLAND_BOTTOM), the island at
// its foot. None said (an older island.rs), the top centre. In the
// playground, fake-tauri.js says it as island.rs does (tools/preview).
//
// Only the sums, so the tests can say them: island.js and the panels use
// them. Loaded by the page before island.js, and required by the tests.
;(function (root) {
  const POSITIONS = ['left', 'center', 'right']
  const EDGES = ['top', 'bottom']
  // From the screen's side at a corner: as far as from its top (island.js TOP).
  const SIDE = 8
  // The closed island's height (island.js), centred in a taskbar's strip under it.
  const CAPSULE = 36

  // The setting as kept (settings.rs is_island_pos): anything else, the centre.
  const posOf = v => (POSITIONS.includes(v) ? v : 'center')
  // The edge as kept (settings.rs is_island_edge): anything else, the top.
  const edgeOf = v => (v === 'bottom' ? 'bottom' : 'top')

  // The anchor the page lays the island by: where island.rs put its window
  // (at), at the top of the screen; on the taskbar, or round a Mac's notch
  // (only ever in the middle), none: the centre.
  function anchorOf({ home, at, notch = false } = {}) {
    if (home !== 'island' || notch) return 'center'
    return posOf(at)
  }

  // The edge the page lays the island along: where island.rs put its window
  // (edge), the top or the bottom of the screen; on the taskbar (its strip
  // has its own edge), or round a Mac's notch, the top.
  function edgeAt({ home, edge, notch = false } = {}) {
    if (home !== 'island' || notch) return 'top'
    return edgeOf(edge)
  }

  // Along the foot of the screen, how far up from the window's foot the
  // closed island lies: centred in the strip of Windows' own taskbar under
  // it (strip: its thickness in the page's px; Windows 11's 48: 6), on the
  // foot itself under one thinner than the island; SIDE with none there.
  function foot(strip) {
    return strip > 0 ? Math.max(0, (strip - CAPSULE) / 2) : SIDE
  }

  // Where the island lies at a new size (width, height) from where it lies
  // (r: a DOMRect's left, top, width, height): its top kept, or along the
  // foot of the screen its bottom; about its middle at the centre, its
  // anchored side kept at a corner (it grows toward the middle).
  function grown(r, width, height, anchor, edge = 'top') {
    const x = anchor === 'left' ? r.left : anchor === 'right' ? r.left + r.width - width : r.left + (r.width - width) / 2
    const y = edge === 'bottom' ? r.top + r.height - height : r.top
    return { x, y, width, height }
  }

  // The left of what is w wide hung under the island (r: its box; the
  // capsule's panel): under its middle at the centre, lined up with its
  // anchored side at a corner; kept within the window (vw) by margin.
  function under(r, w, anchor, vw, margin = SIDE) {
    const x = anchor === 'left' ? r.left : anchor === 'right' ? r.left + r.width - w : r.left + r.width / 2 - w / 2
    return Math.max(margin, Math.min(x, vw - w - margin))
  }

  // The window's left on the screen (logical px) for an anchor, as island.rs
  // puts it (fit): the work area's side at a corner, else its middle.
  // What the playground and the gallery place the island's page by.
  function windowLeft(anchor, area, winWidth) {
    const [x, , w] = area
    if (anchor === 'left') return x
    if (anchor === 'right') return x + w - winWidth
    return x + (w - winWidth) / 2
  }

  // And its top: the work area's top, or along the foot of the screen its
  // foot on the screen's bottom edge, strip below the work area's foot
  // (Windows' taskbar there; 0 with none, the work area's foot).
  function windowTop(edge, area, winHeight, strip = 0) {
    const [, y, , h] = area
    return edge === 'bottom' ? y + h + strip - winHeight : y
  }

  // The closed capsule's width (logical) within the room it has in Windows'
  // taskbar (island.rs islandRoom, embed.rs room_of: a quarter of the strip;
  // 0 or less: no limit). As set (set: island.js WIDTHS narrow, normal or
  // wide) while that fits; else the narrowest of widths; else 0, her
  // portrait alone in a round capsule (island.js TIGHT_W). Whether even
  // that fits is embed.rs's call (the capsule hidden else).
  function capsuleWidth(set, room, widths = {}) {
    if (!(room > 0) || set <= room) return set
    const narrow = Math.min(set, ...Object.values(widths).filter(w => w > 0))
    return narrow <= room ? narrow : 0
  }

  const api = { POSITIONS, EDGES, SIDE, CAPSULE, posOf, edgeOf, anchorOf, edgeAt, foot, grown, under, windowLeft, windowTop, capsuleWidth }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
  } else {
    root.Anchor = api
  }
})(this ?? window)

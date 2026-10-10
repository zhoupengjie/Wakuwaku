// The taskbar's Mica (island.js): the desktop's picture under the strip,
// blurred and tinted as Windows' own Mica is. Mica takes the wallpaper, not
// what is behind the window, and nothing but the wallpaper is behind the
// taskbar anyway (windows keep out of its strip). So the picture is drawn
// once, small, whenever it changes (taskbar.rs sends which file and how
// Windows lays it on the display), and shown as it is: nothing is worked out
// again as the strip draws.
;(function (root) {
  // How blurred (the strip's px), the scale it is drawn at (blurred, more
  // would not be seen), and the margin round the strip taken in, so its edges
  // are blurred with what lies beyond them.
  const BLUR = 36
  const SCALE = 0.25
  const MARGIN = 2 * BLUR
  // The tint, as Windows' dark Mica has it: the picture's lightness taken
  // to the tint's, keeping its hues (so any wallpaper is as dark), then the
  // tint itself over it, part of the way.
  const TINT = '#202020'
  const LUMINOSITY = 0.8
  const TINT_OPACITY = 0.45
  // A new picture fades in over the old.
  const FADE_MS = 400

  // Where the picture lies, in the strip's px (paper.mon: the display,
  // paper.screen: all of them, from the strip's top-left), as Windows lays
  // it: filling the display (cut to fit), fitting in it, stretched to it, in
  // its middle at its own size, tiled from its corner, or filling all the
  // displays together (span). natural: its size in pixels.
  function place(paper, natural) {
    const [mx, my, mw, mh] = paper.mon
    const sf = paper.sf || 1
    const { w: iw, h: ih } = natural
    const centred = ([bx, by, bw, bh], w, h) => ({ x: bx + (bw - w) / 2, y: by + (bh - h) / 2, w, h })
    switch (paper.position) {
      case 'stretch':
        return { x: mx, y: my, w: mw, h: mh }
      case 'fit': {
        const s = Math.min(mw / iw, mh / ih)
        return centred(paper.mon, iw * s, ih * s)
      }
      case 'center':
        return centred(paper.mon, iw / sf, ih / sf)
      case 'tile':
        return { x: mx, y: my, w: iw / sf, h: ih / sf, tile: true }
      case 'span': {
        const [, , sw, sh] = paper.screen
        const s = Math.max(sw / iw, sh / ih)
        return centred(paper.screen, iw * s, ih * s)
      }
      default: {
        const s = Math.max(mw / iw, mh / ih)
        return centred(paper.mon, iw * s, ih * s)
      }
    }
  }

  // The strip's Mica, width × height and the margin round it, in a canvas
  // drawn at SCALE: the colour round the picture, the picture where it lies,
  // the display's edges carried on past them (below the strip, beyond its
  // ends: blurred with nothing there, they would darken), blurred, tinted.
  // img: the picture, loaded (none: the colour alone).
  function draw(paper, img, width, height) {
    const W = Math.ceil((width + 2 * MARGIN) * SCALE)
    const H = Math.ceil((height + 2 * MARGIN) * SCALE)
    const raw = document.createElement('canvas')
    raw.width = W
    raw.height = H
    const c = raw.getContext('2d')
    c.fillStyle = paper.color || '#000'
    c.fillRect(0, 0, W, H)
    c.setTransform(SCALE, 0, 0, SCALE, MARGIN * SCALE, MARGIN * SCALE)
    if (img) {
      const at = place(paper, { w: img.naturalWidth, h: img.naturalHeight })
      c.imageSmoothingQuality = 'high'
      if (at.tile) {
        const tiles = c.createPattern(img, 'repeat')
        tiles.setTransform(new DOMMatrix().translate(at.x, at.y).scale(at.w / img.naturalWidth, at.h / img.naturalHeight))
        c.fillStyle = tiles
        c.fillRect(-MARGIN, -MARGIN, width + 2 * MARGIN, height + 2 * MARGIN)
      } else {
        c.drawImage(img, at.x, at.y, at.w, at.h)
      }
    }
    c.setTransform(1, 0, 0, 1, 0, 0)
    const [mx, my, mw, mh] = paper.mon || [0, 0, width, height]
    const L = Math.min(W, Math.max(0, Math.ceil((mx + MARGIN) * SCALE)))
    const R = Math.max(L + 1, Math.min(W, Math.floor((mx + mw + MARGIN) * SCALE)))
    const T = Math.min(H, Math.max(0, Math.ceil((my + MARGIN) * SCALE)))
    const B = Math.max(T + 1, Math.min(H, Math.floor((my + mh + MARGIN) * SCALE)))
    if (L > 0) c.drawImage(raw, L, 0, 1, H, 0, 0, L, H)
    if (R < W) c.drawImage(raw, R - 1, 0, 1, H, R, 0, W - R, H)
    if (T > 0) c.drawImage(raw, 0, T, W, 1, 0, 0, W, T)
    if (B < H) c.drawImage(raw, 0, B - 1, W, 1, 0, B, W, H - B)

    const out = document.createElement('canvas')
    out.width = W
    out.height = H
    const o = out.getContext('2d')
    o.filter = `blur(${BLUR * SCALE}px)`
    o.drawImage(raw, 0, 0)
    o.filter = 'none'
    o.fillStyle = TINT
    o.globalCompositeOperation = 'luminosity'
    o.globalAlpha = LUMINOSITY
    o.fillRect(0, 0, W, H)
    o.globalCompositeOperation = 'source-over'
    o.globalAlpha = TINT_OPACITY
    o.fillRect(0, 0, W, H)
    return out
  }

  // The Mica in box (the strip's own size): set to a picture (taskbar.rs's
  // paper) and the strip's size, drawn anew only when either changes, the
  // new faded in over the old; cleared.
  function mount(box) {
    let key = ''
    let turn = 0
    return {
      async set(paper, width, height) {
        const now = JSON.stringify([paper, width, height])
        if (!paper || now === key) return
        key = now
        const mine = ++turn
        let img = null
        if (paper.url) {
          img = new Image()
          img.src = paper.url
          try {
            await img.decode()
          } catch {
            img = null
          }
        }
        if (mine !== turn) return
        const canvas = draw(paper, img, width, height)
        if (img) img.src = ''
        Object.assign(canvas.style, {
          left: `${-MARGIN}px`,
          top: `${-MARGIN}px`,
          width: `${width + 2 * MARGIN}px`,
          height: `${height + 2 * MARGIN}px`,
          opacity: '0',
        })
        const old = [...box.children]
        box.append(canvas)
        canvas.getBoundingClientRect()
        canvas.style.opacity = '1'
        setTimeout(() => old.forEach(o => o.remove()), FADE_MS)
      },
      clear() {
        key = ''
        turn++
        box.replaceChildren()
      },
    }
  }

  const api = { place, draw, mount }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
  } else {
    root.Mica = api
  }
})(this)

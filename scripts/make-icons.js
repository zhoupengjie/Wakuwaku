#!/usr/bin/env node
// Draws the app and tray icons with ImageMagick: a round face, two black
// capsule eyes, no mouth, a pastel colour per mood. Our own drawing; the
// outputs (src-tauri/icons) are committed, so building needs no ImageMagick.
//
//   node scripts/make-icons.js      (needs `magick` on PATH)
const { execFileSync } = require('child_process')
const fs = require('fs')
const path = require('path')

const ROOT = path.join(__dirname, '..')
const EYE = '#0b1a33'

// Each mood: face colour, eye shape, and a badge.
const MOODS = {
  idle: { face: '#cfe0f7', rim: '#8fb0dc', eyes: 'open' },
  working: { face: '#9fc2f5', rim: '#5f8fd8', eyes: 'focused' },
  waiting: { face: '#f7d79a', rim: '#d9a648', eyes: 'open', badge: { color: '#e08a1e', mark: '!' } },
  done: { face: '#bfe7c9', rim: '#74c08a', eyes: 'happy' },
  review: { face: '#d9cdf7', rim: '#a08fe0', eyes: 'happy', badge: { color: '#7c3aed', mark: '✓' } },
  error: { face: '#f6c4cf', rim: '#de8a9c', eyes: 'sad' },
}

// Draw commands for a face on an s x s canvas.
function face(s, { face: color, rim, eyes, badge }) {
  const c = s / 2
  const r = s * 0.46
  const eyeW = s * 0.085
  const eyeH = s * 0.2
  const dx = s * 0.15
  const ey = c + s * 0.02
  const draw = [`fill ${color} stroke ${rim} stroke-width ${Math.max(1, s * 0.035)} circle ${c},${c} ${c},${c - r}`]

  const capsule = (x, y, w, h, angle = 0) =>
    `fill ${EYE} translate ${x},${y} rotate ${angle} roundrectangle ${-w / 2},${-h / 2} ${w / 2},${h / 2} ${w / 2},${w / 2}`

  const eyeAt = (x, angle) => {
    if (eyes === 'focused') return capsule(x, ey + s * 0.02, eyeW, eyeH * 0.62, angle)
    if (eyes === 'happy') return capsule(x, ey + s * 0.03, eyeH * 0.62, eyeW * 0.75, 0)
    if (eyes === 'sad') return capsule(x, ey, eyeW, eyeH * 0.85, angle)
    return capsule(x, ey, eyeW, eyeH, 0)
  }
  draw.push(eyeAt(c - dx, eyes === 'sad' ? 28 : 0))
  draw.push(eyeAt(c + dx, eyes === 'sad' ? -28 : 0))
  // A touch of blush.
  draw.push(`fill #ffffff55 ellipse ${c - s * 0.27},${c + s * 0.16} ${s * 0.06},${s * 0.03} 0,360`)
  draw.push(`fill #ffffff55 ellipse ${c + s * 0.27},${c + s * 0.16} ${s * 0.06},${s * 0.03} 0,360`)

  if (badge) {
    const br = s * 0.15
    const bx = s - br - s * 0.02
    const by = br + s * 0.02
    draw.push(`fill ${badge.color} circle ${bx},${by} ${bx},${by - br}`)
    if (badge.mark === '!') {
      draw.push(`fill #ffffff roundrectangle ${bx - s * 0.022},${by - br * 0.6} ${bx + s * 0.022},${by + br * 0.18} ${s * 0.02},${s * 0.02}`)
      draw.push(`fill #ffffff circle ${bx},${by + br * 0.5} ${bx},${by + br * 0.5 - s * 0.024}`)
    } else {
      draw.push(`stroke #ffffff stroke-width ${s * 0.035} fill none polyline ${bx - br * 0.45},${by} ${bx - br * 0.1},${by + br * 0.38} ${bx + br * 0.5},${by - br * 0.35}`)
    }
  }
  return draw
}

// Each command in its own push/pop, so a translate or stroke never leaks.
// 8-bit RGBA: Tauri's PNG decoder takes no 16-bit images. `extra` goes before
// the output (the .ico's sizes).
function render(size, mood, out, extra = []) {
  const args = ['-size', `${size}x${size}`, 'xc:none']
  for (const cmd of face(size, MOODS[mood])) args.push('-draw', `push graphic-context ${cmd} pop graphic-context`)
  fs.mkdirSync(path.dirname(out), { recursive: true })
  execFileSync('magick', [...args, '-depth', '8', ...extra, out.endsWith('.png') ? `PNG32:${out}` : out])
}

const ICONS = path.join(ROOT, 'src-tauri', 'icons')
// The tray's faces, one per mood (32 px: sharp at 200%, scaled down at 100%).
for (const mood of Object.keys(MOODS)) render(32, mood, path.join(ICONS, 'tray', `${mood}.png`))
// The app: the window icon, the notifications' icon, and the .exe's.
render(256, 'idle', path.join(ICONS, 'icon.png'))
render(512, 'idle', path.join(ICONS, 'icon.ico'), ['-define', 'icon:auto-resize=256,64,48,32,16'])
console.log('icons written')

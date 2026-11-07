// One shot as the part of the screen it shows (its view): the wallpaper
// where it lies on the 1600 x 900 screen, under it Windows' acrylic where
// the island asks for it (island_plate), and over it her page's window
// (page.html in a frame) where her home has it; a screen shot has two, the
// desktop's page under her home's. Shot by render.ps1, framed by gallery.html.
;(function () {
  const Q = new URLSearchParams(location.search)
  const mode = Q.get('mode') === 'light' ? 'light' : 'dark'
  const S = window.Scenes
  const shot = S.byId(Q.get('scene') || '')
  const screenEl = document.getElementById('screen')
  const pending = document.getElementById('pending')
  if (!shot) {
    screenEl.textContent = `No scene "${Q.get('scene')}".`
    screenEl.style.color = '#fff'
    return
  }
  const [SW, SH] = S.SCREEN
  const [vx, vy, vw, vh] = shot.view
  Object.assign(screenEl.style, { width: `${vw}px`, height: `${vh}px` })
  document.body.style.width = `${vw}px`

  const wall = document.getElementById('wall')
  Object.assign(wall.style, { left: `${-vx}px`, top: `${-vy}px`, width: `${SW}px`, height: `${SH}px`, backgroundImage: `url("${Q.get('wall') || 'assets/wall.jpg'}")` })

  // The pages and their windows: the shot's own, or the desktop's and her home's.
  // Split, the dock's too, at the foot (bars.rs).
  const pages = shot.page === 'screen' ? [['desk', S.WINDOWS.desk], ['island', S.homeWindowOf(shot)], ...(shot.split ? [['bar', S.WINDOWS.dock]] : [])] : [[shot.page, S.windowOf(shot)]]
  const frames = pages.map(([role, [wx, wy, ww, wh]]) => {
    const frame = document.createElement('iframe')
    Object.assign(frame.style, { left: `${wx - vx}px`, top: `${wy - vy}px`, width: `${ww}px`, height: `${wh}px` })
    // Her capsule's own page in Windows' taskbar (island.js part=capsule).
    frame.src = `page.html?role=${role}&scene=${encodeURIComponent(shot.id)}&mode=${mode}${shot.part ? `&part=${shot.part}` : ''}`
    screenEl.append(frame)
    return { frame, at: [wx, wy], frozen: false, errors: [] }
  })

  // Windows' acrylic under the island, where the page puts it (backplate.rs).
  let plate = null
  window.addEventListener('message', e => {
    const m = e.data
    const f = frames.find(x => x.frame.contentWindow === e.source)
    if (!m || !m.wk || !f) return
    if ('plate' in m) {
      plate?.remove()
      plate = null
      if (m.plate) {
        plate = document.createElement('div')
        plate.className = `plate${m.plate.dark ? '' : ' light'}`
        Object.assign(plate.style, { left: `${f.at[0] - vx + m.plate.x}px`, top: `${f.at[1] - vy + m.plate.y}px`, width: `${m.plate.width}px`, height: `${m.plate.height}px`, borderRadius: '8px' })
        f.frame.before(plate)
      }
    }
    if (m.frozen) {
      f.frozen = true
      f.errors = m.errors || []
      if (frames.some(x => !x.frozen)) return
      pending.remove()
      const errors = frames.flatMap(x => x.errors)
      if (errors.length) {
        const notes = document.createElement('pre')
        notes.id = 'notes'
        notes.textContent = errors.join('\n')
        screenEl.append(notes)
      }
      document.documentElement.dataset.frozen = '1'
    }
  })
})()

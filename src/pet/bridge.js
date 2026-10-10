// window.pet: what the page asks of the Rust side and hears from it, over
// Tauri's IPC, in her window (role=pet) and the island's (role=island).
// Loaded before the page's own scripts. The Rust side knows which window a
// call comes from.
//
// A window that lets clicks through hears no mouse moves. So while the
// pointer is near and the window lets clicks through, the Rust side sends
// where it is (pet:pointer), and this plays the mouseenter / mouseleave the
// page would have heard.
;(function () {
  const { invoke } = window.__TAURI__.core
  const { listen } = window.__TAURI__.event

  // Listeners register over IPC; the Rust side starts talking only once they
  // all have (pet_ready).
  const pending = []
  const on = channel => fn => {
    pending.push(listen(channel, e => fn(e.payload)))
  }
  const noop = () => {}

  // One at a time, in order: each call is its own request, and a drop must
  // not overtake the release before it.
  let queue = Promise.resolve()
  const send = (cmd, args) => {
    queue = queue.then(() => invoke(cmd, args)).catch(err => console.error(cmd, err))
  }

  window.pet = {
    onUpdate: on('pet:update'),
    onReact: on('pet:react'),
    // A widget asks to open the island for itself (widgets.rs).
    onNudge: on('pet:nudge'),
    onAlert: on('pet:alert'),
    onCursor: on('pet:cursor'),
    onDrag: on('pet:drag'),
    onDragEnd: on('pet:drag-end'),
    onAsks: on('pet:asks'),
    onShift: on('pet:shift'),
    answer: (id, choice) => send('pet_answer', { id, choice }),
    dismiss: id => send('pet_dismiss', { id }),
    keyboard: needs => send('pet_keyboard', { needs: needs === true }),
    panel: measured => send('pet_panel', { measured: measured || null }),
    hover: isOver => {
      // The real pointer has the page now, or has left it: start over.
      inside = []
      send('pet_hover', { isOver: isOver === true })
    },
    dragStart: () => send('pet_drag_start'),
    dragEnd: () => send('pet_drag_end'),
    menu: () => send('pet_menu'),
    doubleClick: () => send('pet_double_click'),
    // Out of the queue: it answers when the walk is over.
    walk: (dx, ms) => invoke('pet_walk', { dx, ms }).catch(() => false),
    walkStop: () => send('pet_walk_stop'),
    // The island's page: she broke free of the drop (her middle this far from
    // the cursor), the button was let go, the button is held on her.
    releaseHer: offset => send('island_release', { offset: offset || {} }),
    dropHer: () => send('island_drop'),
    holding: isHolding => send('island_holding', { isHolding: isHolding === true }),
    onReach: on('island:reach'),
    onAbsorb: on('island:absorb'),
    onLanded: on('island:landed'),
    // The settings in the island (settings.js): open them here or from
    // anywhere else (the tray, her double-click), what they show, what they change.
    onSettings: on('island:settings'),
    onSettingsChanged: on('settings:changed'),
    // The island's window lost the keyboard (a click elsewhere).
    onBlur: on('island:blur'),
    openSettings: tab => send('settings_open', { tab: tab || null }),
    settingsClosed: () => send('settings_closed'),
    // To a session's window; true when there was one to go to.
    jump: id => invoke('session_jump', { id: String(id || '') }).catch(() => false),
    // The taskbar (taskbar.rs): what its own buttons open (start, search,
    // tasks, widgets, desktop, quick, notifications), a press on a tray icon
    // (leftDown, leftUp, rightDown, rightUp, double, move, in, out; in order,
    // so a press never overtakes the one before), where the page drew the
    // tray's icons; the icons as they change.
    taskbar: {
      open: what => invoke('taskbar_open', { what: String(what) }).catch(() => false),
      tray: (key, press) => send('taskbar_tray', { key: Number(key), press: String(press) }),
      trayRects: rects => send('taskbar_tray_rects', { rects }),
      // A press on a window's button: press (to the front, or minimized), close.
      window: (id, what) => invoke('taskbar_window', { id: Number(id), what: String(what) }).catch(() => false),
      // A tray icon kept out on the taskbar, or folded away (its name, systray.rs).
      trayPin: (name, pinned) => send('taskbar_tray_pin', { name: String(name), pinned: pinned === true }),
      // A program's button: launch (it, or another window of it), pin, unpin.
      app: (path, what) => invoke('taskbar_app', { path: String(path), what: String(what) }).catch(() => false),
      // Room for windows' live pictures in the page: [[window, x, y, w, h]], or none.
      thumbs: items => send('taskbar_thumbs', { items }),
    },
    onTray: on('taskbar:tray'),
    // The windows' buttons as they change ({ id, title, exe, png, front, min, flash, sessions }).
    onWindows: on('taskbar:windows'),
    // The keyboard as it changes ({ lang, native, caps }).
    onKeys: on('taskbar:keys'),
    // The desktop's picture under the strip, for its Mica ({ url, position,
    // color, mon, screen, sf }; mica.js).
    onWallpaper: on('taskbar:wallpaper'),
    // Windows' look ({ mode: dark | light, accent: { base, light, dark } }),
    // for the taskbar to be as Windows' own is.
    onLook: on('taskbar:look'),
    settings: {
      get: () => invoke('settings_get'),
      set: patch => invoke('settings_set', { patch }),
      showPet: on => invoke('settings_show_pet', { on: on === true }),
      login: on => invoke('settings_login', { on: on === true }),
      hooks: action => invoke('settings_hooks', { action }),
      codexHooks: action => invoke('settings_codex_hooks', { action }),
      fetch: reference => invoke('settings_fetch', { reference: String(reference || '') }),
      gallery: (page, sort) => invoke('settings_gallery', { page, sort }),
      openSite: place => invoke('settings_open_site', { place }),
    },
    // Mail accounts (mail.rs): the server for an address, an account kept
    // once it signs in, one removed, one on or off, a server tried without
    // signing in (Re-test); an inbox's newest letters (id "*" every
    // account's; filter all | unseen | flagged), a star put on or off, one
    // letter opened (and marked read), one handed to an agent (agent claude
    // | codex, how talk | open), more asked in its talk; a letter sent
    // (send.rs); Re-test of the outgoing server (kind smtp).
    mail: {
      discover: address => invoke('mail_discover', { address: String(address || '') }),
      probe: (host, port, security, kind) => invoke('mail_probe', { host: String(host || ''), port: String(port ?? ''), security: String(security || 'auto'), kind: kind === 'smtp' ? 'smtp' : 'imap' }),
      send: letter => invoke('mail_send', { letter }),
      save: (account, password) => invoke('mail_save', { account, password: String(password || '') }),
      remove: id => invoke('mail_remove', { id: String(id) }),
      switch: (id, on) => invoke('mail_switch', { id: String(id), on: on === true }),
      letters: (id, count, filter) => invoke('mail_letters', { id: String(id), count: Number(count) || 50, filter: String(filter || 'all') }),
      flag: (id, uid, on) => invoke('mail_flag', { id: String(id), uid: Number(uid), on: on === true }),
      say: (key, text) => invoke('mail_say', { key: String(key), text: String(text || '') }),
      letter: (id, uid) => invoke('mail_letter', { id: String(id), uid: Number(uid) }),
      hand: (id, uid, agent, how) => invoke('mail_hand', { id: String(id), uid: Number(uid), agent: String(agent), how: String(how) }),
    },
  }

  // --- Mouse moves while clicks pass through ------------------------------------

  // The elements under the pointer, outermost first, as last played.
  let inside = []

  function chainAt(x, y) {
    const chain = []
    for (let el = document.elementFromPoint(x, y); el && el !== document.documentElement; el = el.parentElement) chain.unshift(el)
    return chain
  }

  function play(type, el, x, y) {
    el.dispatchEvent(new MouseEvent(type, { clientX: x, clientY: y, bubbles: false, view: window }))
  }

  pending.push(
    listen('pet:pointer', ({ payload: at }) => {
      const next = at ? chainAt(at.x, at.y) : []
      const was = inside
      inside = next
      for (const el of was) if (!next.includes(el)) play('mouseleave', el, at?.x ?? -1, at?.y ?? -1)
      for (const el of next) if (!was.includes(el)) play('mouseenter', el, at.x, at.y)
    }),
  )

  // The page's console.log into debug.log (when it is on).
  const log = console.log.bind(console)
  console.log = (...args) => {
    log(...args)
    invoke('pet_log', { line: args.map(String).join(' ') }).catch(noop)
  }

  // Debug only (/debug/eval): run code, give back what it returns.
  window.__wkEval = (id, fn) => {
    Promise.resolve()
      .then(fn)
      .catch(err => 'error: ' + err.message)
      .then(value => invoke('debug_result', { id, value: value === undefined ? null : value }))
  }

  // Errors too, so a page that never comes up says why.
  window.addEventListener('error', e => console.log('error:', e.message, e.filename, e.lineno))
  window.addEventListener('unhandledrejection', e => console.log('unhandled:', e.reason))

  document.addEventListener('DOMContentLoaded', () => {
    Promise.all(pending)
      .then(() => send('pet_ready'))
      .catch(err => console.log('listeners failed:', err))
  })
})()

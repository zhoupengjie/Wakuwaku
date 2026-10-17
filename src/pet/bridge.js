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
  // all have (pet_ready). Each listens for this window alone: listen() by
  // itself hears what is sent to any window (Tauri 2), so her page heard the
  // island's pet:pointer and played it as the pointer over her, at the
  // island's coordinates, and the island heard hers; the windows' mouse
  // modes bounced caught/through 30 times a second (2026-10-11), and every
  // event was handled twice.
  const LABEL = window.__TAURI_INTERNALS__?.metadata?.currentWebview?.label || window.__TAURI__.webviewWindow?.getCurrentWebviewWindow?.()?.label || new URLSearchParams(location.search).get('role') || 'pet'
  const target = { kind: 'WebviewWindow', label: LABEL }
  const pending = []
  const on = channel => fn => {
    pending.push(listen(channel, e => fn(e.payload), { target }))
  }
  const noop = () => {}

  // One at a time, in order: each call is its own request, and a drop must
  // not overtake the release before it.
  let queue = Promise.resolve()
  const send = (cmd, args) => {
    queue = queue.then(() => invoke(cmd, args)).catch(err => console.error(cmd, err))
  }

  window.pet = {
    // On a Mac (WebKit says MacIntel, on Apple silicon too): what is done
    // for its notch and its menus only there, so Windows and macOS never
    // change each other.
    mac: /^Mac/.test(navigator.platform || ''),
    onUpdate: on('pet:update'),
    // Only the widgets' words changed (the monitor's readings): { widgets }.
    onWidgets: on('pet:widgets'),
    // Nobody at the keyboard or mouse for a while, or someone again: { on }.
    onQuiet: on('pet:quiet'),
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
    // Her menu; on the volume, the network or the clock of her taskbar
    // (part), the one Windows' own taskbar has there.
    menu: part => send('pet_menu', { part: part || null }),
    doubleClick: () => send('pet_double_click'),
    // Out of the queue: it answers when the walk is over.
    walk: (dx, ms) => invoke('pet_walk', { dx, ms }).catch(() => false),
    walkStop: () => send('pet_walk_stop'),
    // The island's page: she broke free of the drop (her middle this far from
    // the cursor), the button was let go, the button is held on her.
    releaseHer: offset => send('island_release', { offset: offset || {} }),
    dropHer: () => send('island_drop'),
    holding: isHolding => send('island_holding', { isHolding: isHolding === true }),
    // The taskbar's layout being edited, or not, and where its bar, library
    // and tools are ([[x, y, w, h]], page px): the window keeps the mouse
    // there whatever enter and leave say (island.rs set_editing, #53).
    editing: (on, rects) => send('island_editing', { on: on === true, rects: Array.isArray(rects) ? rects : [] }),
    // Her seat on the taskbar, where the page has laid her island out (logical px from its left).
    seat: x => send('island_seat', { x: Number(x) || 0 }),
    // A blur of what is behind her island, under it (backplate.rs): where
    // the island is ({ x, y, width, height, radius, vw, vh, dark }), faded
    // in over fade ms; none, faded out.
    plate: (rect, fade) => send('island_plate', { rect: rect || null, fade: Math.max(0, Math.round(Number(fade) || 0)) }),
    onReach: on('island:reach'),
    onAbsorb: on('island:absorb'),
    onLanded: on('island:landed'),
    // The prompt waiting asked for from her taskbar on another display (bars.rs).
    onLookHere: on('island:look-here'),
    // Up over an app full screen for the prompts alone ({ on }, island.rs over_full): her island only.
    onOverFull: on('island:over-full'),
    // A maximized window on her display, at the top of the screen ({ on }, island.rs look_over_max).
    onOverMax: on('island:over-max'),
    // An app full screen on her display, or none now ({ on }, main.rs
    // look_fullscreen): no flow light's ring while one is (island.js).
    onFullScreen: on('pet:full-screen'),
    // At the top of the screen, a window under her closed island ({ on },
    // island.rs look_under); the pointer has called it down from out of
    // the way, or has gone from it ({ on }, call_look); it is up out of the
    // way or not, and its closed size (island.js setStow).
    onUnder: on('island:under'),
    onCalled: on('island:called'),
    stow: (stowed, width, height) => send('island_stow', { stowed: stowed === true, width: Number(width) || 0, height: Number(height) || 0 }),
    // Something opened up from her taskbar's strip: its window over any
    // other that has come over it (island.rs raise, zhoupengjie/Wakuwaku#55).
    raise: () => send('island_raise'),
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
      // The volume that many steps up (down below 0), by Windows' volume keys.
      volume: steps => invoke('taskbar_volume', { steps: Math.round(Number(steps) || 0) }).catch(() => false),
      // The clock's tip as Windows' own has it: the date in full, and
      // Windows' additional clocks (clock.rs).
      clockTip: () => invoke('taskbar_clock_tip').catch(() => ''),
      // Whether Windows' End task is on now (Settings → For developers).
      endTask: () => invoke('taskbar_end_task').catch(() => false),
      // The input methods to pick from ([{ id, lang, langName, name, icon, own,
      // current }], inputs.rs), and one of them switched to for the window in front.
      inputs: () => invoke('taskbar_inputs').catch(() => []),
      input: id => invoke('taskbar_input', { id: String(id) }).catch(() => false),
      // Windows' own battery font (an ArrayBuffer; empty where Windows has none).
      batteryFont: () => invoke('taskbar_battery_font').catch(() => null),
      tray: (key, press) => send('taskbar_tray', { key: Number(key), press: String(press) }),
      trayRects: rects => send('taskbar_tray_rects', { rects }),
      // A press on a window's button: press (to the front, or minimized),
      // front (to the front alone: the next of a program's windows), drag (to
      // the front, a file held over its button),
      // close, end (its process ended, as Windows' End task does).
      window: (id, what) => invoke('taskbar_window', { id: Number(id), what: String(what) }).catch(() => false),
      // A program's windows all to the front, or all minimized when one is in front.
      windows: ids => invoke('taskbar_windows', { ids: (ids || []).map(Number) }).catch(() => false),
      // Shift and Ctrl held now, as Windows has them ({ shift, ctrl }; null
      // if it could not say): read at a press on a program's button.
      held: () => invoke('taskbar_held').catch(() => null),
      // Win pressed while her window has the keyboard: given back while it
      // is held (winkeys.rs pause), the keys' hook hears nothing meanwhile.
      keysPause: () => send('taskbar_keys_pause'),
      // A window's own menu, or one for all of a program's windows, at the pointer.
      windowMenu: ids => invoke('taskbar_window_menu', { ids: (ids || []).map(Number) }).catch(() => false),
      // Presses anywhere told back (onPress) while something is open that closes on one.
      watch: on => send('taskbar_watch_presses', { on: on === true }),
      // A tray icon kept out on the taskbar, or folded away (its name,
      // systray.rs; null: where it was), and the tray's order (names) after it.
      trayPin: (name, pinned, order) =>
        send('taskbar_tray_pin', { name: String(name), pinned: typeof pinned === 'boolean' ? pinned : null, order: Array.isArray(order) ? order.map(String) : null }),
      // A program's button: launch (it, or another window of it), admin (run
      // as administrator), pin, unpin.
      app: (path, what, id) => invoke('taskbar_app', { path: String(path), what: String(what), id: id ? String(id) : null }).catch(() => false),
      // Room for windows' live pictures in the page: [[window, x, y, w, h]], or none.
      thumbs: items => send('taskbar_thumbs', { items }),
      // The handover with Windows' own taskbar (handoff.js): its picture is
      // over it (covered), hers is bare for Windows' to show under it (bare),
      // the dust has settled (done). Out of the queue: the program may be
      // waiting on it inside an earlier call (pet_ready waits for covered).
      handoff: what => invoke('taskbar_handoff', { what: String(what) }).catch(() => {}),
    },
    // The handover's steps ({ step: in, picture } with Windows' strip as a
    // picture, { step: go } once Windows' is hidden under it, { step: out }).
    onHandoff: on('taskbar:handoff'),
    onTray: on('taskbar:tray'),
    // The strip's thickness as it lies on the screen (physical px): { thick }.
    onStrip: on('taskbar:strip'),
    // A Mac's screen top, the island over its menu bar (notch.rs):
    // { height, notch } in px, the notch's width 0 without one.
    onTop: on('island:top'),
    // Her tray icon clicked on a Mac: the island in the notch drops open, or
    // goes back (island.js peek).
    onPeek: on('island:peek'),
    // The windows' buttons as they change ({ id, title, exe, png, front, min, flash, sessions }).
    onWindows: on('taskbar:windows'),
    // The keyboard as it changes ({ lang, native, caps, ime: { id, name, lang,
    // icon, own, mode: { tip, glyph, icon, off } } }), and the input methods'
    // icons the keys name ({ key: png }), each sent once.
    onKeys: on('taskbar:keys'),
    onImeIcons: on('taskbar:imeIcons'),
    onPress: on('taskbar:press'),
    // A file dragged from another program over the window: (type, at) with
    // type enter, over, leave or drop, at where it is in the page's px (none
    // once it has left). Tauri's drop handler has the window's drags, so the
    // page hears no dragenter of its own.
    onFileDrag: fn => fileDrags.push(fn),
    // Win+T, Win+B (winkeys.rs): the keyboard, given her window already, on
    // her programs' buttons or her tray ({ part: apps | tray, back }).
    onFocusBar: on('taskbar:focus'),
    // The desktop's picture under the strip, for its Mica ({ url, position,
    // color, mon, screen, sf }; mica.js).
    onWallpaper: on('taskbar:wallpaper'),
    // Windows' look ({ mode: dark | light, accent: { base, light, dark } }),
    // for the taskbar to be as Windows' own is.
    onLook: on('taskbar:look'),
    // The taskbar's layout to be edited by dragging (its menu; { picker }:
    // 添加小工具…, the picker open with it).
    onTaskbarEdit: on('taskbar:edit'),
    // Her menu in the island's home (picker.js): 添加小工具… ({ open:
    // 'picker' }) or 编辑胶囊 ({ open: 'capsule' }).
    onPicker: on('island:picker'),
    // A widget dragged from her home over the desktop (picker.js), for the
    // desktop's page of the display under it (desk-drop.js; the Rust side
    // forwards both, phase 2): where it would land shown ({ x, y, item,
    // size }, in this page's px; null: off the desktop again), and let go
    // there ({ x, y, item, size, ticket }). What that page said back: {
    // ticket, ok, where, why } (ticket null: about the preview).
    deskPreview: at => send('desk_drag_preview', { at: at || null }),
    deskDrop: at => send('desk_drop', { at }),
    onDeskAnswer: on('island:desk-answer'),
    // A desktop widget's page asks the island's (desk.rs desk_ask): a
    // widget's settings ({ what: 'settings', args: { widget } }), widgets
    // put elsewhere ({ what: 'place', args: { items, where, stack } }).
    onDeskAsk: on('desk:ask'),
    // The desktop's widgets (desk.rs): entries put there or changed, ids gone.
    desk: {
      keep: (entries, gone) => invoke('desk_keep', { entries: entries || [], gone: gone || [] }),
    },
    // Windows' Start menu opened or closed ({ open }), for the Start button.
    onStart: on('taskbar:start'),
    settings: {
      get: () => invoke('settings_get'),
      set: patch => invoke('settings_set', { patch }),
      showPet: on => invoke('settings_show_pet', { on: on === true }),
      login: on => invoke('settings_login', { on: on === true }),
      // Windows' own mode ({ light }) or accent colour ({ accent: "#rrggbb" }).
      winLook: ({ light, accent } = {}) => invoke('settings_win_look', { light: typeof light === 'boolean' ? light : null, accent: typeof accent === 'string' ? accent : null }),
      // The desktop's picture: picked (picture, folder), laid (position), its turns (every, shuffle), the next now.
      wallpaper: (what, value) => invoke('settings_wallpaper', { what: String(what), value: value == null ? null : String(value) }),
      hooks: action => invoke('settings_hooks', { action }),
      codexHooks: action => invoke('settings_codex_hooks', { action }),
      agyHooks: action => invoke('settings_agy_hooks', { action }),
      // Our plugin in OpenCode's plugin folder: install, remove.
      opencode: action => invoke('settings_opencode', { action }),
      // omp's extension, in each of its agent folders: 'install', 'remove'.
      ompHooks: action => invoke('settings_omp_hooks', { action }),
      zcodeHooks: action => invoke('settings_zcode_hooks', { action }),
      dshHooks: action => invoke('settings_dsh_hooks', { action }),
      // The CPU temperature/power helper (#18 part B): 'install',
      // 'installPawnIO' (download PawnIO first), 'remove'. Each shows one UAC.
      cpuSensors: action => invoke('settings_cpu_sensors', { action }),
      fetch: reference => invoke('settings_fetch', { reference: String(reference || '') }),
      gallery: (page, sort, query) => invoke('settings_gallery', { page, sort, query }),
      openSite: place => invoke('settings_open_site', { place }),
      // Her updates: 'check', 'page' (the release's), 'update' (updates.rs).
      update: what => invoke('settings_update', { what }),
      // LibreHardwareMonitor for the monitor: 'get', 'start', 'startup', 'noStartup',
      // 'usePort', 'folder', 'look', 'dismiss' (lhm_setup.rs).
      lhm: what => invoke('settings_lhm', { what: String(what) }),
      // Her crash reports: 'yes', 'no' (her asking answered), 'send', 'open' (the last one's issue; crash.rs).
      crash: what => invoke('settings_crash', { what }),
    },
    // Mail accounts (mail.rs): the server for an address, an account kept
    // once it signs in, one removed, one on or off, a server tried without
    // signing in (Re-test); an inbox's newest letters (id "*" every
    // account's; filter all | unseen | flagged), a star put on or off, one
    // letter opened (and marked read), one handed to an agent (agent claude
    // | codex | agy, how talk | open), more asked in its talk; a letter sent
    // (send.rs); Re-test of the outgoing server (kind smtp). The lookup and
    // Re-test go the way the account being set up does (proxy: its own, or
    // null for the Mail page's; mail/proxy.rs).
    mail: {
      discover: (address, proxy) => invoke('mail_discover', { address: String(address || ''), proxy: proxy || null }),
      probe: (host, port, security, kind, proxy) =>
        invoke('mail_probe', { host: String(host || ''), port: String(port ?? ''), security: String(security || 'auto'), kind: kind === 'smtp' ? 'smtp' : 'imap', proxy: proxy || null }),
      send: letter => invoke('mail_send', { letter }),
      save: (account, password) => invoke('mail_save', { account, password: String(password || '') }),
      remove: id => invoke('mail_remove', { id: String(id) }),
      switch: (id, on) => invoke('mail_switch', { id: String(id), on: on === true }),
      letters: (id, count, filter) => invoke('mail_letters', { id: String(id), count: Number(count) || 50, filter: String(filter || 'all') }),
      flag: (id, uid, on, folder) => invoke('mail_flag', { id: String(id), uid: Number(uid), on: on === true, folder: String(folder || 'inbox') }),
      say: (key, text) => invoke('mail_say', { key: String(key), text: String(text || '') }),
      letter: (id, uid, folder) => invoke('mail_letter', { id: String(id), uid: Number(uid), folder: String(folder || 'inbox') }),
      // An account's sync (or every one's, '*') to look at the server now.
      kick: id => invoke('mail_kick', { id: String(id) }),
      // The letter being written, as kept in the data folder (null: none).
      draft: () => invoke('mail_draft'),
      keepDraft: draft => invoke('mail_keep_draft', { draft: draft || null }),
      hand: (id, uid, agent, how, folder) => invoke('mail_hand', { id: String(id), uid: Number(uid), agent: String(agent), how: String(how), folder: String(folder || 'inbox') }),
    },
  }

  // --- Mouse moves while clicks pass through ------------------------------------

  // The elements under the pointer, outermost first, as last played.
  let inside = []
  // What the page has had a mouseenter for and no mouseleave since, real or
  // played, in the order entered (heard in the capture phase: neither
  // bubbles). Left by the next word from the Rust side that the pointer is
  // off it (pointer.rs; hovers.js): entered by a played mouseenter, the leave
  // may never come from WebView2 itself.
  const entered = new Set()
  document.addEventListener('mouseenter', e => e.target instanceof Element && e.target !== document.documentElement && entered.add(e.target), true)
  document.addEventListener('mouseleave', e => entered.delete(e.target), true)

  function chainAt(x, y) {
    const chain = []
    for (let el = document.elementFromPoint(x, y); el && el !== document.documentElement; el = el.parentElement) chain.unshift(el)
    return chain
  }

  function play(type, el, x, y) {
    el.dispatchEvent(new MouseEvent(type, { clientX: x, clientY: y, bubbles: false, view: window }))
  }

  pending.push(
    listen(
      'pet:pointer',
      ({ payload: at }) => {
        const next = at ? chainAt(at.x, at.y) : []
        // Whatever the page has entered, either way, and the pointer is not
        // on now (gone: all of it) is left; caught, only once the pointer is
        // clear of its box (hovers.js).
        for (const el of [...entered]) if (!el.isConnected) entered.delete(el)
        for (const el of window.Hovers.stale([...entered], next, at, el => el.getBoundingClientRect())) {
          entered.delete(el)
          play('mouseleave', el, at?.x ?? -1, at?.y ?? -1)
        }
        // Caught, the window has the mouse: WebView2's own events enter.
        if (at?.caught) return
        const was = inside
        inside = next
        for (const el of next) if (!was.includes(el)) play('mouseenter', el, at.x, at.y)
      },
      { target },
    ),
  )

  // --- A file dragged over the window ---------------------------------------------

  // While a drag of files from another program is over the window (Tauri's
  // drop handler has it: tauri://drag-*, where it is in physical px), the
  // drag holds the mouse and the page hears none. Where it is is played as
  // the pointer's, in a chain of its own (hover() clears the pointer's), so
  // that the window lets the drag through once it is off her things, as it
  // does the pointer: caught, the clear window over the strip took the drop
  // meant for the window under it. Then told the page (onFileDrag).
  const fileDrags = []
  let dragged = []
  for (const type of ['enter', 'over', 'leave', 'drop']) {
    pending.push(
      listen(
        `tauri://drag-${type}`,
        ({ payload }) => {
          const p = type === 'leave' ? null : payload?.position
          const at = p ? { x: p.x / devicePixelRatio, y: p.y / devicePixelRatio } : null
          if (at && type !== 'drop') {
            const next = chainAt(at.x, at.y)
            const was = dragged
            dragged = next
            for (const el of was) if (!next.includes(el)) play('mouseleave', el, at.x, at.y)
            for (const el of next) if (!was.includes(el)) play('mouseenter', el, at.x, at.y)
          } else {
            // Gone, or let go of: the mouse is the page's again.
            dragged = []
          }
          for (const fn of fileDrags) fn(type, at)
        },
        { target },
      ),
    )
  }

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

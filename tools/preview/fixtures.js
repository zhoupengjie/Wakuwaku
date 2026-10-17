// Sample data for the scenes, as the Rust side would send it: sessions in
// each mood (state.rs view), a prompt (asks.rs view), widgets with their
// details and sizes (widgets.rs), the plugins (scripts.rs view), the
// taskbar's programs, tray and keyboard (taskbar.rs), and desktop layouts
// (desk.rs). Loaded after fake-tauri.js (its clock, Fake.T0). Node reads it
// too (the tests of scenes.js): there the icons are a placeholder.
;(function (root) {
  const T0 = root.Fake?.T0 ?? Date.now()
  const canDraw = typeof document !== 'undefined'
  const NO_ICON = 'data:image/png;base64,'

  // A program's icon as Windows gives one (a PNG): a rounded square in its
  // colour with its letter, drawn here.
  function appIcon(color, letter, size = 48) {
    if (!canDraw) return NO_ICON
    const c = document.createElement('canvas')
    c.width = c.height = size
    const g = c.getContext('2d')
    const grad = g.createLinearGradient(0, 0, size, size)
    grad.addColorStop(0, color)
    grad.addColorStop(1, shade(color, -0.25))
    g.fillStyle = grad
    g.beginPath()
    g.roundRect(size * 0.08, size * 0.08, size * 0.84, size * 0.84, size * 0.2)
    g.fill()
    g.fillStyle = '#fff'
    g.font = `600 ${Math.round(size * 0.46)}px "Segoe UI Variable Display", "Segoe UI", sans-serif`
    g.textAlign = 'center'
    g.textBaseline = 'middle'
    g.fillText(letter, size / 2, size / 2 + size * 0.03)
    return c.toDataURL()
  }
  // A tray icon: a small round mark in its colour.
  function trayIcon(color, letter, size = 32) {
    if (!canDraw) return NO_ICON
    const c = document.createElement('canvas')
    c.width = c.height = size
    const g = c.getContext('2d')
    g.fillStyle = color
    g.beginPath()
    g.arc(size / 2, size / 2, size * 0.44, 0, Math.PI * 2)
    g.fill()
    g.fillStyle = '#fff'
    g.font = `700 ${Math.round(size * 0.5)}px "Segoe UI", sans-serif`
    g.textAlign = 'center'
    g.textBaseline = 'middle'
    g.fillText(letter, size / 2, size / 2 + 1)
    return c.toDataURL()
  }
  function shade(hex, by) {
    const n = parseInt(hex.slice(1), 16)
    const f = v => Math.max(0, Math.min(255, Math.round(v + v * by)))
    return `rgb(${f(n >> 16)}, ${f((n >> 8) & 255)}, ${f(n & 255)})`
  }

  // --- Sessions ---------------------------------------------------------------------------
  const BASE = { id: 's1', project: 'pet', agent: 'claude', jump: true, name: '任务栏设计', detail: null, at: T0, since: null, took: null, step: null, stepSince: null, todo: null, files: 0, reply: '', error: null }
  const MOODS = {
    idle: {},
    working: { since: T0 - 95e3, step: { key: 'step.edit', vars: { file: 'island.js' } }, stepSince: T0 - 4e3, todo: { done: 3, total: 7, active: '胶囊的流光' } },
    waiting: { since: T0 - 131e3, detail: { key: 'detail.needsApproval', vars: { tool: 'Bash' } } },
    done: { took: 151e3, reply: '改好了：浅色下胶囊的描边，和小点的位置。' },
    review: { took: 151e3, files: 3, reply: '改了三个文件，来看看。' },
    error: { took: 42e3, error: { key: 'error.rate_limit', vars: {} } },
  }
  const session = (mood, extra = {}) => ({ ...BASE, mood, ...MOODS[mood], ...extra })
  // Others beside the one shown: Codex at work, a turn done.
  const others = () => [
    { ...BASE, id: 's2', agent: 'codex', name: '邮件同步', project: 'mail', mood: 'working', since: T0 - 412e3, step: { key: 'step.command', vars: { what: 'cargo test' } }, stepSince: T0 - 30e3 },
    { ...BASE, id: 's3', name: '发布说明', project: 'pet', mood: 'done', took: 73e3, reply: '写好了。' },
  ]
  // A third at work: reading, in another project.
  const reading = () => ({ ...BASE, id: 's3', name: '周报', project: 'docs', mood: 'working', since: T0 - 48e3, step: { key: 'step.read', vars: { file: 'style.md' } }, stepSince: T0 - 2e3 })

  // A permission prompt (asks.rs view).
  const ASK = { id: 1, kind: 'permission', tool: 'Bash', summary: 'npm test -- --watch=false', project: 'pet', session: 's1', always: { rules: ['Bash(npm test:*)'], dirs: [], modes: [], where: ['localSettings'] } }

  // --- Widgets ----------------------------------------------------------------------------
  const hourNow = new Date(T0).getHours()
  const hours = Array.from({ length: 24 }, (_, h) => (h >= 9 && h <= hourNow ? [2, 5, 9, 14, 11, 18, 22, 25, 19, 23, 27, 22, 15, 9, 6][h - 9] || 4 : 0))
  const cpu = Array.from({ length: 30 }, (_, i) => 15 + Math.round(8 * Math.sin(i / 3) + (i % 5)))
  const net = [7400, 9000, 6000, 12000, 30000, 8000, 7400, 6600, 9100, 7400]
  const WIDGETS = [
    {
      id: 'monitor', builtIn: true, from: 'builtIn', on: true, icon: 'gauge', color: '#5e9bff', label: { key: 'widget.monitor' },
      value: { parts: [{ icon: 'cpu', text: '17%' }, { icon: 'memory', text: '44%' }, { icon: 'down', text: '7.4K' }, { icon: 'up', text: '337K' }] },
      detail: { sizes: ['s', 'm', 'l'], series: { points: cpu, unit: '%', hi: 100, lo: 0 }, rows: [{ kind: 'cpu', value: '17%', text: '3.4 GHz', name: '8', unit: '16' }, { kind: 'memory', value: '44%', text: '14.1 / 32 GB' }, { kind: 'gpu', value: '23%', name: 'RTX 4070', text: '52°' }, { kind: 'disk', value: '61%', name: 'Samsung SSD 990 PRO' }, { kind: 'net', value: '7.4K', text: '337K', points: net }, { kind: 'uptime', value: '273600' }] },
    },
    { id: 'today', builtIn: true, from: 'builtIn', on: true, icon: 'today', color: '#34d27b', label: { key: 'widget.today' }, value: { key: 'widget.todayValue', vars: { turns: '197', time: '2:01', approvals: '3' } }, detail: { sizes: ['s', 'm', 'l'], series: { points: hours, unit: 'turns' } } },
    { id: 'tokens', builtIn: true, from: 'builtIn', on: true, icon: 'code', color: '#ff9f0a', label: { key: 'widget.tokens' }, value: { key: 'widget.tokensValue', vars: { total: '1.7B', output: '3.4M' } } },
    {
      id: 'weather', plugin: 'weather', from: 'script', on: true, icon: 'weather', color: '#ffb340', label: 'Dresden · 阴', value: '11°',
      detail: {
        sizes: ['s', 'm', 'l'], extra: '8°~16°',
        series: { unit: '°', points: [9.5, 10.2, 11.8, 13.6, 15.1, 16.2, 16.0, 15.2, 14.0, 12.6, 11.4, 10.6, 10.0, 9.5, 9.1, 8.8, 8.6, 8.4, 8.3, 8.4, 8.9, 9.6, 10.4, 11.2] },
        rows: [{ kind: 'hour', when: '16时', icon: 'cloud', value: '11°' }, { kind: 'hour', when: '17时', icon: 'partly', value: '13°' }, { kind: 'hour', when: '18时', icon: 'sun', value: '15°' }, { kind: 'hour', when: '19时', icon: 'sun', value: '16°' }, { kind: 'hour', when: '20时', icon: 'cloud', value: '14°' }, { kind: 'hour', when: '21时', icon: 'cloud', value: '11°' }, { kind: 'hour', when: '22时', icon: 'moon', value: '9°' }, { kind: 'day', when: '明天', icon: 'rain', value: '9°~14°', name: '小雨' }, { kind: 'day', when: '后天', icon: 'cloud', value: '7°~12°', name: '阴' }],
      },
    },
    { id: 'stock-aapl', plugin: 'stock', from: 'script', on: true, icon: 'stock', color: '#ff5c6c', label: '苹果', value: '230.40 +0.80%', detail: { sizes: ['s', 'm', 'l'], series: { points: [228.6, 228.9, 229.4, 229.1, 229.8, 230.2, 229.9, 230.6, 230.1, 230.4, 230.0, 230.5, 230.8, 230.4], hi: 231.2, lo: 228.1 }, rows: [{ code: 'AAPL', price: '230.40', change: '+0.80%', value: '4,812 万' }] } },
    { id: 'stock-nvda', plugin: 'stock', from: 'script', on: true, icon: 'stock', color: '#34d27b', label: '英伟达', value: '141.20 −1.30%', detail: { sizes: ['s', 'm', 'l'], series: { points: [143.1, 142.6, 142.9, 142.0, 141.7, 141.9, 141.2] }, rows: [{ code: 'NVDA', price: '141.20', change: '−1.30%' }] } },
    {
      id: 'inbox-a1', from: 'mail', on: true, icon: 'mail', color: '#5e9bff', private: true, label: 'Gmail', value: { key: 'mail.unread', vars: { n: '13' } },
      detail: { sizes: ['s', 'm', 'l'], rows: [{ from: 'Anna', subject: '周五见：我订好了那家餐厅', at: T0 - 3600e3 }, { from: 'GitHub', subject: '[wakuwaku] #52 merged into main', at: T0 - 5 * 3600e3 }, { from: 'Werner', subject: 'Summer School on Quantum Computing 2027', at: T0 - 26 * 3600e3 }, { from: 'Gitee', subject: '仓库 wakuwaku 收到一个新的 Star', at: T0 - 30 * 3600e3 }, { from: 'Cloudflare', subject: 'Your Worker relay was deployed', at: T0 - 80 * 3600e3 }, { from: 'Aliyun', subject: '函数计算 9 月账单', at: T0 - 100 * 3600e3 }] },
    },
    { id: 'pomodoro', plugin: 'pomodoro', from: 'script', on: true, icon: 'timer', color: '#ff5c6c', label: '专注', value: '18:42', detail: { sizes: ['s', 'm'], extra: '之后休息 5 分钟', series: { points: [1122], hi: 1500, lo: 0, unit: 's' }, rows: [{ kind: 'round', value: '3', unit: '4' }] } },
    { id: 'countdown', plugin: 'countdown', from: 'script', on: true, icon: 'flag', color: '#ffb340', label: 'v0.4 发布', value: '还有 3 天', detail: { sizes: ['s'], extra: '10 月 20 日 周二', rows: [{ kind: 'days', value: '3', unit: '天' }] } },
    // Scripts that sent no icon of their own: the first letter of their name
    // as their mark, one in its own colour, one with none (grey).
    { id: 'deploy', from: 'script', on: true, icon: 'dot', color: '#36cfc9', label: '部署', value: '成功', leftMs: 240e3 },
    { id: 'build', from: 'script', on: true, icon: 'dot', label: 'Build', value: '3:12', leftMs: 60e3 },
  ]
  const widget = id => WIDGETS.find(w => w.id === id)
  // Only these, in this order.
  const widgets = (...ids) => ids.map(widget).filter(Boolean)

  // The plugins she runs (the Plugins page): those on, and the others there to turn on.
  const PLUGINS = [
    { id: 'weather', icon: 'weather', on: true, state: 'running', params: [{ name: 'City', kind: 'text', required: false, value: 'Dresden' }] },
    { id: 'stock', icon: 'stock', on: true, state: 'running', params: [{ name: 'Symbol', kind: 'text', required: false, value: 'AAPL,NVDA' }, { name: 'Name', kind: 'text', required: false, value: '苹果,英伟达' }, { name: 'Colors', kind: 'choice', required: false, options: ['RedUp', 'GreenUp'], value: 'RedUp', chosen: false }] },
    { id: 'pomodoro', icon: 'timer', on: true, state: 'running', params: [{ name: 'Work', kind: 'number', required: false, value: '25' }, { name: 'Break', kind: 'number', required: false, value: '5' }] },
    { id: 'countdown', icon: 'flag', on: true, state: 'running', params: [{ name: 'Title', kind: 'text', required: true, value: 'v0.4 发布' }, { name: 'At', kind: 'text', required: true, value: '2026-10-20' }] },
    { id: 'ci', icon: 'check', on: false, state: 'off', params: [{ name: 'Repo', kind: 'text', required: true, value: '' }] },
    { id: 'devserver', icon: 'server', on: false, state: 'off', params: [{ name: 'Url', kind: 'text', required: true, value: '' }] },
    { id: 'stretch', icon: 'timer', on: false, state: 'off', params: [{ name: 'Minutes', kind: 'number', required: false, value: '50' }] },
  ]

  // What the plugins off above (PLUGINS) say once they are turned on (put
  // somewhere from the picker: fake-tauri.js widgetsNow).
  const SLEEPING = {
    ci: { id: 'ci-main', plugin: 'ci', from: 'script', on: true, icon: 'check', color: '#34d27b', label: 'wakuwaku · main', value: '通过', detail: { sizes: ['s', 'm'], extra: '3 分钟前' } },
    devserver: { id: 'devserver', plugin: 'devserver', from: 'script', on: true, icon: 'server', color: '#36cfc9', label: 'localhost:5173', value: '运行中' },
    stretch: { id: 'stretch', plugin: 'stretch', from: 'script', on: true, icon: 'timer', color: '#b18cff', label: '久坐提醒', value: '32 分钟后' },
  }

  // --- The taskbar ------------------------------------------------------------------------
  let apps = 0
  const app = (name, color, letter, titles, opts = {}) => {
    const n = ++apps
    const png = appIcon(color, letter)
    return {
      app: name.toLowerCase(),
      path: `C:\\Program Files\\${name}\\${name}.exe`,
      name,
      pinned: !!opts.pinned,
      png,
      windows: titles.map((title, i) => ({ id: 100 * n + i, title, png, front: !!opts.front && i === 0, min: false, max: false, flash: false, sessions: [] })),
    }
  }
  const WINDOWS = [
    app('Explorer', '#f2c23a', 'F', ['下载'], { pinned: true }),
    app('Edge', '#1f9be0', 'E', ['Wakuwaku · GitHub'], { pinned: true }),
    app('Terminal', '#3b3f4a', '>', ['pet — claude', 'mail — codex'], { front: true }),
    app('Code', '#2f80ed', 'C', ['island.js — pet']),
    app('Claude', '#d97757', '✳', ['Claude']),
    app('Notes', '#7a5af8', 'N', [], { pinned: true }),
  ]
  const TRAY = [
    { key: 1, name: 'onedrive', png: trayIcon('#2d7ff9', '☁'), tip: 'OneDrive', exe: 'OneDrive.exe', windowsOut: true },
    { key: 2, name: 'wechat', png: trayIcon('#2dc100', 'W'), tip: '微信', exe: 'Weixin.exe', windowsOut: true },
    { key: 3, name: 'steam', png: trayIcon('#1b2838', 'S'), tip: 'Steam', exe: 'steam.exe', windowsOut: false },
  ]
  const KEYS = { lang: 0x0804, native: true, caps: false, net: 'wifi', volume: [40, false], ime: { id: 'ms-pinyin', lang: '中文(简体，中国)', name: '微软拼音', own: true, mode: { tip: '中文', glyph: '中' } } }

  // --- The desktop (desk.rs layouts) ------------------------------------------------------
  const K = 'd1'
  const at = (c, r) => ({ d: K, at: { [K]: [c, r] } })
  const DESK = {
    // Each size of four kinds: a column each, large over medium over small.
    sizes: [
      { id: 'ws1', type: 'plugin', of: 'weather', size: 'l', ...at(0, 0) },
      { id: 'ws2', type: 'plugin', of: 'weather', size: 'm', ...at(0, 4) },
      { id: 'ws3', type: 'plugin', of: 'weather', size: 's', ...at(0, 6) },
      { id: 'wm1', type: 'widget', of: 'monitor', size: 'l', ...at(4, 0) },
      { id: 'wm2', type: 'widget', of: 'monitor', size: 'm', ...at(4, 4) },
      { id: 'wm3', type: 'widget', of: 'monitor', size: 's', ...at(4, 6) },
      { id: 'wt1', type: 'widget', of: 'today', size: 'l', ...at(8, 0) },
      { id: 'wt2', type: 'widget', of: 'today', size: 'm', ...at(8, 4) },
      { id: 'wt3', type: 'widget', of: 'today', size: 's', ...at(8, 6) },
      { id: 'wd1', type: 'widget', of: 'deploy', size: 'm', ...at(12, 0) },
      { id: 'wd2', type: 'widget', of: 'deploy', size: 's', ...at(12, 2) },
      { id: 'wd3', type: 'widget', of: 'build', size: 's', ...at(14, 2) },
    ],
    // One alone, a stack, a board open and one folded.
    scene: [
      { id: 'wa', type: 'plugin', of: 'weather', size: 'm', ...at(0, 0) },
      { id: 'wst', type: 'stack', size: 'm', ...at(0, 3), items: [{ type: 'widget', of: 'inbox-a1' }, { type: 'widget', of: 'today' }, { type: 'plugin', of: 'pomodoro' }] },
      {
        id: 'wbw', type: 'board', name: '工作', ...at(5, 0),
        items: [
          { type: 'widget', of: 'today', size: 'm', at: [0, 0] },
          { type: 'widget', of: 'inbox-a1', size: 'm', at: [4, 0] },
          { type: 'plugin', of: 'stock', size: 's', at: [0, 2] },
          { type: 'widget', of: 'monitor', size: 's', at: [2, 2] },
          { type: 'plugin', of: 'pomodoro', size: 's', at: [4, 2] },
          { type: 'plugin', of: 'countdown', size: 's', at: [6, 2] },
        ],
      },
      {
        id: 'wbf', type: 'board', name: '行情', folded: true, ...at(5, 6),
        items: [{ type: 'plugin', of: 'stock', size: 's', at: [0, 0] }, { type: 'plugin', of: 'weather', size: 's', at: [2, 0] }, { type: 'widget', of: 'deploy', size: 's', at: [4, 0] }],
      },
    ],
    // An everyday desktop (the playground's from the start): a stack, a
    // board, two alone at the far side.
    everyday: [
      { id: 'wds', type: 'stack', size: 'm', ...at(0, 0), items: [{ type: 'widget', of: 'inbox-a1' }, { type: 'plugin', of: 'pomodoro' }] },
      {
        id: 'wdb', type: 'board', name: '工作', ...at(0, 3),
        items: [{ type: 'widget', of: 'today', size: 'm', at: [0, 0] }, { type: 'widget', of: 'tokens', size: 's', at: [4, 0] }, { type: 'plugin', of: 'countdown', size: 's', at: [6, 0] }],
      },
      { id: 'wd1', type: 'widget', of: 'deploy', size: 's', ...at(16, 0) },
      { id: 'wd2', type: 'plugin', of: 'stock', size: 's', ...at(16, 2) },
    ],
  }
  // A full desktop: the scene's (one alone, a stack, two boards, one
  // folded) and two more alone at the far side.
  DESK.full = [...DESK.scene, { id: 'wfm', type: 'widget', of: 'monitor', size: 'm', ...at(14, 0) }, { id: 'wfd', type: 'widget', of: 'deploy', size: 's', ...at(14, 2) }, { id: 'wfb', type: 'widget', of: 'build', size: 's', ...at(16, 2) }]

  // --- The taskbar's layouts (taskbarLayout) -------------------------------------------------
  const PARTS = ['tray', 'keys', 'quick', 'clock']
  const TASKBAR = {
    // Windows 11's: her at the left, Start and the programs in the middle,
    // the monitor, the tray, the keyboard, quick settings and the clock.
    win11: { left: ['her'], center: ['start', 'apps'], right: ['monitor', ...PARTS] },
    // Widgets put on the strip: one medium, a carousel and a stack.
    widgets: {
      left: ['her'],
      center: ['start', 'apps'],
      right: [
        { id: 'wtw', type: 'plugin', of: 'weather', size: 'M', icon: 'weather' },
        { id: 'wtc', type: 'carousel', items: [{ type: 'widget', of: 'inbox-a1', icon: 'mail' }, { type: 'plugin', of: 'stock' }] },
        { id: 'wts', type: 'stack', items: [{ type: 'widget', of: 'monitor', icon: 'gauge' }, { type: 'widget', of: 'today', icon: 'today' }, { type: 'plugin', of: 'countdown' }] },
        ...PARTS,
      ],
    },
    // As full as one would make it: a medium by her; a medium, a carousel
    // and a stack at the right (more is wider than a 1600 px strip has room for).
    full: {
      left: ['her', { id: 'wfp', type: 'plugin', of: 'pomodoro', size: 'M', icon: 'timer' }],
      center: ['start', 'apps'],
      right: [
        { id: 'wfw', type: 'plugin', of: 'weather', size: 'M', icon: 'weather' },
        { id: 'wtc', type: 'carousel', items: [{ type: 'widget', of: 'inbox-a1', icon: 'mail' }, { type: 'plugin', of: 'stock' }] },
        { id: 'wts', type: 'stack', items: [{ type: 'widget', of: 'today', icon: 'today' }, { type: 'widget', of: 'monitor', icon: 'gauge' }, { type: 'widget', of: 'tokens', icon: 'code' }] },
        ...PARTS,
      ],
    },
  }

  const api = { T0, appIcon, trayIcon, session, others, reading, ASK, WIDGETS, widget, widgets, PLUGINS, SLEEPING, WINDOWS, TRAY, KEYS, DESK, TASKBAR }
  if (typeof module !== 'undefined' && module.exports) module.exports = api
  else root.Fixtures = api
})(typeof window !== 'undefined' ? window : globalThis)

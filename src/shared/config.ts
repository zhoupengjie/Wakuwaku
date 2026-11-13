// Her settings as the pages hold them: config.json, as the snapshot gives it
// (settings.rs) and a patch sets it. The keys the Rust side reads are
// declared in src-tauri/src/config.rs, each with its kind and default, and
// their lines here are written from there (its test the_pages_config_is_
// written_from_here; test/config.test.ts looks too): the type its kind
// reads (a switch is boolean), the words it may be set to where they are a
// few (its check there: 'auto' | 'zh' | 'en'), else by its default. Then
// the keys the pages alone keep and set by other ways than a patch. Every key may be missing (a config from before it): optional.
// Types alone: nothing of this is in the built page.
export interface Config {
  // --- Declared in config.rs (written from it: cargo test config, CONFIG_TS_BLESS=1) ---
  lang?: 'auto' | 'zh' | 'en'
  pet?: string
  scale?: number
  display?: string
  out?: boolean
  islandWidth?: 'narrow' | 'normal' | 'wide'
  islandPos?: string
  islandEdge?: string
  islandEmbed?: boolean
  islandHideUnder?: boolean
  taskbarMaterial?: 'mica' | 'black' | 'clear'
  homeDisplay?: string
  taskbarDisplays?: 'main' | 'all'
  taskbarClearWhen?: 'mica' | 'solid' | 'always'
  taskbarButtons?: 'icons' | 'labels'
  taskbarAlign?: 'center' | 'left'
  taskbarEdge?: 'bottom' | 'top' | 'left' | 'right'
  splitBar?: number
  splitDock?: 'hide' | 'room'
  splitKeys?: 'top' | 'dock'
  oledShift?: boolean
  autoUpdateCheck?: boolean
  updateFrom?: 'auto' | 'github' | 'gitee'
  details?: boolean
  widgetsOff?: any[]
  monitor?: Record<string, any>
  widgetOrder?: any[]
  widgetSpin?: number
  widgetNudge?: boolean
  plugins?: Record<string, any>
  mail?: any[]
  mailCodes?: boolean
  walk?: boolean
  look?: boolean
  hold?: number | 'seen'
  endCue?: 'open' | 'ring' | 'none'
  idleHideMin?: number
  notify?: Record<string, any>
  sound?: boolean
  promptWaitSec?: number
  appAsks?: boolean
  dnd?: boolean
  hideInFullscreen?: boolean
  onboarded?: boolean
  shortcutsEnabled?: boolean
  shortcutApprove?: string
  shortcutDeny?: string
  shortcutIsland?: string
  lookDefaults?: number
  connectHintOff?: boolean
  crashTold?: string
  crashReports?: 'auto' | 'off' | 'ask'
  deskLayout?: any
  dshVia?: 'plugin' | 'bridge'
  islandLook?: any
  islandTopLook?: any
  mailAgentConf?: any
  mailOffline?: '90d' | 'year' | 'all' | 'opened'
  mailProxy?: any
  monitorSource?: any
  netProxy?: any
  pluginAutoUpdate?: boolean
  settingsPin?: boolean
  settingsTop?: boolean
  taskbarLayout?: any
  taskbarOrder?: any
  taskbarPinned?: any
  taskbarPinsImported?: boolean
  trayPinned?: any
  updateTold?: string
  welcomeOpened?: boolean
  x?: number
  y?: number
  capsule?: any
  boardSets?: any
  widgetSize?: Record<string, string>
  mailAgent?: 'claude' | 'codex' | 'agy'
  mailPanes?: 'one' | 'two'
  mailPictures?: string[]
  // --- The pages' own ---
  // The tray's icons in the order she shows them (island-look.ts trayOrder).
  trayOrder?: any
}

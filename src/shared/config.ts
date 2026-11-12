// Her settings as the pages hold them: config.json, as the snapshot gives it
// (settings.rs) and a patch sets it. The keys the Rust side reads are
// declared in src-tauri/src/config.rs, each with its kind and default, and
// their lines here are written from there (its test the_pages_config_is_
// written_from_here; test/config.test.ts looks too): the type its kind
// reads (a switch is boolean), else by its default. Then the keys the pages
// alone keep. Every key may be missing (a config from before it): optional.
// Types alone: nothing of this is in the built page.
export interface Config {
  // --- Declared in config.rs (written from it: cargo test config, CONFIG_TS_BLESS=1) ---
  lang?: string
  pet?: string
  scale?: number
  display?: string
  out?: boolean
  islandWidth?: string
  islandPos?: string
  islandEdge?: string
  islandEmbed?: boolean
  islandHideUnder?: boolean
  taskbarMaterial?: string
  homeDisplay?: string
  taskbarDisplays?: string
  taskbarClearWhen?: string
  taskbarButtons?: string
  taskbarAlign?: string
  taskbarEdge?: string
  splitBar?: number
  splitDock?: string
  splitKeys?: string
  oledShift?: boolean
  autoUpdateCheck?: boolean
  updateFrom?: string
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
  endCue?: string
  idleHideMin?: number
  notify?: Record<string, any>
  sound?: boolean
  promptWaitSec?: number
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
  crashReports?: string
  deskLayout?: any
  dshVia?: any
  islandLook?: any
  islandTopLook?: any
  mailAgentConf?: any
  mailOffline?: any
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
  // --- The pages' own ---
  // Her capsule's members and how each turns (capsule.ts).
  capsule?: any
  // The boards' combos kept to put up again (deskgrid.ts setsOf).
  boardSets?: any
  // The tray's icons in the order she shows them (island-look.ts trayOrder).
  trayOrder?: any
  // Each widget's size where its person set it, by id (island.ts; addons.rs
  // drops a plugin's when it goes).
  widgetSize?: Record<string, string>
  // The Mail page's: who a letter goes to first ("claude", "codex" or
  // "agy"), the page as two panes or one ("two" or "one"), and the senders
  // whose letters show their pictures from elsewhere, in lower case
  // (settings-mail.ts, settings-letter.ts; settings.rs checks each).
  mailAgent?: string
  mailPanes?: string
  mailPictures?: string[]
}

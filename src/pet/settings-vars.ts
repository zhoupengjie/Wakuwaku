// What the settings remember (settings.ts and its parts, settings-*.ts):
// every value that changes as they are used, in one object made as the
// settings are (settingsVars: st in settingsPage, handed to each part in
// its core), read and written as st.<name> wherever it is used; where
// they are (SettingsState: st.isOpen, st.tab, st.sub...) read alone, moved
// on by settings-state.ts's events (settingsPage go). Only types
// and the values they start at: nothing here runs or touches the page. (The
// objects whose insides change, the gallery, the market, the inbox and the
// like, stay consts in settingsPage.)
import type * as AddonUI from './addonui.ts'
import { initial, type SettingsState } from './settings-state.ts'
import type { Config } from '../shared/config.ts'
import type { Widget } from './widgets.ts'

// Her settings as the snapshot has them and a patch changes them
// (settings.rs settings_set): the config's keys, and the Mail page's that
// mail.rs reads besides (settings.rs is_ok takes them; config.rs does not
// declare them).
export type Settings = Config & { mailAgent?: string; mailPanes?: string; mailPictures?: string[] }

// A plugin she runs (plugins.rs view), someone else's (addons.rs view)
// and a pack (packs.rs view): the Rust side's JSON, these fields read by
// name, the rest as it comes.
export interface Plugin {
  id: string
  [field: string]: any
}
export interface Addon {
  id: string
  name: string
  settings?: AddonField[]
  [field: string]: any
}
export interface Pack {
  kind: string
  id: string
  name: string
  [field: string]: any
}
// One of a plugin's settings, from its manifest: text, number, flag or
// choice (its options' own words); as set, else its default.
export interface AddonField {
  key: string
  type?: string
  label?: string
  value?: unknown
  default?: unknown
  options?: string[]
  optionLabels?: string[]
  required?: boolean
  hint?: string
}

// A mail account (mail.rs, the setting mail): its id, address and
// incoming server read by name, the rest of its JSON (the outgoing one,
// its proxy, on, its status) as it comes.
export interface MailAccount {
  id: string
  address: string
  host: string
  port: number
  [field: string]: any
}

// What main sends (settings.rs snapshot): her settings, and the lists the
// pages go through, each entry the Rust side's JSON, as is all the rest.
export interface Snapshot {
  settings: Settings
  pets: any[]
  sessions?: any[]
  packs?: Pack[]
  widgets?: Widget[]
  plugins?: Plugin[]
  addons?: Addon[]
  addonsBroken?: any[]
  marketUpdates?: any[]
  mail?: MailAccount[]
  displays?: any[]
  asks?: any[]
  shortcutsFailed?: string[]
  [field: string]: any
}

// Taken away once asked again over the panel: a plugin by its id, a pack
// by its kind too, a pet deleted (del); busy while it goes.
export interface Gone {
  id: string
  kind?: string
  del?: boolean
  busy: boolean
}

// Asked over the panel in the market: installing one, or 全部更新.
export type MarketAsk = { kind: 'install'; id: string } | { kind: 'updates'; id?: undefined }

// A word for a few seconds (bad: what went wrong).
export interface Note {
  text: string
  bad?: boolean
}

// What a server answered to Re-test: how it is reached, how one signs in.
export interface Probed {
  security: string
  port: number | string
  auths: string[]
}

// A mail account as its form has it. A port is a number as found, a
// string as typed. step: start (the address and password), found (the
// servers looked up), manual (their fields to fill in); busy: find,
// probe, save or ''. result: what came of the last try, the server's own
// words (said) and where to put it right (fix).
export interface MailForm {
  id: string
  address: string
  name: string
  password: string
  host: string
  port: number | string
  security: string
  auth: string
  username: string
  smtpHost: string
  smtpPort: number | string
  smtpSecurity: string
  smtpAuth: string
  smtpUser: string
  proxyMode: string
  proxyKind: string
  proxyAddress: string
  step: string
  source?: string
  notFound?: boolean
  oauth?: boolean
  advanced: boolean
  probed: Probed | null
  probedOut: Probed | null
  findingOut?: boolean
  busy: string
  result: { bad: boolean; text: string; said?: string; fix?: { site: string; label: string } | null } | null
}
// Its boxes typed in (data-mf), and its choices picked from a list (data-mfpick).
export type MailBox = 'address' | 'name' | 'password' | 'host' | 'port' | 'username' | 'smtpHost' | 'smtpPort' | 'smtpUser' | 'proxyAddress'
export type MailPick = 'security' | 'auth' | 'smtpSecurity' | 'smtpAuth' | 'proxyMode' | 'proxyKind'

// A letter by where it is: its account, its folder, its UID there.
export interface LetterAt {
  account: string
  folder: string
  uid: number
}

// A letter as letters.rs gives it: its JSON, many fields, each read where
// it is drawn.
export type Letter = any
// A conversation (threads.rs): its key, whose, its letters, who wrote
// (oneself: me), the letter a click opens; the rest as it comes.
export interface Thread {
  key: string
  account: string
  letters: Letter[]
  people: { name?: string; me?: boolean }[]
  open?: { folder: string; uid: number } | null
  [field: string]: any
}
// A talk with an agent about a letter (agent.rs): with whom, how it goes
// (running, failed, done), what went wrong, what was said in turns (who:
// start, me, tool, agent).
export interface Talk {
  agent: string
  state: string
  error?: string
  turns: { who: string; text: string }[]
}

// The letter open: busy while it is read, then the letter and its talk,
// or what went wrong (mail.rs: its JSON).
export interface Reading extends LetterAt {
  busy?: boolean
  letter?: Letter
  talk?: Talk | null
  error?: any
}

// A file added to a letter: read whole, as base64.
export interface DraftFile {
  name: string
  type: string
  size: number
  data: string
}

// The letter being written: new, reply, all or forward; from which
// account, to whom, the subject and text, the letter it answers or passes
// on (that one's files, each by its index there), files added; touched
// once typed in; sending, what went wrong, a word about it.
export interface Draft {
  kind: string
  account: string
  to: string
  cc: string
  bcc: string
  showCc: boolean
  subject: string
  text: string
  reply: (LetterAt & { messageId?: string; references: string[] }) | null
  forward: (LetterAt & { files: { index: number; name: string; size: number }[] }) | null
  files: DraftFile[]
  touched: boolean
  busy: boolean
  error: string | null
  note: string | null
}
// Its boxes typed in (data-w).
export type DraftBox = 'to' | 'cc' | 'bcc' | 'subject' | 'text'

// Windows' two modes (light, dark).
export type WinMode = 'dark' | 'light'

export interface SettingsVars extends SettingsState {
  // --- Open --------------------------------------------------------------------------
  // What main last sent (settings.rs snapshot: the settings, the sessions,
  // the agents, the plugins...), and the language it is in. None until
  // the first comes as they open: read once they are drawn (draw waits).
  snap: Snapshot
  lang: string
  // The seconds' clock while open (tick).
  clockTimer: Timer | undefined

  // --- General -------------------------------------------------------------------------
  // The crash report shown, as it would be sent (pageCrash).
  crashShown: boolean
  // Custom picked for how long endings stay, its box shown before anything
  // is typed in it (until the settings close or another choice is picked).
  holdCustom: boolean
  // A shortcut being recorded (its key; her global shortcuts let go
  // meanwhile, setRecording), and those Windows would not give her.
  recordingShortcut: string | null
  failedShortcuts: Set<string> | null
  // The Agent page: an agent being connected, the plugin's row open.
  connecting: string
  pluginOpen: boolean

  // --- Look --------------------------------------------------------------------------
  // The download box: what is typed, and how the last download went.
  typedRef: string
  fetching: string | null
  fetchNote: { text: string; isError: boolean }
  // The gallery's search waiting for the typing to stop, and its thumbnails'.
  searchTimer: Timer | undefined
  thumbTimers: Timer[]
  // Her island's look folded out (island-look.ts drawIslandLook), and the
  // mode being set ('' follows Windows' own).
  ilookOpen: boolean
  ilookMode: WinMode | ''
  // Windows' dialog open for the desktop's picture: its own mark, held until
  // the dialog answers (picking, a letter's files', is let go a second after
  // the island has the focus again, which a press on it mid-dialog gives).
  pickingPaper: boolean
  // Windows' question for LibreHardwareMonitor (run as administrator) takes
  // the focus too: from the press, and while its job is at that stage.
  lhmAsking: number

  // --- Plugins -----------------------------------------------------------------------
  // The plugin whose settings are folded out.
  openPlugin: string
  // Other people's plugins (addons.rs; addonui.ts the logic; the part of
  // the Plugins page opened in its place is st.sub): the confirm layer over
  // the panel (installing from a file, by AddonUI.next's steps; taking one
  // away: gone { id, busy }, a pack's with its kind), Windows' dialog open
  // for a .zip (its focus does not close them).
  flow: typeof AddonUI.IDLE
  gone: Gone | null
  pickingZip: boolean
  // A plugin whose newly-asked permissions are being allowed (finding 1).
  approveFor: string | null
  // Asked over the panel in the market: installing one that asks for more
  // than the web ({ kind: 'install', id }), or 全部更新 when one asks for
  // more ({ kind: 'updates' }).
  mkAsk: MarketAsk | null

  // --- Mail ----------------------------------------------------------------------------
  // The mail account being set up or changed, as Thunderbird's dialog has
  // it: the address and password; then (step found) the server looked up,
  // or (step manual) the server's fields; busy while looking or signing in;
  // what came of the last try. Null while none is.
  mailForm: MailForm | null
  // An account's delete asked once: asked again, it goes.
  mailDelete: string
  // A proxy address as typed and not yet kept (key: mailProxy, netProxy): being typed,
  // or (bad) not read as address:port.
  proxyTyped: { key: string; text: string; bad: boolean } | null
  // A letter to open once the Mail page shows (a click on the island's word
  // that an agent answered); the letter open ({ uid, busy | letter, summary
  // | error }, null while the list shows); what came of handing one to an
  // agent, for a few seconds.
  letterToOpen: { account: string; uid: number; folder?: string } | null
  reading: Reading | null
  handNote: Note | null
  handNoteTimer: Timer | undefined
  // The letter being written, one at a time: from which account, to whom,
  // the subject and text, the letter it answers or passes on, files added;
  // touched once typed in; sending, what went wrong. Kept until sent or
  // thrown away, across a restart too (in the data folder, mail.rs
  // mail_draft: each copy of the pet its own, which the page's storage is
  // not; its files left out). writing: it shows; else the inbox does, with
  // a line to go back to it. A discard asked once: asked again, it goes.
  draft: Draft | null
  writing: boolean
  dropSure: boolean
  draftTimer: Timer | undefined
  // The file picker is open for a letter's files: the focus it takes does
  // not close the settings (the desktop's picture has its own, pickingPaper).
  picking: boolean
  // The same menu as a picker: a model or an effort, the one set ticked.
  menuPick: ((value: string) => void) | null
}

export function settingsVars(idleFlow: typeof AddonUI.IDLE): SettingsVars {
  return {
    ...initial(),
    snap: null!,
    lang: 'en',
    clockTimer: undefined,

    crashShown: false,
    holdCustom: false,
    recordingShortcut: null,
    failedShortcuts: null,
    connecting: '',
    pluginOpen: false,

    typedRef: '',
    fetching: null,
    fetchNote: { text: '', isError: false },
    searchTimer: undefined,
    thumbTimers: [],
    ilookOpen: false,
    ilookMode: '',
    pickingPaper: false,
    lhmAsking: 0,

    openPlugin: '',
    flow: idleFlow,
    gone: null,
    pickingZip: false,
    approveFor: null,
    mkAsk: null,

    mailForm: null,
    mailDelete: '',
    proxyTyped: null,
    letterToOpen: null,
    reading: null,
    handNote: null,
    handNoteTimer: undefined,
    draft: null,
    writing: false,
    dropSure: false,
    draftTimer: undefined,
    picking: false,
    menuPick: null,
  }
}

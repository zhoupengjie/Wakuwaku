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

export interface SettingsVars extends SettingsState {
  // --- Open --------------------------------------------------------------------------
  // What main last sent (settings.rs snapshot: the settings, the sessions,
  // the agents, the plugins...), and the language it is in.
  snap: any
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
  fetching: any
  fetchNote: { text: string; isError: boolean }
  // The gallery's search waiting for the typing to stop, and its thumbnails'.
  searchTimer: Timer | undefined
  thumbTimers: Timer[]
  // Her island's look folded out (island-look.ts drawIslandLook), and the
  // mode being set ('' follows Windows' own).
  ilookOpen: boolean
  ilookMode: string
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
  flow: any
  gone: any
  pickingZip: boolean
  // A plugin whose newly-asked permissions are being allowed (finding 1).
  approveFor: any
  // Asked over the panel in the market: installing one that asks for more
  // than the web ({ kind: 'install', id }), or 全部更新 when one asks for
  // more ({ kind: 'updates' }).
  mkAsk: any

  // --- Mail ----------------------------------------------------------------------------
  // The mail account being set up or changed, as Thunderbird's dialog has
  // it: the address and password; then (step found) the server looked up,
  // or (step manual) the server's fields; busy while looking or signing in;
  // what came of the last try. Null while none is.
  mailForm: any
  // An account's delete asked once: asked again, it goes.
  mailDelete: string
  // A proxy address as typed and not yet kept (key: mailProxy, netProxy): being typed,
  // or (bad) not read as address:port.
  proxyTyped: any
  // A letter to open once the Mail page shows (a click on the island's word
  // that an agent answered); the letter open ({ uid, busy | letter, summary
  // | error }, null while the list shows); what came of handing one to an
  // agent, for a few seconds.
  letterToOpen: any
  reading: any
  handNote: any
  handNoteTimer: Timer | undefined
  // The letter being written, one at a time: from which account, to whom,
  // the subject and text, the letter it answers or passes on, files added;
  // touched once typed in; sending, what went wrong. Kept until sent or
  // thrown away, across a restart too (in the data folder, mail.rs
  // mail_draft: each copy of the pet its own, which the page's storage is
  // not; its files left out). writing: it shows; else the inbox does, with
  // a line to go back to it. A discard asked once: asked again, it goes.
  draft: any
  writing: boolean
  dropSure: boolean
  draftTimer: Timer | undefined
  // The file picker is open for a letter's files: the focus it takes does
  // not close the settings (the desktop's picture has its own, pickingPaper).
  picking: boolean
  // The same menu as a picker: a model or an effort, the one set ticked.
  menuPick: any
}

export function settingsVars(idleFlow: typeof AddonUI.IDLE): SettingsVars {
  return {
    ...initial(),
    snap: null,
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

// index.html's one script: the page of her own window (role=pet) and of her
// home's (role=island). What the page is made of, made here in the order it
// has always been made in (each one's listeners in that order too), each
// given what it works with: her (pet.ts), her home (island.ts), the picker
// and her capsule's panel (with the island on), the handover from Windows'
// taskbar and the plugins (her home's page alone), the settings, and the
// prompts' panel. The island and the picker work with what is made after
// them (the settings; her capsule's panel): theirs once made (later).
//
// Each is made apart (entry.ts made: one that fails is none, the rest made
// without it), and what each hands back is put on window too (expose), for
// what looks in from outside alone: the preview, /debug/eval and the test
// VM's scripts (window.Island.isOpen(), window.Settings.close()...).
import { pet } from './bridge.ts'
import { herPage } from './pet.ts'
import { islandPage } from './island.ts'
import { pickerPage } from './picker.ts'
import { capsulePanel } from './capsule-panel.ts'
import { handoff } from './handoff.ts'
import { addonsPage } from './addons.ts'
import { settingsPage } from './settings.ts'
import { panelPage } from './panel.ts'
import { expose, made } from './entry.ts'
import type { IslandLater } from './island.ts'
import type { PickerLater } from './picker.ts'

const ROLE = new URLSearchParams(location.search).get('role') === 'island' ? 'island' : 'pet'

const HerTalk = made(herPage)
expose({ HerTalk })

const islandLater: IslandLater = { Settings: null, Picker: null }
const island = made(() => islandPage(islandLater))
const Island = island?.face ?? null
const Isle = island?.isle ?? null
expose({ Island, Isle })

// Only with the island on.
const pickerLater: PickerLater = { CapsulePanel: null }
const Picker = Isle?.isOn() ? made(() => pickerPage(Isle, pickerLater)) : null
islandLater.Picker = Picker
expose({ Picker })

// Only with the island and the picker there.
const CapsulePanel = Isle && Picker ? made(() => capsulePanel(Isle, Picker)) : null
pickerLater.CapsulePanel = CapsulePanel
expose({ CapsulePanel })

if (ROLE === 'island') made(handoff)
// Her home's page runs the plugins.
if (ROLE === 'island' && pet?.addons) made(() => addonsPage(Island))

// Her own window's page has none: isOpen alone, always false.
const Settings = ROLE === 'island' ? made(() => settingsPage(Island, Picker)) : { isOpen: () => false }
islandLater.Settings = Settings
expose({ Settings })

made(() => panelPage(Island, HerTalk))

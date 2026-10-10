# Wakuwaku

English · [中文](README.md)

> The name is Anya's "waku waku" from SPY×FAMILY: excited, can't wait. Claude works, and she watches, all eager.

A little pet that floats on your desktop and shows what Claude Code is doing: working, waiting for your approval, done with changes for you to review, done, or stuck on an error. When Claude needs your OK, you can answer right on the panel above her head. She can watch [Codex](#3-connect-codex-optional) too.

She uses the v2 sprite format from the [Codex Pets](https://codex-pets.net) community, so any v2 pet on that site works. The default is [Claude小姐](https://codex-pets.net/#/pets/claude-chan), downloaded from the site on first run.

## What she does

Each v2 pet has 11 animations, and every one of them has a job:

Both v2 pets (11 animations) and older v1 pets (9, without looking around; she just looks ahead when idle) work.

| Animation | When | From which hook |
| --- | --- | --- |
| Idle | Nothing going on (a blink, then a few still seconds) | After you've seen an ending (mouse over her); `SessionEnd` |
| Running | Claude is working; the island shows how long this turn has taken | `UserPromptSubmit`, `PreToolUse`, `PostToolUse` |
| Waiting | Waiting on you: a permission prompt, a question, a plan to approve, an MCP form | `PermissionRequest`; `PreToolUse` (AskUserQuestion / ExitPlanMode); `Elicitation` |
| Review | The turn edited files and is done, ready for you to look | `Stop` (after Edit / Write and friends ran this turn) |
| Waving | The turn is done without edits; hello when a session starts or she appears | `Stop`, `SessionStart` |
| Jumping | A task on the to-do list is done; you clicked her | `TaskCompleted` |
| Failed | A request failed (stays until you've seen it); a tool call failed (a quick flash) | `StopFailure`; `PostToolUseFailure` |
| Run right / left | A short stroll when idle; running the way you drag her | — |
| Look around (16 directions) | Her eyes follow the mouse when idle; now and then she glances around | — |

- **Several Claude Code sessions at once**: each keeps its own state. She shows the one that most needs you (waiting > error > review > done > working) and tells you how many others are busy; when another one is waiting on you, the bubble's second line names it.
- **Where it is, at a glance**: a session goes by its name in Claude Code (from `/rename` or the desktop app's title; else the first thing you asked). While it works she says the step it is on: "Editing island.rs", "$ cargo test", "Agent: explore the code", "Thinking", with "for 1:12" once a step runs long, and "3/7" and a progress bar when it keeps a to-do list. Waiting, she says on what ("Needs your OK: $ git push"); done, how many files changed and a line of Claude's reply; failed, why ("Usage limit reached"). The project is the git repository's name, worktrees included. When sharing your screen, turn off Show the specifics on the Look page and only the project and the status show.
- **Endings don't slip by**: done, review and error stay until the mouse passes over her, or for a few seconds if you prefer. System notifications and a sound are optional.
- **Context compaction** also fires `SessionStart`; she ignores it and doesn't drop back to idle mid-work.

## Answer Claude on the pet

| What | What the panel lets you do |
| --- | --- |
| Permission ("Allow Bash to run npm test?") | **Allow** / **Always allow** / **Deny**, with the command and project shown |
| A question (AskUserQuestion) | Pick options, type your own answer, fill text and number questions; several questions and multi-select work |
| A plan (ExitPlanMode) | **Approve** / **Deny** |

- The terminal's own dialog still appears, and **whichever answer comes first wins**: answer on the pet and the terminal dialog closes; answer in the terminal and the panel goes away.
- **Always allow** adds the rules Claude Code itself suggests (the same as "don't ask again" in the terminal), and the panel spells out which rule goes where. It only ever adds allow rules.
- Buttons stay disabled for 0.6 s after a panel appears, so a click already on its way can't hit them. To type, click the box first; she takes the keyboard only then, never from the window you're typing in.
- How long the panel waits is up to you (30 s to 5 min); after that the terminal has it. In do-not-disturb, or while an app is full screen, prompts go straight to the terminal.
- Limits:
  - After you answer in the terminal, the panel closes on that session's next event, so it can linger for a moment. Clicking it then does nothing.
  - Non-interactive runs such as `claude -p` wait for the hook before deciding, so with nobody clicking they wait until the panel gives up.
  - MCP forms (Elicitation) are announced, not filled in.
- Codex's permission prompts come to the panel too, with **Allow** / **Deny** only (Codex takes no rules from a hook). Codex shows its own dialog only once the panel lets go, so the panel waits 1 minute at most; to answer in the terminal, click Handle in terminal.

## Install

### 1. Get the pet

- **One exe (Windows)**: `wakuwaku.exe` is about 5 MB, runs from any folder and keeps her settings and pets in a `wakuwaku-data` folder beside it; to remove her, delete the two. It uses the WebView2 that comes with Windows 11, nothing else to install.

Download `wakuwaku-<version>-windows-x64.exe` from [Releases](https://github.com/zhoupengjie/Wakuwaku/releases); there is nothing to install. The exe is not code-signed, so on first run Windows SmartScreen may stop it: click More info → Run anyway. Or build it yourself, see below.
- **From source**: see below.

On first launch she downloads the default pet, Claude小姐 (online; offline, the settings rise at the top of the screen so you can pick one later). For another, click the island to open the settings and pick one on the Pets page (the gallery comes straight from codex-pets.net; click Download), or paste a pet page URL.

### 2. Connect Claude Code (the plugin is recommended)

In Claude Code, enter (the Connect page of the settings has copy buttons):

```
/plugin marketplace add zhoupengjie/wakuwaku
/plugin install wakuwaku@wakuwaku
```

Claude Code installs and removes the plugin itself, and **we don't touch your settings file** (Claude Code notes it in its own `enabledPlugins` and takes it out on uninstall). A plugin can't start the pet, so turn on Start at login on the Connect page.

If you'd rather not use a plugin, Advanced on the Connect page writes the hooks into `~/.claude/settings.json` (backed up first; removing takes out only our entries). In return, new sessions start the pet. With both on, the Connect page warns that events arrive twice.

### 3. Connect Codex (optional)

Click Install in the Codex part of the Connect page. It writes `~/.codex/hooks.json` (backed up first; removing takes out only our entries). Then **enter `/hooks` in Codex and trust Wakuwaku's hooks**: Codex runs only hooks you trusted, and the Connect page reminds you until the first event arrives. If the app moves, click Repair and trust them again.

How it differs from Claude Code:

| | Claude Code | Codex |
| --- | --- | --- |
| Working, waiting, done, review | ✓ | ✓ (a turn that ran `apply_patch` ends in review) |
| Errors | ✓ | No (Codex sends no event when a request fails; she goes back to idle after 15 minutes) |
| A jump when a to-do item is done | ✓ | No |
| Answer on the panel | Allow / Always allow / Deny, questions, plans | Allow / Deny |
| New sessions start the pet | With the settings.json way | Yes |

Codex hooks can only be commands: `wakuwaku.exe --wakuwaku-codex-hook` runs once and hands the event to her. On Windows Codex runs hooks in PowerShell, about 0.3 s to start each time, so Codex waits for it only when a turn starts and ends, on an edit (`apply_patch`), on a prompt, and when a session starts and ends; each step's tool is reported in the background. Background hooks need Codex 0.148 or later; an older one gets only the essential hooks, and the bubble doesn't name each tool.

The pets in Codex's desktop app (`~/.codex/pets`) use the same format, so they show up on the Pets page, marked From Codex.

### From source

Needs Rust (the MSVC toolchain on Windows). Only tested on Windows 11 so far; Tauri builds on macOS and Linux too, but full-screen detection, start at login, notifications and the button state are written for Windows only.

```bash
git clone https://github.com/zhoupengjie/wakuwaku.git
cd wakuwaku/src-tauri
cargo run                 # debug
cargo build --release     # target/release/wakuwaku.exe, one file
```

No Node and no Tauri CLI needed. The first run downloads the default pet; connect Claude Code from the Connect page of the settings. Run from source, her settings, downloaded pets and log live in the project's `data/` folder (not in git); the first run copies over whatever settings and pets the old version had in `%APPDATA%\wakuwaku`.

## Use

- **Drag** her anywhere; she runs the way you drag, and remembers the spot.
- **Click** her: she jumps, and the window of the session she shows comes to the front. **Double-click**: out on the desktop, she flies back home.
- **A session takes you to its window**: click a session in the open island, a row on the Now page of the settings, or Handle in terminal on a prompt, and the window it runs in comes to the front: Windows Terminal, a console window, VS Code, or the Claude desktop app (which opens that very session). When the session's process has ended, the island says it can't find its window. Windows Terminal comes to the front as a window, not at the tab.
- **Mail**: on the Mail page of the settings, as in Thunderbird: give the address and password, and the server is looked up (or type in the server, port and security). Advanced settings fold out below, as in Thunderbird's manual setup: hostname, port, connection security, authentication method and user name, with Re-test to see what the server takes without signing in. TU Dresden mailboxes (tu-dresden.de, mailbox.tu-dresden.de) are built in: the address and the ZIH password will do. Once it signs in, new mail pops up in the island with who it is from and what about, and the unread count shows otherwise (the island marks nothing read). Below is the inbox: with several mailboxes, all of them as one by default, or any one (each with its unread count); all letters, the unread or the starred; a star on any letter. Click a letter to read it; right-click one (or open it and press Hand to…) to hand it to Claude Code or Codex: the answer shows under the letter as it is written, with a box below to ask more ("write the reply"), the island says when it answered and a click there brings the letter back; the talk is kept for next time. A terminal session is in the right-click menu too. Hand letters to picks which comes first; for each agent you choose what it may do (read only by default: Claude runs no commands and has no web, Codex sits in its read-only sandbox), and its model and effort. What a letter asks for, the agent tells you; it does not do it. Sending is not there yet. QQ, 163 and Gmail take an authorization code or app password; Outlook and Hotmail take only a browser sign-in, not supported yet. The password is kept in Windows Credential Manager.
- **Plugins in the island**: while no session needs you, plugins take turns in the island, like "Today 12 turns · 1:43 · 5 approved", or the weather, or a stock; sessions always come first. Hover to see them all, scroll to turn to the next, click one to keep it. Built in: Today and Today's tokens (Claude Code's and Codex's), on by default; and the Monitor, off: CPU, memory, network and battery, each with a switch, shown in the island as icons and numbers, updated every 1 to 10 seconds (2 by default). She can also run some for you in the background: weather, stocks/rates/coins, a pomodoro, a countdown, a stretch reminder, GitHub CI and dev servers; switch one on in the Plugins page and give it a city or a repository, no terminal needed, and they start with her. Any other is a script sending a few words to `127.0.0.1:47213/widget`. [examples/widgets](examples/widgets) has `waku <command>` (tells you when it is done), CI and pull requests, a pomodoro, deadlines, dev servers, stocks/rates/coins, weather, a stretch reminder, and mail (IMAP, Microsoft 365 / Outlook.com, and a [Thunderbird extension](integrations/thunderbird)). A private widget such as mail shows only its count while Show the specifics is off. A plugin may open the island once to tell you something (before a meeting, say), but never while a session waits on you. The Plugins page of the settings turns them on and off, orders them, and sets how often they take turns.
- **Right-click** her (or the tray icon): switch pets, size, walking, follow the mouse, her home (the island, the taskbar), let her out or call her back home, back to the corner, do not disturb, settings, quit. Left-click the tray icon to show or hide her (in do not disturb, the settings). The tray face changes with her mood: idle, working, waiting, done, review, error.
- **Her home, two ways** (Look → Display in the settings):
  - **The island**: below.
  - **The taskbar**: in place of Windows' own, which is put away while it is up. Her and the session that most needs you at the left, then Start and the programs that are open (one button per program, with live previews of its windows when the pointer rests on it; a right click opens another window, pins it to the taskbar or closes it; a pinned program starts with a click when it is not running), each Claude Code or Codex session marked with a dot in its mood's colour on the button of the window it runs in; with no session busy, the plugins take turns at the left beside her; at the right the system monitor (when pinned), the tray (folded as Windows' is, some icons kept out, the rest behind ^, dragged from one to the other), the input method (click to switch), Caps Lock, the quick settings (the network and volume icons; click for Windows' own quick settings: Wi-Fi, Bluetooth, airplane mode, volume and more), the clock (click for the notifications and the calendar) and the settings, which open above the taskbar as Windows' own panels do, leaving it uncovered. It steps aside for an app full screen. Switch to another home, or quit, and Windows' taskbar comes back; if she is killed, a guard process gives it back.
- **The island**: if the whole pet is too much, she moves into a small black island at the top centre of the screen, like the iPhone's Dynamic Island. Closed, it has her at the left, a line of words in the middle and the time or a widget's number at the right edge, at a width that stays (narrow, normal or wide, in the settings) whatever it shows. The Monitor is pinned by default: the island grows a little to the right and keeps CPU, memory and the network in sight there while the middle takes its turns; unpin it in its settings and it takes turns like any other widget. Most of the time it shows her round portrait, ringed in the colour of her mood, with the step the session is on (or, once it is done, whose it is) and the time. When something happens (a prompt, a turn done, changes to review, an error) the island opens by itself and the portrait grows into her whole self, standing inside and playing that mood, beside the session's name, where it is and Claude's reply, with a line below for each other session, then shrinks back after a few seconds; a prompt opens it into a panel with her beside it. Hovering opens it too. When other sessions are busy too, a small "+1" sits by the time.
- **Letting her out, and back in**: press her in the island and pull down. The island stretches like a drop of ink with her in it; pull far enough and the drop pinches off and she follows the cursor anywhere, landing where you let go. The island stays while she is out, her seat empty, still showing the status, the time and any prompt (on the desktop she only acts, she does not talk). To bring her home, drag her under the island: it reaches out a drop for her, and letting go close enough draws her back in; or double-click her and she flies back by herself. The right-click menu has Let her out and Call her back in too.
- **Settings grow out of the island**: there is no main window. Click the island and it opens into the settings; only then does it take the keyboard. Hovering, opening by itself or a prompt arriving never take it, so nothing you type in the terminal is lost. Esc, a click outside, ✕ or a click on the head close them, and the keyboard goes back where it was. Five pages: Now (sessions and four quick switches), Pets (yours, downloads, the codex-pets.net gallery), Look and Alerts (the settings), Connect (how Claude Code reaches her, start at login, about); ← → switch pages. A prompt arriving while they are open shows as a banner below the pages, with a dot on Now. With the pet on her own, opening the settings raises an island at the top of the screen for them, gone again once they close.
- Clicks pass through her transparent parts to whatever is underneath.
- **Lost her?** Start the app again and she comes back to the bottom right. She also checks every 2 s and walks back if she ends up off screen.

### Another pet

Paste its URL (like `https://codex-pets.net/#/pets/deepseek-chan`) on the Pets page of the settings, or just its id (the URL's last part); or click Download in the gallery below. Only codex-pets.net URLs are taken, and only that site is downloaded from.

### Off for a while

| You want | Do |
| --- | --- |
| Off for now | Right-click → Quit. The next new Claude Code session brings her back |
| Some quiet | Right-click → Do not disturb: she hides, no notifications, prompts go to the terminal |
| No automatic start, only when I open her | Connect with the plugin (it never starts her) and turn off Start at login; open her yourself |

While she's closed, Claude Code works as usual: the events sent to her are refused at once (about 1 ms) and nothing waits.

### Uninstall

1. If you turned on **Start at login**, turn it off on the Connect page.
2. With the plugin: in Claude Code, `/plugin uninstall wakuwaku@wakuwaku`, then `/plugin marketplace remove wakuwaku`. With hooks in settings.json: click Remove on the Connect page; only this project's entries go, your own hooks stay, and the file as it was before the first change is at `~/.claude/settings.json.wakuwaku.bak`. With Codex: click Remove in its part of the Connect page; the backup is `~/.codex/hooks.json.wakuwaku.bak`.
3. Quit her, then delete the exe or the project folder. Notifications register her name in `HKCU\Software\Classes\AppUserModelId\com.zhoupengjie.wakuwaku`; delete that key too.
4. Delete her settings and downloaded pets: the `wakuwaku-data` folder beside the exe, or the project's `data/` when run from source.

> From source, remove the hooks before deleting the folder: they hold the program's path. If you moved the folder, settings will say the hooks point somewhere else; click Repair.

## How it works

```
Claude Code ──HTTP hooks──▶ the pet (127.0.0.1:47213/hook)
            └─SessionStart─▶ the app itself --wakuwaku-ensure-running (in the background; starts her if she's not up)
Codex ──command hooks──▶ the app itself --wakuwaku-codex-hook ──▶ the pet (/hook?agent=codex)
```

- Every event is a Claude Code **HTTP hook** sent straight to her, so a tool call starts no process (about 0.3 ms here). She answers `{}` at once, meaning no decision; only a prompt waits until you answer on the panel.
- Claude Code runs no HTTP hook for `SessionStart`. The settings.json way gives it a background command instead: the app itself, which passes the event on if she's up (a hello) or starts her if not. No Node needed. The plugin has no such command; she starts at login.
- At rest she takes about 1% of one core (measured on Windows 11) and about 185 MB of private memory, mostly WebView2 (that is, Chromium) itself.

| Variable | Does |
| --- | --- |
| `WAKUWAKU_PORT` | Another port (default 47213); set it for both her and the hooks |
| `CLAUDE_CONFIG_DIR` | Claude Code's config folder, when it isn't `~/.claude` |
| `CODEX_HOME` | Codex's config folder, when it isn't `~/.codex` |

## Tests

```bash
cd src-tauri && cargo test     # Rust: hook mapping, the multi-session state machine, prompt replies and queueing, hooks install, pet URLs...
node --test "test/*.test.js"   # the page: i18n, 16 look directions (no npm install needed)
```

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) (in Chinese) for development notes.

## License and credits

- The code is under the [MIT License](LICENSE).
- The pet sprites come from [codex-pets.net](https://codex-pets.net) and belong to their authors. **They are not in this repository or the exe**; you download them in the settings.
- The default pet, [Claude小姐](https://codex-pets.net/#/pets/claude-chan), is pixel art by zhoupengjie: an unofficial fan work of the Claude小姐 character by Bilibili creator [ZipZipPipe](https://space.bilibili.com/4168597). It too is not in this repository or the exe; the first run downloads it from codex-pets.net.
- The 16 look directions follow codex-pets.net's mapping (clockwise from straight up, one step per 22.5°).

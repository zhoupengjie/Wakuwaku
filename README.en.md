# Wakuwaku

English · [中文](README.md)

> The name is Anya's "waku waku" from SPY×FAMILY: excited, can't wait. Claude works, and she watches, all eager.

A little pet that floats on your desktop and shows what Claude Code is doing: working, waiting for your approval, done with changes for you to review, done, or stuck on an error. When Claude needs your OK, you can answer right on the panel above her head.

She uses the v2 sprite format from the [Codex Pets](https://codex-pets.net) community, so any v2 pet on that site works. The default is [大肥鱼/Deepseek Chan](https://codex-pets.net/#/pets/deepseek-chan).

## What she does

Each v2 pet has 11 animations, and every one of them has a job:

Both v2 pets (11 animations) and older v1 pets (9, without looking around; she just looks ahead when idle) work.

| Animation | When | From which hook |
| --- | --- | --- |
| Idle | Nothing going on (a blink, then a few still seconds) | After you've seen an ending (mouse over her); `SessionEnd` |
| Running | Claude is working; the bubble shows how long this turn has taken | `UserPromptSubmit`, `PreToolUse`, `PostToolUse` |
| Waiting | Waiting on you: a permission prompt, a question, a plan to approve, an MCP form | `PermissionRequest`; `PreToolUse` (AskUserQuestion / ExitPlanMode); `Elicitation` |
| Review | The turn edited files and is done, ready for you to look | `Stop` (after Edit / Write and friends ran this turn) |
| Waving | The turn is done without edits; hello when a session starts or she appears | `Stop`, `SessionStart` |
| Jumping | A task on the to-do list is done; you clicked her | `TaskCompleted` |
| Failed | A request failed (stays until you've seen it); a tool call failed (a quick flash) | `StopFailure`; `PostToolUseFailure` |
| Run right / left | A short stroll when idle; running the way you drag her | — |
| Look around (16 directions) | Her eyes follow the mouse when idle; now and then she glances around | — |

- **Several Claude Code sessions at once**: each keeps its own state. She shows the one that most needs you (waiting > error > review > done > working), names its project, and tells you how many others are busy.
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

## Install

### 1. Get the pet

- **Portable (Windows)**: `Wakuwaku-<version>-portable.exe` runs from any folder and keeps her settings, pets and caches in a `wakuwaku-data` folder beside it, writing nothing else to the system; to remove her, delete the two.

For now you build it yourself: `npm run dist`, see below. There is no installer.
- **From source**: see below.

The main window opens on first launch: pick a pet on the Pets tab (the gallery comes straight from codex-pets.net; click Download), or paste a pet page URL.

### 2. Connect Claude Code (the plugin is recommended)

In Claude Code, enter (the Claude Code tab of the main window has copy buttons):

```
/plugin marketplace add zhoupengjie/wakuwaku
/plugin install wakuwaku@wakuwaku
```

Claude Code installs and removes the plugin itself, and **we don't touch your settings file** (Claude Code notes it in its own `enabledPlugins` and takes it out on uninstall). A plugin can't start the pet, so turn on Start at login in the main window.

If you'd rather not use a plugin, Advanced on the Claude Code tab writes the hooks into `~/.claude/settings.json` (backed up first; removing takes out only our entries). In return, new sessions start the pet. With both on, the main window warns that events arrive twice and offers to remove the old hooks.

### From source

Needs Node.js 18+. Only tested on Windows 11 so far; macOS and Linux should work (on Linux, transparent windows need a compositor; full-screen detection is Windows only).

```bash
git clone https://github.com/zhoupengjie/wakuwaku.git
cd wakuwaku
npm install
npm run fetch-pet        # downloads the default pet, deepseek-chan, from codex-pets.net
npm run install-hooks    # adds the hooks to ~/.claude/settings.json
```

New Claude Code sessions then start her by themselves; `npm start` works too. `npm run dist` builds the portable exe into `dist/`. Run from source, her settings, downloaded pets and caches live in the project's `data/` folder (not in git), never in the system's folders; the first run copies over whatever settings and pets she had in `%APPDATA%\wakuwaku`.

If `node_modules/electron/dist` is empty after `npm install`, run `node node_modules/electron/install.js` once.

## Use

- **Drag** her anywhere; she runs the way you drag, and remembers the spot.
- **Click** her: she jumps.
- **Right-click** her (or the tray icon): switch pets, size, bubble, walking, follow the mouse, island mode, back to the corner, do not disturb, main window, quit. Left-click the tray icon to show or hide her. The tray face changes with her mood: idle, working, waiting, done, review, error.
- **Island mode**: if the whole pet is too much, she moves into a small black island at the top centre of the screen, like the iPhone's Dynamic Island. Most of the time it shows her round portrait, ringed in the colour of her mood, with the project, the status and the time. When something happens (a prompt, a turn done, changes to review, an error) the island opens by itself and the portrait grows into her whole self, standing inside and playing that mood, then shrinks back after a few seconds; a prompt opens it into a panel with her beside it. Hovering opens it too. When other sessions are busy too, a small "+1" sits by the time. Click the island to open the main window.
- **Letting her out, and back in**: press her in the island and pull down. The island stretches like a drop of ink with her in it; pull far enough and the drop pinches off and she follows the cursor anywhere, landing where you let go. The island stays while she is out, her seat empty, still showing the status, the time and any prompt (on the desktop she only acts, she does not talk). To bring her home, drag her under the island: it reaches out a drop for her, and letting go close enough draws her back in. The right-click menu has Let her out and Call her back in too.
- **Main window**: Now lists every session (project, status, time) and any prompt waiting on you, to answer right there; Pets manages and downloads pets, with the codex-pets.net gallery; Look and Alerts hold the settings; Claude Code is how she's connected; About. Your current pet peeks in from the bottom left.
- Clicks pass through her transparent parts to whatever is underneath.
- **Lost her?** Start the app again (`npm start` or the Start menu) and she comes back to the bottom right. She also checks every 2 s and walks back if she ends up off screen.

### Another pet

Paste its URL on the Pets tab of the main window, or:

```bash
npm run fetch-pet -- https://codex-pets.net/#/pets/deepseek-chan
```

Several at once work, separated by spaces, and so does the bare id (the URL's last part). Only codex-pets.net URLs are taken, and only that site is downloaded from. Quote a URL that contains `&`.

### Off for a while

| You want | Do |
| --- | --- |
| Off for now | Right-click → Quit. The next new Claude Code session brings her back |
| Some quiet | Right-click → Do not disturb: she hides, no notifications, prompts go to the terminal |
| No automatic start, only when I open her | In settings, tick "Don't let Claude Code start the pet" and install the hooks again (`npm run install-hooks -- --http-only` from source); open her yourself, or turn on Start at login |

While she's closed, Claude Code works as usual: the events sent to her are refused at once (about 1 ms) and nothing waits.

### Uninstall

1. If you turned on **Start at login**, turn it off in the main window.
2. With the plugin: in Claude Code, `/plugin uninstall wakuwaku@wakuwaku`, then `/plugin marketplace remove wakuwaku`. With hooks in settings.json: click Remove on the Claude Code tab (`npm run uninstall-hooks` from source); only this project's entries go, your own hooks stay, and the file as it was before the first change is at `~/.claude/settings.json.wakuwaku.bak`.
3. Quit her, then delete the portable exe or the project folder.
4. Delete her settings and downloaded pets: the `wakuwaku-data` folder beside the portable exe, or the project's `data/` when run from source. She writes nowhere else.

> From source, remove the hooks before deleting the folder: they hold the program's path. If you moved the folder, settings will say the hooks point somewhere else; click Repair.

## How it works

```
Claude Code ──HTTP hooks──▶ the pet (127.0.0.1:47213/hook)
            └─SessionStart─▶ the app itself --wakuwaku-ensure-running (in the background; starts her if she's not up)
```

- Every event is a Claude Code **HTTP hook** sent straight to her, so a tool call starts no process (about 0.3 ms here). She answers `{}` at once, meaning no decision; only a prompt waits until you answer on the panel.
- Claude Code runs no HTTP hook for `SessionStart`. The settings.json way gives it a background command instead: the app itself, which passes the event on if she's up (a hello) or starts her if not. No Node needed. The plugin has no such command; she starts at login.
- At rest she takes about 1% of one core (measured on Windows 11) and about 180 MB of memory, mostly Electron itself.

| Variable | Does |
| --- | --- |
| `WAKUWAKU_PORT` | Another port (default 47213); set it for both her and the hooks |
| `CLAUDE_CONFIG_DIR` | Claude Code's config folder, when it isn't `~/.claude` |

## Tests

```bash
npm test          # unit tests: hook mapping, the multi-session state machine, prompt replies and queueing, hooks install, i18n, 16 look directions...
npm run smoke     # end to end: a real window on its own port and profile, through every feature, snapshots in out/smoke/
```

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) (in Chinese) for development notes.

## License and credits

- The code is under the [MIT License](LICENSE).
- The pet sprites come from [codex-pets.net](https://codex-pets.net) and belong to their authors. **They are not in this repository or the portable build**; you download them in the settings or with `npm run fetch-pet`.
- The default pet, [大肥鱼/Deepseek Chan](https://codex-pets.net/#/pets/deepseek-chan), is by Dullsaw.
- The 16 look directions follow codex-pets.net's mapping (clockwise from straight up, one step per 22.5°).

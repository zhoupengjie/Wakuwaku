# claude-pets

English · [中文](README.md)

A little pet that floats on your desktop and shows what Claude Code is doing: working, waiting for your approval, done with changes for you to review, done, or stuck on an error. When Claude needs your OK, you can answer right on the panel above her head.

She uses the v2 sprite format from the [Codex Pets](https://codex-pets.net) community, so any v2 pet on that site works. The default is [大肥鱼/Deepseek Chan](https://codex-pets.net/#/pets/deepseek-chan).

## What she does

Each v2 pet has 11 animations, and every one of them has a job:

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

### With the installer (Windows)

Run `Claude Pets Setup <version>.exe` (for now you build it yourself: `npm run dist`, see below). The settings open on first launch:

1. Under Pets, paste a pet page URL from codex-pets.net and click Download.
2. Under Claude Code, click Install to add the hooks.

### From source

Needs Node.js 18+. Only tested on Windows 11 so far; macOS and Linux should work (on Linux, transparent windows need a compositor; full-screen detection is Windows only).

```bash
git clone https://github.com/zhoupengjie/claude-pets.git
cd claude-pets
npm install
npm run fetch-pet        # downloads the default pet, deepseek-chan, from codex-pets.net
npm run install-hooks    # adds the hooks to ~/.claude/settings.json
```

New Claude Code sessions then start her by themselves; `npm start` works too. `npm run dist` builds an installer into `dist/`.

If `node_modules/electron/dist` is empty after `npm install`, run `node node_modules/electron/install.js` once.

## Use

- **Drag** her anywhere; she runs the way you drag, and remembers the spot.
- **Click** her: she jumps.
- **Right-click** her (or the tray icon): switch pets, size, bubble, walking, follow the mouse, back to the corner, do not disturb, settings, quit. Left-click the tray icon to show or hide her.
- **Settings**: download and pick pets, look, alerts (how long endings stay, notifications, sound), how long the prompt panel waits, do not disturb and hiding during full screen, language (中文 / English / system), hooks status and repair, start at login.
- Clicks pass through her transparent parts to whatever is underneath.
- **Lost her?** Start the app again (`npm start` or the Start menu) and she comes back to the bottom right. She also checks every 2 s and walks back if she ends up off screen.

### Another pet

Paste its URL under Pets in the settings, or:

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

1. If you turned on **Start at login**, turn it off in settings.
2. In settings, click Remove next to the hooks (`npm run uninstall-hooks` from source). Only this project's entries go; your own hooks stay. The file as it was before the first change is at `~/.claude/settings.json.claude-pets.bak`.
3. Quit her, then uninstall the app or delete the folder. The installer's uninstaller does steps 1 and 2 for you.
4. (Optional) Delete her settings and downloaded pets: `%APPDATA%\claude-pets` on Windows, `~/Library/Application Support/claude-pets` on macOS, `~/.config/claude-pets` on Linux.

> From source, remove the hooks before deleting the folder: they hold the program's path. If you moved the folder, settings will say the hooks point somewhere else; click Repair.

## How it works

```
Claude Code ──HTTP hooks──▶ the pet (127.0.0.1:47213/hook)
            └─SessionStart─▶ the app itself --claude-pets-ensure-running (in the background; starts her if she's not up)
```

- Every event is a Claude Code **HTTP hook** sent straight to her, so a tool call starts no process (about 0.3 ms here). She answers `{}` at once, meaning no decision; only a prompt waits until you answer on the panel.
- `SessionStart` also runs a background command: the app itself, checking she's up and starting her if not. No Node needed. She runs on her own, so closing a session leaves her be.
- At rest she takes about 1% of one core (measured on Windows 11) and about 180 MB of memory, mostly Electron itself.

| Variable | Does |
| --- | --- |
| `CLAUDE_PETS_PORT` | Another port (default 47213); set it for both her and the hooks |
| `CLAUDE_CONFIG_DIR` | Claude Code's config folder, when it isn't `~/.claude` |

## Tests

```bash
npm test          # unit tests: hook mapping, the multi-session state machine, prompt replies and queueing, hooks install, i18n, 16 look directions...
npm run smoke     # end to end: a real window on its own port and profile, through every feature, snapshots in out/smoke/
```

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) (in Chinese) for development notes.

## License and credits

- The code is under the [MIT License](LICENSE).
- The pet sprites come from [codex-pets.net](https://codex-pets.net) and belong to their authors. **They are not in this repository or the installer**; you download them in the settings or with `npm run fetch-pet`.
- The default pet, [大肥鱼/Deepseek Chan](https://codex-pets.net/#/pets/deepseek-chan), is by Dullsaw.
- The 16 look directions follow codex-pets.net's mapping (clockwise from straight up, one step per 22.5°).

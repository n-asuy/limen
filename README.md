# Limen

A macOS virtual desktop (Space) switcher that gives each Space a **name and identity**.

Press `Option+Space` to summon a ring-shaped overlay at the center of your screen showing every Space with its name, emoji, and running apps. Jump directly with number keys `1`–`9`, navigate with arrow keys and `Enter`, or dismiss with `Esc`. Switch between workspaces instantly without opening Mission Control.

## Features

- **Ring UI** — Up to 9 Spaces arranged in a neumorphic ring. The current Space is highlighted
- **Named Spaces** — Assign custom names and emojis to each Space. Identify by meaning, not number
- **App Icons** — See the top running apps per Space as icons. Know what's there before you switch
- **Recent Trail** — The 3 most recently visited Spaces are visually marked
- **Keyboard-Driven** — `Option+Space` → `1`–`9` / Arrow keys + `Enter` / `Esc`. No mouse needed
- **Tray Integration** — Current Space number shown in the menu bar. Switch from the tray menu too
- **Custom Shortcuts** — Change the global shortcut from the settings window (tray menu → Settings...)
- **Persistent State** — Space names, app data, and icon cache are saved automatically across restarts

## Tech Stack

| Layer | Technology |
|---|---|
| Frontend | React 19 + Vite 7 + TypeScript |
| Backend | Rust + Tauri v2 |
| macOS Integration | Cocoa / Objective-C (CGWindowList, NSWorkspace, Accessibility API, CGEvent) |
| Plugins | tauri-plugin-global-shortcut, tauri-plugin-autostart, tauri-plugin-log |

## Architecture

```
User: Option+Space
  │
  ▼
Global Shortcut (tauri-plugin-global-shortcut)
  │
  ▼
Backend: toggle_main_window()  ──→  Tauri Window (496×496, transparent, always-on-top)
  │                                        │
  │                                        ▼
  │                              Frontend: Ring UI (React)
  │                              - 9 Spaces in a ring layout
  │                              - App icon rendering
  │                              - Keyboard navigation
  │
  ▼
macOS Space Change Listener (NSNotificationCenter)
  │
  ├─→ infer_active_space_index()  ... infer Space from window fingerprint
  ├─→ collect_visible_apps()      ... gather running apps per Space
  └─→ Emit events ──→ Frontend
        - "space-changed"
        - "apps-visible"
        - "frontmost-changed"
```

**Space Detection**: macOS does not expose a public API for the active Space index. Limen builds a mapping by hashing visible windows (PID, name, layer) into a "fingerprint" and associating it with the Space index it was observed on.

**Space Switching**: Injects `Ctrl+[1-9]` keyboard events via `CGEventCreateKeyboardEvent`, triggering the built-in Mission Control hotkeys. No SIP disabling required.

## Setup Requirements

Limen needs two macOS settings to work:

1. **Accessibility permission** — required to inject the switching key events. Limen prompts on first launch; grant it under System Settings → Privacy & Security → Accessibility. If you dismissed the prompt, a failed switch reopens that pane for you.
2. **Mission Control keyboard shortcuts** — Limen switches Spaces by triggering the built-in `Ctrl+1`–`9` shortcuts, which macOS keeps **disabled by default**. Enable "Switch to Desktop 1..9" under System Settings → Keyboard → Keyboard Shortcuts → Mission Control. Settings shows which of the nine are live and opens that pane for you; if none are, Limen opens Settings at launch, since switching cannot work at all.

Limen never sends anything over the network; all data stays on your machine.

## Prerequisites (development)

- [Rust toolchain](https://rustup.rs/)
- [Bun](https://bun.sh/)
- Tauri CLI: `cargo install tauri-cli --version ^2`

## Development

```bash
bun install
bun run dev
```

Vite dev server: `http://localhost:5199`

With performance profiling:

```bash
bun run dev:perf
```

Event logs are written to `log/performance/` in JSON Lines format. Profiling is
opt-in: `bun run dev:perf` is the only command that writes to `log/`, and both
`bun run dev` and release builds record nothing.

## Build

```bash
bun run build
```

### Packaging (macOS)

x86_64:

```bash
rustup target add x86_64-apple-darwin   # first time only
bun run package:mac:x86
```

Universal (arm64 + x86_64):

```bash
bun run package:mac:universal
```

Output: `src-tauri/target/<triple>/release/bundle/`

### GitHub Release Build

Pushing a `v*` tag runs `.github/workflows/release.yml` and uploads a universal macOS bundle to GitHub Releases.

Release notes come from the **annotated tag message**, so tag with the matching [`CHANGELOG.md`](CHANGELOG.md) entry as the annotation:

```bash
git tag -a v0.1.0 -m "$(sed -n '/^## 0.1.0$/,/^## /p' CHANGELOG.md | sed '$d')"
git push origin v0.1.0
```

The workflow validates that the git tag matches `src-tauri/tauri.conf.json`'s version, fails if the tag has no annotation, requires the Apple Developer ID signing secrets (`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`) to be configured, and publishes signed and notarized artifacts.

## Project Structure

```
src/                           # Frontend (React + TypeScript)
├── app.tsx                    # Main UI — ring layout, keyboard handling, inline editing
├── app.css                    # Neumorphic styling, light/dark mode
├── hooks/
│   └── use-space-store.ts     # Space state management (names, apps, icons) + persistence
├── lib/
│   └── space-utils.ts         # Pure utility functions (topApps extraction, etc.)
└── perf/
    └── recorder.ts            # Frontend performance instrumentation

src-tauri/                     # Backend (Rust + Tauri v2)
├── src/
│   ├── lib.rs                 # Tauri setup, IPC commands, window management
│   ├── macos.rs               # macOS-specific — Space detection, key injection, app enumeration
│   ├── config.rs              # Shortcut preference read/write
│   ├── tray.rs                # System tray menu construction
│   └── perf.rs                # Backend profiling (feature-gated)
└── tauri.conf.json            # Tauri app configuration
```

## Data Persistence

| Data | Location |
|---|---|
| Space names & app info | `~/Library/Application Support/Limen/space.json` |
| App icon cache | `~/Library/Application Support/Limen/icons/` |
| Shortcut preferences | `~/Library/Application Support/Limen/config.json` |

## License

[MIT](LICENSE) © Curino

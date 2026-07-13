# Changelog

## 0.1.1

- Added in-app updates: **Settings → Software update** checks GitHub Releases for a newer signed build and installs it in place, verifying the update signature before applying. Nothing is contacted over the network unless you press "Check for updates"
- Fixed the tray menu not refreshing: Space names and the current-Space check mark now update after a rename or a Space switch, instead of only at launch
- Documented the limits of Space detection (single display recommended, up to 9 Spaces, empty Spaces are ambiguous) in the README

## 0.1.0

- Initial release of Limen, a macOS virtual desktop (Space) switcher that gives each Space a name and identity
- Added a ring-shaped overlay summoned with a global shortcut, showing up to 9 Spaces with their names, emojis, and top running apps, with the current Space highlighted and the 3 most recently visited Spaces marked
- Added keyboard-driven navigation: jump directly with number keys `1`-`9`, move with arrow keys and `Enter`, and dismiss with `Esc`
- Added Space switching by injecting the built-in Mission Control `Ctrl+1`-`9` hotkeys, so no SIP disabling is required
- Added detection of the "Switch to Desktop 1..9" shortcuts that Space switching depends on: Settings reports which Spaces are reachable and opens the Keyboard pane, and Limen opens Settings at launch when none of them are enabled
- Added active-Space detection by fingerprinting visible windows, since macOS exposes no public API for the current Space index
- Added a menu bar tray showing the current Space number, with switching and a Settings entry available from the tray menu
- Added a customizable global shortcut editable from the settings window, defaulting to `Option+Space`
- Added persistent storage of Space names, per-app data, and the icon cache under `~/Library/Application Support/Limen/`
- Added a signed and notarized universal macOS release build published on tagged releases

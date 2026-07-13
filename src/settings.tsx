import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import ShortcutRecorder from "./components/shortcut-recorder";
import { humanizeShortcutError } from "./lib/shortcut";
import { describeSpaceShortcuts, summarizeSpaceShortcuts } from "./lib/space-shortcuts";
import "./settings.css";

type ShortcutPreferences = {
  primary: string;
  fallback: string[];
};

type AvailableUpdate = {
  version: string;
  current_version: string;
  notes: string | null;
};

// idle: not checked yet. checking/installing: request in flight.
// uptodate/available/error: terminal states of the last check.
type UpdatePhase = "idle" | "checking" | "uptodate" | "available" | "installing" | "error";

export default function Settings() {
  const [shortcut, setShortcut] = useState<string>("");
  const [shortcutError, setShortcutError] = useState<string | null>(null);
  const [accessibilityGranted, setAccessibilityGranted] = useState<boolean | null>(null);
  const [spaceShortcuts, setSpaceShortcuts] = useState<number[] | null>(null);
  const [autostartEnabled, setAutostartEnabled] = useState<boolean | null>(null);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [updatePhase, setUpdatePhase] = useState<UpdatePhase>("idle");
  const [update, setUpdate] = useState<AvailableUpdate | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);

  useEffect(() => {
    invoke<ShortcutPreferences>("get_shortcut_config")
      .then((prefs) => setShortcut(prefs.primary))
      .catch(() => setShortcut(""));
    isEnabled()
      .then(setAutostartEnabled)
      .catch(() => setAutostartEnabled(null));
    getVersion()
      .then(setAppVersion)
      .catch(() => setAppVersion(null));
  }, []);

  const refreshAccessibility = useCallback(() => {
    invoke<boolean>("check_accessibility")
      .then(setAccessibilityGranted)
      .catch(() => setAccessibilityGranted(null));
  }, []);

  const refreshSpaceShortcuts = useCallback(() => {
    invoke<number[]>("check_space_shortcuts")
      .then(setSpaceShortcuts)
      .catch(() => setSpaceShortcuts(null));
  }, []);

  const shortcutSummary = spaceShortcuts ? summarizeSpaceShortcuts(spaceShortcuts) : null;
  const shortcutsReady = shortcutSummary?.status === "ready";
  const shortcutDescription = describeSpaceShortcuts(shortcutSummary);

  // Both are changed in System Settings, outside this window. Poll while
  // either is incomplete so the status flips without a manual refresh.
  useEffect(() => {
    refreshAccessibility();
    refreshSpaceShortcuts();
    if (accessibilityGranted && shortcutsReady) return;
    const timer = window.setInterval(() => {
      refreshAccessibility();
      refreshSpaceShortcuts();
    }, 2000);
    return () => window.clearInterval(timer);
  }, [accessibilityGranted, shortcutsReady, refreshAccessibility, refreshSpaceShortcuts]);

  const changeShortcut = useCallback((accelerator: string) => {
    setShortcutError(null);
    invoke<ShortcutPreferences>("update_shortcut_config", { primary: accelerator })
      .then((prefs) => setShortcut(prefs.primary))
      .catch((err) => setShortcutError(humanizeShortcutError(String(err))));
  }, []);

  const resetShortcut = useCallback(() => {
    setShortcutError(null);
    invoke<ShortcutPreferences>("reset_shortcut_config")
      .then((prefs) => setShortcut(prefs.primary))
      .catch((err) => setShortcutError(humanizeShortcutError(String(err))));
  }, []);

  const openSwitcher = useCallback(() => {
    invoke("toggle_window").catch(() => undefined);
  }, []);

  const requestAccessibility = useCallback(() => {
    invoke<boolean>("request_accessibility")
      .then(setAccessibilityGranted)
      .catch(() => undefined);
  }, []);

  const openShortcutSettings = useCallback(() => {
    invoke("open_space_shortcut_settings").catch(() => undefined);
  }, []);

  const checkForUpdate = useCallback(() => {
    setUpdateError(null);
    setUpdatePhase("checking");
    invoke<AvailableUpdate | null>("check_for_update")
      .then((result) => {
        if (result) {
          setUpdate(result);
          setUpdatePhase("available");
        } else {
          setUpdate(null);
          setUpdatePhase("uptodate");
        }
      })
      .catch((err) => {
        setUpdateError(String(err));
        setUpdatePhase("error");
      });
  }, []);

  const installUpdate = useCallback(() => {
    setUpdateError(null);
    setUpdatePhase("installing");
    // On success the app relaunches, so this promise never resolves.
    invoke("install_update").catch((err) => {
      setUpdateError(String(err));
      setUpdatePhase("error");
    });
  }, []);

  const toggleAutostart = useCallback(() => {
    const next = !autostartEnabled;
    const action = next ? enable() : disable();
    action
      .then(() => isEnabled())
      .then(setAutostartEnabled)
      .catch(() => undefined);
  }, [autostartEnabled]);

  return (
    <div className="settings-surface">
      {/* Clears the native traffic lights; drags the window. */}
      <header className="settings__titlebar" data-tauri-drag-region />

      <div className="settings">
      <section className="settings__section">
        <h2 className="settings__heading">Global shortcut</h2>
        <p className="settings__description">Opens the Space switcher from anywhere.</p>
        <ShortcutRecorder
          value={shortcut}
          onChange={changeShortcut}
          onReset={resetShortcut}
          error={shortcutError}
          hint="Click, then press the new combination"
        />
        <button type="button" className="settings__primary" onClick={openSwitcher}>
          Show me
        </button>
      </section>

      <section className="settings__section">
        <h2 className="settings__heading">Permissions</h2>
        <div className="settings__row">
          <div>
            <div className="settings__row-title">Accessibility</div>
            <p className="settings__description">
              Required to switch Spaces. If switching stops working after granting, restart
              Limen.
            </p>
          </div>
          {accessibilityGranted ? (
            <span className="settings__badge settings__badge--ok">Granted</span>
          ) : (
            <button type="button" className="settings__action" onClick={requestAccessibility}>
              Grant...
            </button>
          )}
        </div>
        <div className="settings__row">
          <div>
            <div className="settings__row-title">Mission Control shortcuts</div>
            <p className="settings__description">{shortcutDescription}</p>
          </div>
          {shortcutsReady ? (
            <span className="settings__badge settings__badge--ok">Enabled</span>
          ) : (
            <button type="button" className="settings__action" onClick={openShortcutSettings}>
              Enable...
            </button>
          )}
        </div>
      </section>

      <section className="settings__section">
        <h2 className="settings__heading">General</h2>
        <div className="settings__row">
          <div>
            <div className="settings__row-title">Launch at login</div>
            <p className="settings__description">Start Limen automatically when you log in.</p>
          </div>
          <button
            type="button"
            role="switch"
            aria-checked={autostartEnabled ?? false}
            className={`settings__toggle ${autostartEnabled ? "is-on" : ""}`}
            onClick={toggleAutostart}
            disabled={autostartEnabled === null}
          >
            <span className="settings__toggle-knob" />
          </button>
        </div>
      </section>

      <section className="settings__section">
        <h2 className="settings__heading">Software update</h2>
        <div className="settings__row">
          <div>
            <div className="settings__row-title">
              {update ? `Version ${update.version} available` : "Limen is up to date"}
            </div>
            <p className="settings__description">
              {updatePhase === "checking"
                ? "Checking for updates..."
                : updatePhase === "installing"
                  ? "Downloading and installing. Limen will restart."
                  : updatePhase === "error"
                    ? (updateError ?? "Could not check for updates.")
                    : update
                      ? `You have ${update.current_version}. Install to update and restart.`
                      : appVersion
                        ? `Current version ${appVersion}.`
                        : "Check for a newer signed release."}
            </p>
          </div>
          {update ? (
            <button
              type="button"
              className="settings__primary"
              onClick={installUpdate}
              disabled={updatePhase === "installing"}
            >
              Install & restart
            </button>
          ) : (
            <button
              type="button"
              className="settings__action"
              onClick={checkForUpdate}
              disabled={updatePhase === "checking"}
            >
              Check for updates
            </button>
          )}
        </div>
      </section>
      </div>
    </div>
  );
}

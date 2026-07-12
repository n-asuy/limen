import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { formatAccelerator, parseKeyboardEventToAccelerator } from "../lib/shortcut";
import "./shortcut-recorder.css";

type ShortcutRecorderProps = {
  value: string;
  onChange: (accelerator: string) => void;
  onReset: () => void;
  disabled?: boolean;
  error?: string | null;
  hint?: string;
};

export function ShortcutRecorder(props: ShortcutRecorderProps) {
  const { value, onChange, onReset, disabled = false, error: externalError, hint } = props;
  const [listening, setListening] = useState(false);
  const [captureError, setCaptureError] = useState<string | null>(null);
  const [pendingDisplay, setPendingDisplay] = useState<string | null>(null);
  const buttonRef = useRef<HTMLButtonElement | null>(null);
  const shortcutsSuspendedRef = useRef(false);

  const displayValue = useMemo(() => {
    if (!value) return "Not set";
    return formatAccelerator(value);
  }, [value]);

  const error = captureError ?? externalError ?? null;

  useEffect(() => {
    if (!listening) return;

    const handler = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();

      if (
        event.key === "Escape" &&
        !event.metaKey &&
        !event.ctrlKey &&
        !event.altKey &&
        !event.shiftKey
      ) {
        setListening(false);
        setCaptureError(null);
        setPendingDisplay(null);
        return;
      }

      const result = parseKeyboardEventToAccelerator(event, { requireModifier: true });
      if (result.status === "pending") {
        setCaptureError(null);
        setPendingDisplay(result.display);
        return;
      }
      if (result.status === "invalid") {
        setCaptureError(result.reason);
        setPendingDisplay(null);
        return;
      }

      setListening(false);
      setCaptureError(null);
      setPendingDisplay(null);
      onChange(result.accelerator);
    };

    const releaseHandler = (event: KeyboardEvent) => {
      if (!event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey) {
        setPendingDisplay(null);
      }
    };

    window.addEventListener("keydown", handler, { capture: true });
    window.addEventListener("keyup", releaseHandler, { capture: true });
    return () => {
      window.removeEventListener("keydown", handler, { capture: true });
      window.removeEventListener("keyup", releaseHandler, { capture: true });
    };
  }, [listening, onChange]);

  // The global shortcut must not fire while the user is typing a new one.
  useEffect(() => {
    if (listening) {
      if (shortcutsSuspendedRef.current) return;
      shortcutsSuspendedRef.current = true;
      invoke("suspend_shortcuts").catch(() => {
        shortcutsSuspendedRef.current = false;
      });
      return;
    }

    if (shortcutsSuspendedRef.current) {
      shortcutsSuspendedRef.current = false;
      invoke("resume_shortcuts").catch(() => undefined);
    }
  }, [listening]);

  useEffect(() => {
    return () => {
      if (shortcutsSuspendedRef.current) {
        shortcutsSuspendedRef.current = false;
        invoke("resume_shortcuts").catch(() => undefined);
      }
    };
  }, []);

  useEffect(() => {
    if (disabled && listening) {
      setListening(false);
    }
  }, [disabled, listening]);

  const startListening = () => {
    if (disabled) return;
    setCaptureError(null);
    setPendingDisplay(null);
    setListening(true);
    window.setTimeout(() => {
      buttonRef.current?.focus();
    }, 0);
  };

  const handleReset = () => {
    setCaptureError(null);
    setPendingDisplay(null);
    setListening(false);
    onReset();
  };

  return (
    <div className={`shortcut-recorder ${disabled ? "is-disabled" : ""}`}>
      <div className="shortcut-recorder__row">
        <button
          ref={buttonRef}
          type="button"
          className={`shortcut-recorder__button ${listening ? "is-listening" : ""}`}
          onClick={startListening}
          disabled={disabled}
        >
          <span>
            {listening ? (pendingDisplay ? `${pendingDisplay} …` : "Press keys...") : displayValue}
          </span>
        </button>
        <button
          type="button"
          className="shortcut-recorder__reset"
          onClick={handleReset}
          disabled={disabled}
        >
          Reset to default
        </button>
      </div>
      <div className="shortcut-recorder__meta">
        {listening ? (
          <span className="shortcut-recorder__hint">Esc to cancel</span>
        ) : hint ? (
          <span>{hint}</span>
        ) : null}
        {error ? <span className="shortcut-recorder__error">{error}</span> : null}
      </div>
    </div>
  );
}

export default ShortcutRecorder;

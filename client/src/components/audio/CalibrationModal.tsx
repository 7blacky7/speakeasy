import { createSignal, Show } from "solid-js";
import { CalibrationResult, startCalibration } from "../../bridge";
import styles from "./CalibrationModal.module.css";

interface CalibrationModalProps {
  onClose: () => void;
  onApply: (result: CalibrationResult) => void;
}

type Phase = "idle" | "measuring" | "done" | "error";

export default function CalibrationModal(props: CalibrationModalProps) {
  const [phase, setPhase] = createSignal<Phase>("idle");
  const [result, setResult] = createSignal<CalibrationResult | null>(null);
  const [errorMsg, setErrorMsg] = createSignal<string | null>(null);

  async function startMeasurement() {
    setPhase("measuring");
    setErrorMsg(null);
    try {
      const r = await startCalibration();
      setResult(r);
      setPhase("done");
    } catch (e) {
      setErrorMsg(String(e));
      setPhase("error");
    }
  }

  function handleApply() {
    const r = result();
    if (r) props.onApply(r);
    props.onClose();
  }

  return (
    <div class={styles.overlay} role="dialog" aria-modal="true" aria-label="Automatisches Audio-Setup">
      <div class={styles.modal}>
        <div class={styles.header}>
          <h2 class={styles.title}>Automatisches Einrichten</h2>
          <button class={styles.closeBtn} onClick={props.onClose} aria-label="Schliessen">
            x
          </button>
        </div>

        <div class={styles.body}>
          <Show when={phase() === "idle"}>
            <p class={styles.text}>
              Klicken Sie auf "Starten", um Ihre Mikrofon-Umgebung zu kalibrieren.
              Bitte schweigen Sie wahrend der Messung fur 3 Sekunden.
            </p>
            <button class={styles.startBtn} onClick={startMeasurement}>
              Messung starten
            </button>
          </Show>

          <Show when={phase() === "measuring"}>
            <p class={styles.text}>Bitte schweigen Sie - Messung laeuft...</p>
            <div class={styles.progressBar}>
              <div class={styles.progressFill} style={{ width: "100%", "animation": "progress 3s linear" }} />
            </div>
            <p class={styles.progressLabel}>Messung laeuft...</p>
          </Show>

          <Show when={phase() === "error"}>
            <p class={`${styles.text} ${styles.errorText}`}>
              Kalibrierung fehlgeschlagen: {errorMsg()}
            </p>
            <button class={styles.startBtn} onClick={() => setPhase("idle")}>
              Erneut versuchen
            </button>
          </Show>

          <Show when={phase() === "done" && result()}>
            {(r) => (
              <>
                <p class={`${styles.text} ${styles.success}`}>
                  Kalibrierung abgeschlossen!
                </p>
                <div class={styles.results}>
                  <div class={styles.resultRow}>
                    <span class={styles.resultLabel}>Gerauschpegel</span>
                    <span class={styles.resultValue}>{r().noiseFloor.toFixed(1)} dBFS</span>
                  </div>
                  <div class={styles.resultRow}>
                    <span class={styles.resultLabel}>Empfohlene Eingangslautstaerke</span>
                    <span class={styles.resultValue}>{Math.round(r().suggestedInputVolume * 100)}%</span>
                  </div>
                  <div class={styles.resultRow}>
                    <span class={styles.resultLabel}>Empfohlene VAD-Empfindlichkeit</span>
                    <span class={styles.resultValue}>
                      {Math.round(r().suggestedVadSensitivity * 100)}%
                    </span>
                  </div>
                </div>
              </>
            )}
          </Show>
        </div>

        <div class={styles.footer}>
          <button class={styles.cancelBtn} onClick={props.onClose}>
            Abbrechen
          </button>
          <Show when={phase() === "done"}>
            <button class={styles.applyBtn} onClick={handleApply}>
              Ubernehmen
            </button>
          </Show>
        </div>
      </div>
    </div>
  );
}

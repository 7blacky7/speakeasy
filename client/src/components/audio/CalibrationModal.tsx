import { createSignal, Show } from "solid-js";
import { CalibrationResult } from "../../bridge";
import styles from "./CalibrationModal.module.css";

interface CalibrationModalProps {
  onClose: () => void;
  onApply: (result: CalibrationResult) => void;
}

type Phase = "idle" | "measuring" | "done";

export default function CalibrationModal(props: CalibrationModalProps) {
  const [phase, setPhase] = createSignal<Phase>("idle");
  const [progress, setProgress] = createSignal(0);
  const [result, setResult] = createSignal<CalibrationResult | null>(null);

  function startMeasurement() {
    setPhase("measuring");
    setProgress(0);

    const total = 3000;
    const interval = 50;
    let elapsed = 0;

    const timer = setInterval(() => {
      elapsed += interval;
      setProgress(Math.min(100, (elapsed / total) * 100));

      if (elapsed >= total) {
        clearInterval(timer);
        const mockResult: CalibrationResult = {
          success: true,
          suggestedVadSensitivity: 0.65,
          suggestedInputVolume: 85,
          noiseFloor: -48,
        };
        setResult(mockResult);
        setPhase("done");
      }
    }, interval);
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
            <p class={styles.text}>Bitte schweigen Sie fur 3 Sekunden...</p>
            <div class={styles.progressBar}>
              <div
                class={styles.progressFill}
                style={{ width: `${progress()}%` }}
              />
            </div>
            <p class={styles.progressLabel}>{Math.round(progress())}%</p>
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
                    <span class={styles.resultValue}>{r().noiseFloor} dBFS</span>
                  </div>
                  <div class={styles.resultRow}>
                    <span class={styles.resultLabel}>Empfohlene Eingangslauststarke</span>
                    <span class={styles.resultValue}>{r().suggestedInputVolume}%</span>
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

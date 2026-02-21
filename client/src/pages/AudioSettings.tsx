import { A } from "@solidjs/router";
import {
  createSignal,
  createEffect,
  onCleanup,
  Show,
  batch,
} from "solid-js";
import { createStore, produce } from "solid-js/store";

import {
  AudioDevice,
  AudioSettingsConfig,
  AudioStats,
  CalibrationResult,
} from "../bridge";

import ModeSwitch from "../components/audio/ModeSwitch";
import DeviceSelector from "../components/audio/DeviceSelector";
import VoiceModeSelector from "../components/audio/VoiceModeSelector";
import PresetGrid from "../components/audio/PresetGrid";
import AudioSlider from "../components/audio/AudioSlider";
import DspModule from "../components/audio/DspModule";
import LatencyDisplay from "../components/audio/LatencyDisplay";
import LatencyBreakdown from "../components/audio/LatencyBreakdown";
import LiveMonitor from "../components/audio/LiveMonitor";
import CalibrationModal from "../components/audio/CalibrationModal";
import CodecSettings from "../components/audio/CodecSettings";
import JitterSettings from "../components/audio/JitterSettings";

import styles from "./AudioSettings.module.css";

// ---- Mock-Geräteliste ----
const MOCK_DEVICES: AudioDevice[] = [
  { id: "mic-1", name: "Standardmikrofon (Realtek)", kind: "input", is_default: true },
  { id: "mic-2", name: "USB-Headset (Logitech G Pro)", kind: "input", is_default: false },
  { id: "mic-3", name: "Eingebautes Mikrofon", kind: "input", is_default: false },
  { id: "out-1", name: "Standardlautsprecher (Realtek)", kind: "output", is_default: true },
  { id: "out-2", name: "USB-Headset (Logitech G Pro)", kind: "output", is_default: false },
  { id: "out-3", name: "HDMI Audio (Monitor)", kind: "output", is_default: false },
];

// ---- Standardwerte ----
const DEFAULT_SETTINGS: AudioSettingsConfig = {
  inputDeviceId: null,
  outputDeviceId: null,
  voiceMode: "vad",
  pttKey: null,
  vadSensitivity: 0.6,
  preset: "speech",
  noiseSuppression: "medium",
  inputVolume: 100,
  outputVolume: 100,
  codec: {
    sampleRate: 48000,
    bufferSize: 256,
    bitrate: 64,
    frameSize: 20,
    application: "voip",
    fec: true,
    dtx: true,
    channels: "mono",
  },
  dsp: {
    noiseGate: { enabled: false, threshold: -40, attack: 5, release: 50 },
    noiseSuppression: { enabled: true, level: "medium" },
    agc: { enabled: false, targetLevel: -18, maxGain: 30, attack: 10, release: 100 },
    echoCancellation: { enabled: true, tailLength: 100 },
    deesser: { enabled: false, frequency: 7000, threshold: -20, ratio: 4 },
  },
  jitter: { minBuffer: 20, maxBuffer: 100, adaptive: true },
};

const DEFAULT_STATS: AudioStats = {
  inputLevel: 0,
  outputLevel: 0,
  processedLevel: 0,
  noiseFloor: -60,
  isClipping: false,
  latency: { device: 12, encoding: 5, jitter: 20, network: 18, total: 55 },
  packetLoss: 0,
  rtt: 36,
  bitrate: 64,
};

const NOISE_LEVELS = ["off", "low", "medium", "high"] as const;

export default function AudioSettings() {
  const [mode, setMode] = createSignal<"simple" | "expert">("simple");
  const [settings, setSettings] = createStore<AudioSettingsConfig>(DEFAULT_SETTINGS);
  const [stats, setStats] = createSignal<AudioStats>(DEFAULT_STATS);
  const [showCalibration, setShowCalibration] = createSignal(false);
  const [noiseLevelIndex, setNoiseLevelIndex] = createSignal(2); // "medium"

  // Simulierte Pegel-Animation
  let animFrame: number;
  let lastTime = 0;

  function animateStats(ts: number) {
    if (ts - lastTime > 50) {
      lastTime = ts;
      setStats((prev) => {
        const base = 0.35 + Math.random() * 0.3;
        const clip = base > 0.92;
        return {
          ...prev,
          inputLevel: Math.max(0, base + (Math.random() - 0.5) * 0.1),
          processedLevel: Math.max(0, base * 0.85 + (Math.random() - 0.5) * 0.08),
          isClipping: clip,
          noiseFloor: -55 + Math.random() * 3,
          latency: {
            ...prev.latency,
            network: 14 + Math.round(Math.random() * 10),
            total: prev.latency.device + prev.latency.encoding + prev.latency.jitter + (14 + Math.round(Math.random() * 10)),
          },
          rtt: 30 + Math.round(Math.random() * 20),
          packetLoss: Math.random() * 0.5,
        };
      });
    }
    animFrame = requestAnimationFrame(animateStats);
  }

  createEffect(() => {
    animFrame = requestAnimationFrame(animateStats);
    onCleanup(() => cancelAnimationFrame(animFrame));
  });

  // Einstellungs-Helpers
  function updateCodec<K extends keyof AudioSettingsConfig["codec"]>(
    key: K,
    value: AudioSettingsConfig["codec"][K]
  ) {
    setSettings("codec", key, value);
    setSettings("preset", "custom");
  }

  function updateDsp<M extends keyof AudioSettingsConfig["dsp"]>(
    module: M,
    patch: Partial<AudioSettingsConfig["dsp"][M]>
  ) {
    setSettings(
      produce((s) => {
        Object.assign(s.dsp[module] as object, patch);
      })
    );
  }

  function updateJitter<K extends keyof AudioSettingsConfig["jitter"]>(
    key: K,
    value: AudioSettingsConfig["jitter"][K]
  ) {
    setSettings("jitter", key, value);
  }

  function selectPreset(id: "speech" | "balanced" | "music" | "low_bandwidth") {
    const map: Record<string, Partial<AudioSettingsConfig["codec"]>> = {
      speech: { bitrate: 32, sampleRate: 16000, channels: "mono", fec: true, dtx: true },
      balanced: { bitrate: 64, sampleRate: 48000, channels: "mono", fec: false, dtx: false },
      music: { bitrate: 192, sampleRate: 48000, channels: "stereo", fec: false, dtx: false },
      low_bandwidth: { bitrate: 12, sampleRate: 8000, channels: "mono", fec: true, dtx: true },
    };
    setSettings("preset", id);
    setSettings(produce((s) => { Object.assign(s.codec, map[id]); }));
  }

  function handleCalibrationApply(result: CalibrationResult) {
    batch(() => {
      setSettings("vadSensitivity", result.suggestedVadSensitivity);
      setSettings("inputVolume", result.suggestedInputVolume);
    });
  }

  function resetToDefaults() {
    setSettings(produce((s) => { Object.assign(s, DEFAULT_SETTINGS); }));
    setNoiseLevelIndex(2);
  }

  const noiseLabel = () =>
    ["Aus", "Niedrig", "Mittel", "Hoch"][noiseLevelIndex()];

  return (
    <div class={styles.page}>
      {/* Breadcrumb */}
      <div class={styles.breadcrumb}>
        <A href="/settings" class={styles.breadcrumbLink}>Einstellungen</A>
        <span class={styles.breadcrumbSep}>›</span>
        <span>Audio</span>
      </div>

      {/* Titel + Mode-Switcher */}
      <div class={styles.titleRow}>
        <h1 class={styles.title}>Audio-Einstellungen</h1>
        <ModeSwitch mode={mode()} onChange={setMode} />
      </div>

      <div class={styles.content}>
        {/* ---- Gerateauswahl ---- */}
        <section class={styles.section}>
          <h2 class={styles.sectionTitle}>Gerate</h2>
          <div class={styles.sectionBody}>
            <DeviceSelector
              label="Mikrofon"
              kind="input"
              devices={MOCK_DEVICES}
              selectedId={settings.inputDeviceId}
              onChange={(id) => setSettings("inputDeviceId", id)}
              onTest={() => console.log("Mikrofon-Test")}
            />
            <DeviceSelector
              label="Ausgabe"
              kind="output"
              devices={MOCK_DEVICES}
              selectedId={settings.outputDeviceId}
              onChange={(id) => setSettings("outputDeviceId", id)}
              onTest={() => console.log("Ausgabe-Test")}
            />
          </div>
        </section>

        {/* ---- Sprach-Modus ---- */}
        <section class={styles.section}>
          <h2 class={styles.sectionTitle}>Sprach-Modus</h2>
          <div class={styles.sectionBody}>
            <VoiceModeSelector
              mode={settings.voiceMode}
              pttKey={settings.pttKey}
              vadSensitivity={settings.vadSensitivity}
              onModeChange={(m) => setSettings("voiceMode", m)}
              onPttKeyChange={() => console.log("PTT-Taste andern")}
              onVadSensitivityChange={(v) => setSettings("vadSensitivity", v)}
            />
          </div>
        </section>

        {/* ---- Presets ---- */}
        <section class={styles.section}>
          <h2 class={styles.sectionTitle}>Klangprofile</h2>
          <div class={styles.sectionBody}>
            <PresetGrid
              activePreset={settings.preset}
              onSelect={selectPreset}
            />
          </div>
        </section>

        {/* ---- Rauschunterdrueckung ---- */}
        <section class={styles.section}>
          <h2 class={styles.sectionTitle}>Rauschunterdrueckung</h2>
          <div class={styles.sectionBody}>
            <div class={styles.noiseRow}>
              <input
                type="range"
                class={styles.noiseSlider}
                min={0}
                max={3}
                step={1}
                value={noiseLevelIndex()}
                onInput={(e) => {
                  const idx = parseInt(e.currentTarget.value);
                  setNoiseLevelIndex(idx);
                  setSettings("noiseSuppression", NOISE_LEVELS[idx]);
                }}
                aria-label="Rauschunterdrueckung"
                aria-valuetext={noiseLabel()}
              />
              <div class={styles.noiseTicks}>
                {["Aus", "Niedrig", "Mittel", "Hoch"].map((l) => (
                  <span class={`${styles.noiseTick} ${noiseLabel() === l ? styles.noiseTickActive : ""}`}>{l}</span>
                ))}
              </div>
            </div>
          </div>
        </section>

        {/* ---- Lautstärke ---- */}
        <section class={styles.section}>
          <h2 class={styles.sectionTitle}>Lautstarke</h2>
          <div class={styles.sectionBody}>
            <AudioSlider
              label="Mikrofonlautstarke"
              value={settings.inputVolume}
              min={0}
              max={200}
              step={1}
              unit="%"
              onChange={(v) => setSettings("inputVolume", v)}
            />
            <AudioSlider
              label="Ausgabelautstarke"
              value={settings.outputVolume}
              min={0}
              max={200}
              step={1}
              unit="%"
              onChange={(v) => setSettings("outputVolume", v)}
            />
            <button
              class={styles.testSoundBtn}
              onClick={() => console.log("Testsignal abspielen")}
            >
              Testsignal abspielen
            </button>
          </div>
        </section>

        {/* ---- Auto-Setup ---- */}
        <section class={styles.section}>
          <h2 class={styles.sectionTitle}>Automatisches Einrichten</h2>
          <div class={styles.sectionBody}>
            <div class={styles.autoSetupRow}>
              <p class={styles.autoSetupDesc}>
                Kalibriert Mikrofon-Empfindlichkeit und Rauschpegel automatisch.
              </p>
              <button
                class={styles.autoSetupBtn}
                onClick={() => setShowCalibration(true)}
              >
                Automatisch einrichten
              </button>
            </div>
            <LatencyDisplay latencyMs={stats().latency.total} />
          </div>
        </section>

        {/* ---- EXPERT MODE ---- */}
        <Show when={mode() === "expert"}>
          {/* Codec */}
          <section class={styles.section}>
            <h2 class={styles.sectionTitle}>Codec-Einstellungen</h2>
            <div class={styles.sectionBody}>
              <CodecSettings
                config={settings.codec}
                onChange={updateCodec}
              />
            </div>
          </section>

          {/* DSP-Module */}
          <section class={styles.section}>
            <h2 class={styles.sectionTitle}>DSP-Verarbeitung</h2>
            <div class={styles.sectionBody}>
              <DspModule
                label="Noise Gate"
                enabled={settings.dsp.noiseGate.enabled}
                onToggle={(v) => updateDsp("noiseGate", { enabled: v })}
                tooltip="Blockiert leise Signale unterhalb einer Schwelle"
              >
                <AudioSlider
                  label="Schwelle"
                  value={settings.dsp.noiseGate.threshold}
                  min={-60}
                  max={0}
                  step={1}
                  unit=" dB"
                  onChange={(v) => updateDsp("noiseGate", { threshold: v })}
                />
                <AudioSlider
                  label="Attack"
                  value={settings.dsp.noiseGate.attack}
                  min={1}
                  max={200}
                  step={1}
                  unit=" ms"
                  onChange={(v) => updateDsp("noiseGate", { attack: v })}
                />
                <AudioSlider
                  label="Release"
                  value={settings.dsp.noiseGate.release}
                  min={10}
                  max={1000}
                  step={10}
                  unit=" ms"
                  onChange={(v) => updateDsp("noiseGate", { release: v })}
                />
              </DspModule>

              <DspModule
                label="Rauschunterdrueckung"
                enabled={settings.dsp.noiseSuppression.enabled}
                onToggle={(v) => updateDsp("noiseSuppression", { enabled: v })}
                tooltip="Algorithmische Filterung von Umgebungsgerauschen"
              >
                <div class={styles.radioInline}>
                  {(["low", "medium", "high"] as const).map((lvl) => (
                    <label class={`${styles.inlineRadio} ${settings.dsp.noiseSuppression.level === lvl ? styles.inlineRadioActive : ""}`}>
                      <input
                        type="radio"
                        name="dsp-noise"
                        checked={settings.dsp.noiseSuppression.level === lvl}
                        onChange={() => updateDsp("noiseSuppression", { level: lvl })}
                      />
                      {lvl === "low" ? "Niedrig" : lvl === "medium" ? "Mittel" : "Hoch"}
                    </label>
                  ))}
                </div>
              </DspModule>

              <DspModule
                label="AGC (Automatic Gain Control)"
                enabled={settings.dsp.agc.enabled}
                onToggle={(v) => updateDsp("agc", { enabled: v })}
                tooltip="Regelt die Lautstarke automatisch auf einen Zielwert"
              >
                <AudioSlider
                  label="Zielpegel"
                  value={settings.dsp.agc.targetLevel}
                  min={-40}
                  max={0}
                  step={1}
                  unit=" dB"
                  onChange={(v) => updateDsp("agc", { targetLevel: v })}
                />
                <AudioSlider
                  label="Max. Verstarkung"
                  value={settings.dsp.agc.maxGain}
                  min={0}
                  max={60}
                  step={1}
                  unit=" dB"
                  onChange={(v) => updateDsp("agc", { maxGain: v })}
                />
                <AudioSlider
                  label="Attack"
                  value={settings.dsp.agc.attack}
                  min={1}
                  max={200}
                  step={1}
                  unit=" ms"
                  onChange={(v) => updateDsp("agc", { attack: v })}
                />
                <AudioSlider
                  label="Release"
                  value={settings.dsp.agc.release}
                  min={10}
                  max={2000}
                  step={10}
                  unit=" ms"
                  onChange={(v) => updateDsp("agc", { release: v })}
                />
              </DspModule>

              <DspModule
                label="Echo-Kompensation"
                enabled={settings.dsp.echoCancellation.enabled}
                onToggle={(v) => updateDsp("echoCancellation", { enabled: v })}
                tooltip="Verhindert Ruckkopplungen bei Lautsprechernutzung"
              >
                <AudioSlider
                  label="Tail Length"
                  value={settings.dsp.echoCancellation.tailLength}
                  min={16}
                  max={500}
                  step={4}
                  unit=" ms"
                  onChange={(v) => updateDsp("echoCancellation", { tailLength: v })}
                />
              </DspModule>

              <DspModule
                label="De-Esser"
                enabled={settings.dsp.deesser.enabled}
                onToggle={(v) => updateDsp("deesser", { enabled: v })}
                tooltip="Reduziert scharfe Zischlaute (S, Sch, Z)"
              >
                <AudioSlider
                  label="Frequenz"
                  value={settings.dsp.deesser.frequency}
                  min={2000}
                  max={16000}
                  step={100}
                  unit=" Hz"
                  onChange={(v) => updateDsp("deesser", { frequency: v })}
                />
                <AudioSlider
                  label="Schwelle"
                  value={settings.dsp.deesser.threshold}
                  min={-40}
                  max={0}
                  step={1}
                  unit=" dB"
                  onChange={(v) => updateDsp("deesser", { threshold: v })}
                />
                <AudioSlider
                  label="Ratio"
                  value={settings.dsp.deesser.ratio}
                  min={1}
                  max={20}
                  step={0.5}
                  unit=":1"
                  onChange={(v) => updateDsp("deesser", { ratio: v })}
                />
              </DspModule>
            </div>
          </section>

          {/* Jitter Buffer */}
          <section class={styles.section}>
            <h2 class={styles.sectionTitle}>Jitter Buffer</h2>
            <div class={styles.sectionBody}>
              <JitterSettings config={settings.jitter} onChange={updateJitter} />
            </div>
          </section>

          {/* Latenz-Aufschlusselung */}
          <section class={styles.section}>
            <h2 class={styles.sectionTitle}>Latenz-Aufschlusselung</h2>
            <div class={styles.sectionBody}>
              <LatencyBreakdown data={stats().latency} />
            </div>
          </section>

          {/* Reset */}
          <section class={styles.section}>
            <h2 class={styles.sectionTitle}>Zurucksetzen</h2>
            <div class={`${styles.sectionBody} ${styles.resetRow}`}>
              <button class={styles.resetBtn} onClick={resetToDefaults}>
                Auf Standardwerte zurucksetzen
              </button>
            </div>
          </section>
        </Show>

        {/* ---- Live Monitor (immer sichtbar) ---- */}
        <div class={styles.monitorWrapper}>
          <h2 class={styles.sectionTitle}>Live-Monitor</h2>
          <LiveMonitor stats={stats()} />
        </div>
      </div>

      {/* Kalibrierungs-Modal */}
      <Show when={showCalibration()}>
        <CalibrationModal
          onClose={() => setShowCalibration(false)}
          onApply={handleCalibrationApply}
        />
      </Show>
    </div>
  );
}

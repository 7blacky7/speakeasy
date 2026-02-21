import { invoke } from "@tauri-apps/api/core";

// --- Typen ---

export interface AudioDevice {
  id: string;
  name: string;
  kind: "input" | "output";
  is_default: boolean;
}

export interface AudioConfig {
  input_device_id: string | null;
  output_device_id: string | null;
  input_volume: number;
  output_volume: number;
  noise_suppression: boolean;
  echo_cancellation: boolean;
}

// --- Erweiterte Audio-Typen (Phase 3) ---

export interface CodecConfig {
  sampleRate: number;
  bufferSize: number;
  bitrate: number;
  frameSize: number;
  application: "voip" | "audio" | "low_delay";
  fec: boolean;
  dtx: boolean;
  channels: "mono" | "stereo";
}

export interface DspConfig {
  noiseGate: {
    enabled: boolean;
    threshold: number;
    attack: number;
    release: number;
  };
  noiseSuppression: {
    enabled: boolean;
    level: "low" | "medium" | "high";
  };
  agc: {
    enabled: boolean;
    targetLevel: number;
    maxGain: number;
    attack: number;
    release: number;
  };
  echoCancellation: {
    enabled: boolean;
    tailLength: number;
  };
  deesser: {
    enabled: boolean;
    frequency: number;
    threshold: number;
    ratio: number;
  };
}

export interface JitterConfig {
  minBuffer: number;
  maxBuffer: number;
  adaptive: boolean;
}

export interface AudioSettingsConfig {
  inputDeviceId: string | null;
  outputDeviceId: string | null;
  voiceMode: "ptt_hold" | "ptt_toggle" | "vad";
  pttKey: string | null;
  vadSensitivity: number;
  preset: "speech" | "balanced" | "music" | "low_bandwidth" | "custom";
  noiseSuppression: "off" | "low" | "medium" | "high";
  inputVolume: number;
  outputVolume: number;
  codec: CodecConfig;
  dsp: DspConfig;
  jitter: JitterConfig;
}

export interface LatencyBreakdown {
  device: number;
  encoding: number;
  jitter: number;
  network: number;
  total: number;
}

export interface AudioStats {
  inputLevel: number;
  outputLevel: number;
  processedLevel: number;
  noiseFloor: number;
  isClipping: boolean;
  latency: LatencyBreakdown;
  packetLoss: number;
  rtt: number;
  bitrate: number;
}

export interface CalibrationResult {
  success: boolean;
  suggestedVadSensitivity: number;
  suggestedInputVolume: number;
  noiseFloor: number;
}

// --- IPC Commands (Phase 3) ---

export async function getAudioSettings(): Promise<AudioSettingsConfig> {
  return invoke("get_audio_settings");
}

export async function setAudioSettings(
  config: AudioSettingsConfig
): Promise<void> {
  return invoke("set_audio_settings", { config });
}

export async function startCalibration(): Promise<CalibrationResult> {
  return invoke("start_calibration");
}

export async function getAudioStats(): Promise<AudioStats> {
  return invoke("get_audio_stats");
}

export async function playTestSound(): Promise<void> {
  return invoke("play_test_sound");
}

export interface ServerInfo {
  name: string;
  description: string;
  version: string;
  max_clients: number;
  online_clients: number;
  channels: ChannelInfo[];
}

export interface ChannelInfo {
  id: string;
  name: string;
  description: string;
  parent_id: string | null;
  clients: ClientInfo[];
  max_clients: number;
}

export interface ClientInfo {
  id: string;
  username: string;
  is_muted: boolean;
  is_deafened: boolean;
  is_self: boolean;
}

export interface ConnectOptions {
  address: string;
  port: number;
  username: string;
  password?: string;
}

// --- IPC Commands ---

export async function connectToServer(opts: ConnectOptions): Promise<void> {
  return invoke("connect_to_server", {
    address: opts.address,
    port: opts.port,
    username: opts.username,
    password: opts.password ?? null,
  });
}

export async function disconnect(): Promise<void> {
  return invoke("disconnect");
}

export async function joinChannel(channelId: string): Promise<void> {
  return invoke("join_channel", { channelId });
}

export async function leaveChannel(): Promise<void> {
  return invoke("leave_channel");
}

export async function getAudioDevices(): Promise<AudioDevice[]> {
  return invoke("get_audio_devices");
}

export async function setAudioConfig(config: AudioConfig): Promise<void> {
  return invoke("set_audio_config", { config });
}

export async function toggleMute(): Promise<boolean> {
  return invoke("toggle_mute");
}

export async function toggleDeafen(): Promise<boolean> {
  return invoke("toggle_deafen");
}

export async function getServerInfo(): Promise<ServerInfo> {
  return invoke("get_server_info");
}

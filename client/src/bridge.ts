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

// --- Chat-Typen (Phase 4) ---

export interface FileInfo {
  id: string;
  filename: string;
  mime_type: string;
  size_bytes: number;
}

export interface ChatMessage {
  id: string;
  channel_id: string;
  sender_id: string;
  sender_name: string;
  content: string;
  message_type: "text" | "file" | "system";
  reply_to: string | null;
  file_info: FileInfo | null;
  created_at: string;
  edited_at: string | null;
}

// --- Chat IPC Commands (Phase 4) ---

export async function sendMessage(
  channelId: string,
  content: string,
  replyTo?: string
): Promise<ChatMessage> {
  return invoke("send_message", {
    channelId,
    content,
    replyTo: replyTo ?? null,
  });
}

export async function getMessageHistory(
  channelId: string,
  before?: string,
  limit?: number
): Promise<ChatMessage[]> {
  return invoke("get_message_history", {
    channelId,
    before: before ?? null,
    limit: limit ?? 50,
  });
}

export async function editMessage(
  messageId: string,
  content: string
): Promise<ChatMessage> {
  return invoke("edit_message", { messageId, content });
}

export async function deleteMessage(messageId: string): Promise<void> {
  return invoke("delete_message", { messageId });
}

export async function uploadFile(
  channelId: string,
  file: File
): Promise<ChatMessage> {
  const buffer = await file.arrayBuffer();
  const data = Array.from(new Uint8Array(buffer));
  return invoke("upload_file", {
    channelId,
    filename: file.name,
    mimeType: file.type || "application/octet-stream",
    data,
  });
}

export async function downloadFile(fileId: string): Promise<Uint8Array> {
  const data: number[] = await invoke("download_file", { fileId });
  return new Uint8Array(data);
}

export async function listFiles(channelId: string): Promise<FileInfo[]> {
  return invoke("list_files", { channelId });
}

// --- Channel-CRUD Commands (Phase 8.1) ---

export async function createChannel(
  name: string,
  description?: string,
  password?: string,
  maxClients?: number,
  parentId?: string
): Promise<ChannelInfo> {
  return invoke("create_channel", {
    name,
    description: description ?? null,
    password: password ?? null,
    maxClients: maxClients ?? null,
    parentId: parentId ?? null,
  });
}

export async function editChannel(
  channelId: string,
  name?: string,
  description?: string,
  password?: string,
  maxClients?: number
): Promise<void> {
  return invoke("edit_channel", {
    channelId,
    name: name ?? null,
    description: description ?? null,
    password: password ?? null,
    maxClients: maxClients ?? null,
  });
}

export async function deleteChannel(channelId: string): Promise<void> {
  return invoke("delete_channel", { channelId });
}

// --- Plugin-Typen (Phase 5) ---

export type PluginState = "Geladen" | "Aktiv" | "Deaktiviert" | { Fehler: string };
export type TrustLevel = "NichtSigniert" | "Signiert" | "Vertrauenswuerdig";

export interface PluginCapabilities {
  filesystem: boolean;
  network: boolean;
  audio_read: boolean;
  audio_write: boolean;
  chat_read: boolean;
  chat_write: boolean;
  user_management: boolean;
  server_config: boolean;
}

export interface PluginInfo {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  state: PluginState;
  trust_level: TrustLevel;
  geladen_am: string;
}

export interface PluginInstallResult {
  id: string;
  name: string;
  trust_level: TrustLevel;
}

// --- Plugin IPC Commands (Phase 5) ---

export async function listPlugins(): Promise<PluginInfo[]> {
  return invoke("list_plugins");
}

export async function enablePlugin(id: string): Promise<void> {
  return invoke("enable_plugin", { id });
}

export async function disablePlugin(id: string): Promise<void> {
  return invoke("disable_plugin", { id });
}

export async function unloadPlugin(id: string): Promise<void> {
  return invoke("unload_plugin", { id });
}

export async function installPlugin(path: string): Promise<PluginInstallResult> {
  return invoke("install_plugin", { path });
}

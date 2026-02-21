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

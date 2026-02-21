import type { AudioSettingsConfig } from "../bridge";

export interface SoundProfile {
  id: string;
  name: string;
  settings: AudioSettingsConfig;
  createdAt: string;
  updatedAt: string;
}

const STORAGE_KEY = "speakeasy-sound-profiles";
const ACTIVE_KEY = "speakeasy-active-profile-id";

export function loadProfiles(): SoundProfile[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

export function saveProfiles(profiles: SoundProfile[]): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(profiles));
}

export function createProfile(
  name: string,
  settings: AudioSettingsConfig
): SoundProfile {
  const profiles = loadProfiles();
  const profile: SoundProfile = {
    id: crypto.randomUUID(),
    name,
    settings,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };
  profiles.push(profile);
  saveProfiles(profiles);
  return profile;
}

export function updateProfile(
  id: string,
  updates: Partial<Pick<SoundProfile, "name" | "settings">>
): SoundProfile | null {
  const profiles = loadProfiles();
  const idx = profiles.findIndex((p) => p.id === id);
  if (idx === -1) return null;
  if (updates.name !== undefined) profiles[idx].name = updates.name;
  if (updates.settings !== undefined) profiles[idx].settings = updates.settings;
  profiles[idx].updatedAt = new Date().toISOString();
  saveProfiles(profiles);
  return profiles[idx];
}

export function deleteProfile(id: string): void {
  const profiles = loadProfiles().filter((p) => p.id !== id);
  saveProfiles(profiles);
  if (getActiveProfileId() === id) {
    setActiveProfileId(null);
  }
}

export function getProfileById(id: string): SoundProfile | undefined {
  return loadProfiles().find((p) => p.id === id);
}

export function getActiveProfileId(): string | null {
  return localStorage.getItem(ACTIVE_KEY);
}

export function setActiveProfileId(id: string | null): void {
  if (id === null) {
    localStorage.removeItem(ACTIVE_KEY);
  } else {
    localStorage.setItem(ACTIVE_KEY, id);
  }
}

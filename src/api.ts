import { invoke } from '@tauri-apps/api/core';

export type RGB = [number, number, number];
export type Target = { deviceId: string; zoneId: string };
export type Assignment = { target: Target; mode: string; color: RGB; brightness: number; speed: number };
export type Effect = { id: string; name: string; color: boolean; brightness: boolean };
export type Zone = { id: string; name: string; effects: Effect[]; direct: boolean };
export type Device = { id: string; name: string; detail: string; zones: Zone[] };
export type Profile = { id: string; name: string; assignments: Assignment[] };
export type ApplyResult = { count: number; issues: string[]; active: Assignment[] };

export const api = {
  language: (language: string) => invoke<void>('set_language', { language }),
  devices: (refresh = false) => invoke<Device[]>('devices', { refresh }),
  apply: (assignments: Assignment[]) => invoke<ApplyResult>('apply_lighting', { assignments }),
  profiles: () => invoke<Profile[]>('list_profiles'),
  save: (name: string, assignments: Assignment[]) => invoke<Profile[]>('save_profile', { name, assignments }),
  rename: (id: string, name: string) => invoke<Profile[]>('rename_profile', { id, name }),
  delete: (id: string) => invoke<Profile[]>('delete_profile', { id }),
  applyProfile: (id: string) => invoke<ApplyResult>('apply_profile', { id }),
};

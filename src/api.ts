import { invoke } from '@tauri-apps/api/core';

export type RGB = [number, number, number];
export type Target = { deviceId: string; zoneId: string };
export type Assignment = { target: Target; mode: string; color: RGB; brightness: number; speed: number };
export type Effect = { id: string; name: string; color: boolean; brightness: boolean };
export type Zone = { id: string; name: string; effects: Effect[]; direct: boolean };
export type Device = {
  id: string; name: string; detail: string; zones: Zone[];
  needsPawnioInstall: boolean; needsPawnioElevation: boolean;
};
export type Profile = { id: string; name: string; assignments: Assignment[] };
export type ProfileLibrary = { profiles: Profile[]; startupId: string | null };
export type Calibration = { red: number; green: number; blue: number };
export type ApplyResult = { revision: number; count: number; issues: string[]; active: Assignment[] };

export const api = {
  language: (language: string) => invoke<void>('set_language', { language }),
  devices: (refresh = false) => invoke<{ devices: Device[]; state: ApplyResult; calibrations: Record<string, Calibration> }>('devices', { refresh }),
  calibrate: (deviceId: string, calibration: Calibration) => invoke<ApplyResult>('set_calibration', { deviceId, calibration }),
  installPawnio: () => invoke<'installed' | 'cancelled' | 'restartRequired'>('install_pawnio'),
  restartForRgbMemory: () => invoke<void>('restart_for_rgb_memory'),
  apply: (assignments: Assignment[]) => invoke<ApplyResult>('apply_lighting', { assignments }),
  profiles: () => invoke<ProfileLibrary>('list_profiles'),
  save: (name: string) => invoke<ProfileLibrary>('save_profile', { name }),
  replace: (id: string) => invoke<ProfileLibrary>('replace_profile', { id }),
  startup: (id: string | null) => invoke<ProfileLibrary>('set_startup_profile', { id }),
  rename: (id: string, name: string) => invoke<ProfileLibrary>('rename_profile', { id, name }),
  delete: (id: string) => invoke<ProfileLibrary>('delete_profile', { id }),
  applyProfile: (id: string) => invoke<ApplyResult>('apply_profile', { id }),
};

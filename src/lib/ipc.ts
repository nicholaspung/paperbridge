// The only file that calls Tauri. Everything else uses these typed wrappers.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DocumentRecord } from "./bindings/DocumentRecord";
import type { PrivacyStatus } from "./bindings/PrivacyStatus";
import type { Settings } from "./bindings/Settings";
import type { Status } from "./bindings/Status";

export const getStatus = () => invoke<Status>("get_status");
export const getLibrary = () => invoke<DocumentRecord[]>("get_library");
export const getPrivacy = () => invoke<PrivacyStatus>("get_privacy");
export const getSettings = () => invoke<Settings>("get_settings");
export const completeFirstRun = () => invoke<void>("complete_first_run");
export const refresh = () => invoke<void>("refresh");

export const onStatusChanged = (fn: (s: Status) => void): Promise<UnlistenFn> =>
  listen<Status>("status-changed", (e) => fn(e.payload));
export const onLibraryUpdated = (fn: () => void): Promise<UnlistenFn> =>
  listen("library-updated", () => fn());

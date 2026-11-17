// The only file that calls Tauri. Everything else uses these typed wrappers.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DocumentRecord } from "./bindings/DocumentRecord";
import type { Job } from "./bindings/Job";
import type { ViewFormat } from "./bindings/ViewFormat";
import type { PrivacyStatus } from "./bindings/PrivacyStatus";
import type { Settings } from "./bindings/Settings";
import type { Status } from "./bindings/Status";

export const getStatus = () => invoke<Status>("get_status");
export const getLibrary = () => invoke<DocumentRecord[]>("get_library");
export const getPrivacy = () => invoke<PrivacyStatus>("get_privacy");
export const getSettings = () => invoke<Settings>("get_settings");
export const completeFirstRun = () => invoke<void>("complete_first_run");
export const refresh = () => invoke<void>("refresh");
export const openDocument = (id: string, format: ViewFormat, external = false) =>
  invoke<void>("open_document", { id, format, external });
export const openExternal = (id: string, format: ViewFormat) => invoke<void>("open_external", { id, format });
export const getDocumentFile = async (id: string, format: ViewFormat) =>
  new Uint8Array(await invoke<ArrayBuffer>("get_document_file", { id, format }));
export const cancelJob = () => invoke<void>("cancel_job");
export const dismissJob = () => invoke<void>("dismiss_job");
export const getJob = () => invoke<Job | null>("get_job");

export const onStatusChanged = (fn: (s: Status) => void): Promise<UnlistenFn> =>
  listen<Status>("status-changed", (e) => fn(e.payload));
export const onLibraryUpdated = (fn: () => void): Promise<UnlistenFn> =>
  listen("library-updated", () => fn());
export const onJobChanged = (fn: (j: Job | null) => void): Promise<UnlistenFn> =>
  listen<Job | null>("job-changed", (e) => fn(e.payload));

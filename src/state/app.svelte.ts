import * as ipc from "../lib/ipc";
import type { DocumentRecord } from "../lib/bindings/DocumentRecord";
import type { Job } from "../lib/bindings/Job";
import type { PrivacyStatus } from "../lib/bindings/PrivacyStatus";
import type { Settings } from "../lib/bindings/Settings";
import type { Status } from "../lib/bindings/Status";

class AppStore {
  status = $state<Status | null>(null);
  library = $state<DocumentRecord[]>([]);
  privacy = $state<PrivacyStatus | null>(null);
  settings = $state<Settings | null>(null);
  error = $state<string | null>(null);
  job = $state<Job | null>(null);
  /** Why the last Open request was refused before it started. */
  openRefused = $state<string | null>(null);

  async init() {
    try {
      await ipc.onStatusChanged((s) => {
        this.status = s;
        void this.loadPrivacy();
      });
      await ipc.onLibraryUpdated(() => void this.loadLibrary());
      await ipc.onJobChanged((j) => (this.job = j));
      [this.settings, this.status, this.job] = await Promise.all([
        ipc.getSettings(),
        ipc.getStatus(),
        ipc.getJob(),
      ]);
      await Promise.all([this.loadLibrary(), this.loadPrivacy()]);
    } catch (e) {
      this.error = String(e);
    }
  }

  async loadLibrary() {
    try {
      this.library = await ipc.getLibrary();
    } catch (e) {
      this.error = String(e);
    }
  }

  async loadPrivacy() {
    try {
      this.privacy = await ipc.getPrivacy();
    } catch (e) {
      this.error = String(e);
    }
  }

  async finishFirstRun() {
    try {
      await ipc.completeFirstRun();
      if (this.settings) this.settings.firstRunDone = true;
    } catch (e) {
      this.error = String(e);
    }
  }

  get jobActive(): boolean {
    const p = this.job?.phase.phase;
    return p === "waiting" || p === "rendering" || p === "receiving" || p === "opening";
  }

  async open(rec: DocumentRecord) {
    this.openRefused = null;
    try {
      await ipc.openDocument(rec.id);
    } catch (e) {
      this.openRefused = String(e);
    }
  }

  cancelJob() {
    ipc.cancelJob().catch((e) => (this.error = String(e)));
  }

  dismissJob() {
    this.openRefused = null;
    ipc.dismissJob().catch((e) => (this.error = String(e)));
  }

  refresh() {
    ipc.refresh().catch((e) => (this.error = String(e)));
  }
}

export const app = new AppStore();

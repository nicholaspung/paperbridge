import * as ipc from "../lib/ipc";
import type { DocumentRecord } from "../lib/bindings/DocumentRecord";
import type { Job } from "../lib/bindings/Job";
import type { ViewFormat } from "../lib/bindings/ViewFormat";
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
  /** The document shown in the app's viewer. */
  viewing = $state<{ id: string; title: string; format: ViewFormat } | null>(null);

  async init() {
    try {
      await ipc.onStatusChanged((s) => {
        this.status = s;
        void this.loadPrivacy();
      });
      await ipc.onLibraryUpdated(() => void this.loadLibrary());
      await ipc.onJobChanged((j) => this.onJob(j));
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

  private onJob(j: Job | null) {
    this.job = j;
    if (j && j.phase.phase === "done" && !j.external) {
      this.viewing = { id: j.documentId, title: j.title, format: j.format };
    }
  }

  get jobActive(): boolean {
    const p = this.job?.phase.phase;
    return p === "waiting" || p === "checking" || p === "rendering" || p === "receiving" || p === "opening";
  }

  /** Books open as their original EPUB; everything else as the tablet's PDF. */
  async open(rec: DocumentRecord, format: ViewFormat = rec.kind === "epub" ? "epub" : "pdf", external = false) {
    this.openRefused = null;
    try {
      await ipc.openDocument(rec.id, format, external);
    } catch (e) {
      this.openRefused = String(e);
    }
  }

  closeViewer() {
    this.viewing = null;
  }

  /** Shows the document being viewed in the other format. */
  switchFormat(format: ViewFormat) {
    const rec = this.library.find((r) => r.id === this.viewing?.id);
    if (rec && format !== this.viewing?.format && !this.jobActive) void this.open(rec, format);
  }

  openExternal() {
    const v = this.viewing;
    if (v) ipc.openExternal(v.id, v.format).catch((e) => (this.openRefused = String(e)));
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

// PDF.js, bundled with the app. The legacy build supports the older WebKit
// versions shipped by LTS Linux distributions and older macOS releases.
// Loaded lazily by the viewer, so the rest of the UI does not pay for it.
import * as pdfjs from "pdfjs-dist/legacy/build/pdf.mjs";
import workerUrl from "pdfjs-dist/legacy/build/pdf.worker.min.mjs?url";
import type { PDFDocumentProxy } from "pdfjs-dist/legacy/build/pdf.mjs";

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;

export type { PDFDocumentProxy };

export interface LoadedPdf {
  doc: PDFDocumentProxy;
  destroy: () => void;
}

export async function loadPdf(data: Uint8Array): Promise<LoadedPdf> {
  // PDF.js 6 no longer evaluates generated code, so the CSP needs no eval.
  const task = pdfjs.getDocument({ data, enableXfa: false, stopAtErrors: false });
  return { doc: await task.promise, destroy: () => void task.destroy() };
}

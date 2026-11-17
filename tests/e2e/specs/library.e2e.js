import { MOCK_URL } from "../wdio.conf.js";

const fault = (name) => fetch(`${MOCK_URL}/__mock/fault/${name}`);
const rows = () => $$('[data-testid="row"]');
const badgeState = () => $('[data-testid="connection-badge"]').getAttribute("data-state");

describe("PaperBridge against the mock tablet", () => {
  after(() => fault("none"));

  it("shows the first-launch guide and connects", async () => {
    await expect($('[data-testid="first-run"]')).toBeDisplayed();
    await browser.waitUntil(async () => (await badgeState()) === "connected", {
      timeout: 20_000,
      timeoutMsg: "never connected to the mock tablet",
    });
    await expect($('[data-testid="privacy-badge"]')).toHaveAttribute("aria-label", "Isolation unverified");
    await $('[data-testid="first-run-continue"]').click();
    await expect($('[data-testid="library"]')).toBeDisplayed();
  });

  it("lists the root folder from the fixtures, folders first", async () => {
    await browser.waitUntil(async () => (await rows().length) === 18, {
      timeoutMsg: "expected the 18 root items of the 3.29.1.0 fixtures",
    });
    await expect(rows()[0]).toHaveAttribute("data-kind", "folder");
  });

  it("opens a folder and comes back", async () => {
    await rows()[0].click();
    await browser.waitUntil(async () => (await $('[data-testid="breadcrumbs"]').$$("button").length) === 2);
    await $('[data-testid="breadcrumbs"]').$$("button")[0].click();
    await browser.waitUntil(async () => (await rows().length) === 18);
  });

  it("searches the whole library and filters by type", async () => {
    await $('[data-testid="filter-epub"]').click();
    await browser.waitUntil(async () => (await rows().length) === 8, { timeoutMsg: "expected 8 EPUBs" });
    await $('[data-testid="filter-epub"]').click();
    await $('[data-testid="search"]').setValue("no such title anywhere");
    await expect($('[data-testid="empty"]')).toBeDisplayed();
    await $('[data-testid="clear-search"]').click();
    await browser.waitUntil(async () => (await rows().length) === 18);
  });

  it("opens a document in the app, reopens it from the local copy, and re-exports after an edit", async () => {
    const bar = () => $('[data-testid="job-bar"]');
    const jobPhase = () => bar().getAttribute("data-phase");
    const openFirstPdf = async () => {
      await rows()[0].click();
      await browser.waitUntil(async () => (await jobPhase()) === "done", { timeout: 20_000, timeoutMsg: "export did not finish" });
      await expect($('[data-testid="viewer"]')).toBeDisplayed();
      // PDF.js really draws the page inside the app's WebView, under the app's CSP.
      await browser.waitUntil(async () => (await $$('[data-testid="pdf-page"][data-rendered="true"]').length) > 0, {
        timeout: 15_000,
        timeoutMsg: "the PDF page was not rendered",
      });
      return await bar().getText();
    };
    const closeViewer = async () => {
      await $('[data-testid="viewer-close"]').click();
      await expect($('[data-testid="library"]')).toBeDisplayed();
      if (await $('[data-testid="job-dismiss"]').isExisting()) await $('[data-testid="job-dismiss"]').click();
    };

    await $('[data-testid="filter-pdf"]').click();
    await browser.waitUntil(async () => (await rows().length) === 7, { timeoutMsg: "expected the 7 PDFs of the fixtures" });
    const id = await rows()[0].getAttribute("data-id");

    expect(await openFirstPdf()).not.toContain("from the copy on this computer");
    await closeViewer();

    expect(await openFirstPdf()).toContain("from the copy on this computer");
    await $('[data-testid="viewer-external"]').click(); // dry run: no viewer is launched in CI
    await closeViewer();

    // Written on since: the app must notice without a library refresh.
    await fetch(`${MOCK_URL}/__mock/touch/${id}`);
    expect(await openFirstPdf()).not.toContain("from the copy on this computer");
    await closeViewer();
  });

  it("reads a book as its EPUB without running its scripts, and switches to the annotated PDF", async () => {
    await $('[data-testid="filter-pdf"]').click(); // back to the full list
    await $('[data-testid="filter-epub"]').click();
    await browser.waitUntil(async () => (await rows().length) === 8, { timeoutMsg: "expected 8 EPUBs" });
    await rows()[0].click();
    const reader = $('[data-testid="epub-reader"]');
    await browser.waitUntil(async () => (await reader.getAttribute("data-ready")) === "true", {
      timeout: 20_000,
      timeoutMsg: "the book did not open",
    });
    await browser.waitUntil(async () => Number(await reader.getAttribute("data-sections")) > 0);
    // The mock book's first chapter tries to run a script and an onload handler.
    expect(await $("body").getAttribute("data-epub-script")).toBeNull();
    expect(await $("body").getAttribute("data-epub-onload")).toBeNull();
    await $('[data-testid="epub-next"]').click();

    await $('[data-testid="show-pdf"]').click();
    await browser.waitUntil(async () => (await $$('[data-testid="pdf-page"][data-rendered="true"]').length) > 0, {
      timeout: 20_000,
      timeoutMsg: "the annotated PDF was not shown",
    });
    await $('[data-testid="viewer-close"]').click();
    await expect($('[data-testid="library"]')).toBeDisplayed();
    if (await $('[data-testid="job-dismiss"]').isExisting()) await $('[data-testid="job-dismiss"]').click();
    await $('[data-testid="filter-epub"]').click();
    await $('[data-testid="filter-pdf"]').click();
    await browser.waitUntil(async () => (await rows().length) === 7);
  });

  it("shows a slow export as rendering and can cancel it", async () => {
    await fault("render");
    await rows()[1].click();
    const bar = $('[data-testid="job-bar"]');
    await browser.waitUntil(async () => (await bar.getAttribute("data-phase")) === "rendering");
    await expect(bar).toHaveText(expect.stringContaining("preparing"));
    await $('[data-testid="job-cancel"]').click();
    await browser.waitUntil(async () => (await bar.getAttribute("data-phase")) === "cancelled");
    await fault("none");
    await $('[data-testid="filter-pdf"]').click();
    await browser.waitUntil(async () => (await rows().length) === 18);
  });

  it("explains a hung tablet, keeps the cached library, and recovers", async () => {
    await fault("hang");
    await $('[data-testid="refresh"]').click();
    await browser.waitUntil(async () => (await badgeState()) === "webInterfaceHung", {
      timeout: 30_000,
      timeoutMsg: "the hung web server was not reported",
    });
    await expect($('[data-testid="connection-help"]')).toHaveText(expect.stringContaining("off, then on again"));
    await expect(rows()).toBeElementsArrayOfSize(18);

    await fault("none");
    await browser.waitUntil(async () => (await badgeState()) === "connected", {
      timeout: 40_000,
      timeoutMsg: "did not reconnect after the mock recovered",
    });
  });
});

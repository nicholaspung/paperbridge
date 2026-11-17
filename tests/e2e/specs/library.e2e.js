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

  it("opens a document, then reopens it from the local copy", async () => {
    const jobPhase = () => $('[data-testid="job-bar"]').getAttribute("data-phase");
    await $('[data-testid="filter-pdf"]').click();
    await browser.waitUntil(async () => (await rows().length) === 8, { timeoutMsg: "expected 8 PDFs" });
    await rows()[0].click();
    await browser.waitUntil(async () => (await jobPhase()) === "done", { timeout: 20_000, timeoutMsg: "export did not finish" });
    await expect($('[data-testid="job-bar"]')).not.toHaveText(expect.stringContaining("from the copy on this computer"));
    await $('[data-testid="job-dismiss"]').click();

    await rows()[0].click();
    await browser.waitUntil(async () => (await jobPhase()) === "done");
    await expect($('[data-testid="job-bar"]')).toHaveText(expect.stringContaining("from the copy on this computer"));
    await $('[data-testid="job-dismiss"]').click();
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

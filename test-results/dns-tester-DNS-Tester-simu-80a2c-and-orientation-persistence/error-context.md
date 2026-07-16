# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: dns-tester.spec.js >> DNS Tester simulates Wi-Fi startup, BOOT reset, and orientation persistence
- Location: tests/browser/dns-tester.spec.js:3:5

# Error details

```
Error: expect(locator).toContainText(expected) failed

Locator: locator('.cyd-simulator-notice')
Expected substring: "Wi-Fi connection is simulated"
Received string:    ""
Timeout: 5000ms

Call log:
  - Expect "toContainText" with timeout 5000ms
  - waiting for locator('.cyd-simulator-notice')
    14 × locator resolved to <div hidden="" role="status" aria-live="polite" aria-atomic="true" class="cyd-simulator-notice"></div>
       - unexpected value ""

```

```yaml
- img "CYD device case"
- button "boot"
- link "Back to gallery":
  - /url: ../../
  - text: ← Gallery
- button "CYD demo DNS Tester Exercise CYD touch, DNS queries, and orientation behavior in your browser. tap for details ›":
  - text: CYD demo
  - strong: DNS Tester
  - text: Exercise CYD touch, DNS queries, and orientation behavior in your browser. tap for details ›
- button "full-screen mode"
```

# Test source

```ts
  1   | import { expect, test } from "@playwright/test";
  2   | 
  3   | test("DNS Tester simulates Wi-Fi startup, BOOT reset, and orientation persistence", async ({ page }) => {
  4   |   test.setTimeout(30_000);
  5   |   const pageErrors = [];
  6   |   const consoleErrors = [];
  7   |   page.on("pageerror", (error) => pageErrors.push(error.stack ?? String(error)));
  8   |   page.on("console", (message) => {
  9   |     if (message.type() === "error") {
  10  |       consoleErrors.push(message.text());
  11  |     }
  12  |   });
  13  | 
  14  |   await page.goto("/dns-tester/");
  15  |   const canvas = page.locator("#screen");
  16  |   await expect(canvas).toHaveAttribute("width", "320");
  17  |   await expect(canvas).toHaveAttribute("height", "240");
  18  | 
  19  |   const canvasBounds = await canvas.boundingBox();
  20  |   expect(canvasBounds).not.toBeNull();
  21  |   if (!canvasBounds) {
  22  |     return;
  23  |   }
  24  | 
  25  |   const screenPoint = (x, y) => [
  26  |     canvasBounds.x + canvasBounds.width * (x / 320),
  27  |     canvasBounds.y + canvasBounds.height * (y / 240),
  28  |   ];
  29  | 
  30  |   // A fresh browser context has no calibration flash record. Complete the
  31  |   // real four-target flow before exercising the Wi-Fi startup path.
  32  |   await page.waitForTimeout(500);
  33  |   for (const target of [[40, 40], [279, 40], [279, 199], [40, 199]]) {
  34  |     await page.mouse.click(...screenPoint(...target));
  35  |     await page.waitForTimeout(500);
  36  |   }
  37  |   await page.mouse.click(...screenPoint(160, 120));
  38  |   await page.waitForTimeout(500);
  39  | 
  40  |   // Startup includes the shared simulated captive-portal and connecting phases.
  41  |   await page.waitForTimeout(5_000);
  42  |   expect({ pageErrors, consoleErrors }).toEqual({ pageErrors: [], consoleErrors: [] });
  43  | 
  44  |   // Start a lookup and immediately press BOOT. The shared DNS loop must
  45  |   // finish or safely leave the lookup, then return through calibration rather
  46  |   // than dropping the BOOT action or spawning a second dashboard loop.
  47  |   await page.mouse.click(...screenPoint(160, 120));
  48  |   const activeLookupBootBounds = await page.locator("#boot-button").boundingBox();
  49  |   expect(activeLookupBootBounds).not.toBeNull();
  50  |   if (!activeLookupBootBounds) {
  51  |     return;
  52  |   }
  53  |   await page.mouse.move(
  54  |     activeLookupBootBounds.x + activeLookupBootBounds.width / 2,
  55  |     activeLookupBootBounds.y + activeLookupBootBounds.height / 2,
  56  |   );
  57  |   await page.mouse.down();
  58  |   await page.waitForTimeout(60);
  59  |   await page.mouse.up();
  60  |   await page.waitForTimeout(1_000);
  61  |   expect({ pageErrors, consoleErrors }).toEqual({ pageErrors: [], consoleErrors: [] });
  62  | 
  63  |   // BOOT during the lookup must have entered the same calibration path as a
  64  |   // main-state BOOT. Complete it before continuing with the settings checks.
  65  |   for (const target of [[40, 40], [279, 40], [279, 199], [40, 199]]) {
  66  |     await page.mouse.click(...screenPoint(...target));
  67  |     await page.waitForTimeout(500);
  68  |   }
  69  |   await page.mouse.click(...screenPoint(160, 120));
  70  |   await page.waitForTimeout(5_000);
  71  | 
  72  |   // Wi-Fi control requests the simulated captive-portal reset and uses the
  73  |   // shared browser notice facility. The connecting notice may replace the
  74  |   // short-lived setup notice before the browser observes it.
  75  |   await page.mouse.click(...screenPoint(160, 216));
> 76  |   await expect(page.locator(".cyd-simulator-notice")).toContainText(
      |                                                       ^ Error: expect(locator).toContainText(expected) failed
  77  |     "Wi-Fi connection is simulated",
  78  |   );
  79  | 
  80  |   // BOOT during the simulated connect must release cleanly and restart rather
  81  |   // than leaving two application loops or a permanently held button.
  82  |   await page.waitForTimeout(250);
  83  |   const bootBounds = await page.locator("#boot-button").boundingBox();
  84  |   expect(bootBounds).not.toBeNull();
  85  |   if (!bootBounds) {
  86  |     return;
  87  |   }
  88  |   await page.mouse.click(
  89  |     bootBounds.x + bootBounds.width / 2,
  90  |     bootBounds.y + bootBounds.height / 2,
  91  |   );
  92  |   await page.waitForTimeout(3_000);
  93  |   expect({ pageErrors, consoleErrors }).toEqual({ pageErrors: [], consoleErrors: [] });
  94  | 
  95  |   // BOOT cleared calibration before restarting, so complete the real flow
  96  |   // again before testing the dashboard controls.
  97  |   for (const target of [[40, 40], [279, 40], [279, 199], [40, 199]]) {
  98  |     await page.mouse.click(...screenPoint(...target));
  99  |     await page.waitForTimeout(500);
  100 |   }
  101 |   await page.mouse.click(...screenPoint(160, 120));
  102 |   await page.waitForTimeout(500);
  103 | 
  104 |   const latestCanvasBounds = await canvas.boundingBox();
  105 |   expect(latestCanvasBounds).not.toBeNull();
  106 |   if (!latestCanvasBounds) {
  107 |     return;
  108 |   }
  109 | 
  110 |   // ROT stores the selected orientation in the app's flash namespace.
  111 |   const orientationPoint = [
  112 |     latestCanvasBounds.x + latestCanvasBounds.width * (260 / 320),
  113 |     latestCanvasBounds.y + latestCanvasBounds.height * (216 / 240),
  114 |   ];
  115 |   await page.mouse.click(...orientationPoint);
  116 |   await page.waitForTimeout(3_000);
  117 |   await expect(canvas).toHaveAttribute("height", "320");
  118 |   await page.reload();
  119 |   await page.waitForTimeout(3_000);
  120 |   await expect(canvas).toHaveAttribute("height", "320");
  121 | 
  122 |   // CAL must enter the same calibration transition as BOOT, including after
  123 |   // the dashboard has been rotated into portrait presentation.
  124 |   const portraitCanvasBounds = await canvas.boundingBox();
  125 |   expect(portraitCanvasBounds).not.toBeNull();
  126 |   if (!portraitCanvasBounds) {
  127 |     return;
  128 |   }
  129 |   await page.mouse.click(
  130 |     portraitCanvasBounds.x + portraitCanvasBounds.width * (46 / 240),
  131 |     portraitCanvasBounds.y + portraitCanvasBounds.height * (294 / 320),
  132 |   );
  133 |   await page.waitForTimeout(500);
  134 |   expect({ pageErrors, consoleErrors }).toEqual({ pageErrors: [], consoleErrors: [] });
  135 | });
  136 | 
```
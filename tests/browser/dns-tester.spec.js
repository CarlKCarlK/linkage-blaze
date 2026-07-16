import { expect, test } from "@playwright/test";

test("DNS Tester uses intrinsic browser touch, DNS latency, and orientation persistence", async ({ page }) => {
  test.setTimeout(30_000);
  const pageErrors = [];
  const consoleErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.stack ?? String(error)));
  page.on("console", (message) => {
    if (message.type() === "error") {
      consoleErrors.push(message.text());
    }
  });

  await page.goto("/dns-tester/");
  const calibrationKeys = await page.evaluate(() =>
    Object.keys(localStorage).filter((key) => key.includes("calibration")),
  );
  expect(calibrationKeys).toEqual([]);
  const canvas = page.locator("#screen");
  await expect(canvas).toHaveAttribute("width", "320");
  await expect(canvas).toHaveAttribute("height", "240");
  await expect(page.locator(".demo-ux-card-tag strong")).toHaveText("DNS Tester");
  await expect(page.locator(".demo-ux-card-tag__preview")).toHaveText(
    "Measure a deterministic simulated DNS lookup on a CYD.",
  );
  await page.locator(".demo-ux-card-tag").click();
  await expect(page.locator(".demo-ux-card-dialog h2")).toHaveText("DNS Tester");
  await expect(page.locator(".demo-ux-card-dialog")).toContainText(
    "The DNS tester exercises the shared device abstraction and reports a fixed browser simulation result.",
  );
  await expect(page.locator(".demo-ux-card-dialog")).toContainText(
    "Touch the panel and press BOOT to interact with the tester.",
  );
  await expect(page.locator(".demo-ux-card-dialog a", { hasText: "Core code" }))
    .toHaveAttribute("href", /dns_tester\.rs$/);
  await page.keyboard.press("Escape");
  await expect(page.locator(".demo-ux-time-chip")).toHaveCount(0);

  const canvasBounds = await canvas.boundingBox();
  expect(canvasBounds).not.toBeNull();
  if (!canvasBounds) {
    return;
  }

  const screenPoint = (x, y) => [
    canvasBounds.x + canvasBounds.width * (x / 320),
    canvasBounds.y + canvasBounds.height * (y / 240),
  ];

  // Startup includes the shared simulated captive-portal and connecting phases.
  await page.waitForTimeout(5_000);
  expect({ pageErrors, consoleErrors }).toEqual({ pageErrors: [], consoleErrors: [] });
  await expect(page.locator(".cyd-simulator-notice")).not.toContainText("Calibration");

  // The dashboard accepts the normal DNS action without a calibration phase.
  const dashboardImageBeforeLookup = await page.locator("#screen").evaluate(
    (screen) => screen.toDataURL(),
  );
  await page.mouse.click(...screenPoint(160, 120));
  await page.waitForTimeout(500);
  const dashboardImageAfterLookup = await page.locator("#screen").evaluate(
    (screen) => screen.toDataURL(),
  );
  expect(dashboardImageAfterLookup).not.toBe(dashboardImageBeforeLookup);

  // Wi-Fi control requests the simulated captive-portal reset and uses the
  // shared browser notice facility. The connecting notice may replace the
  // short-lived setup notice before the browser observes it.
  await page.mouse.click(...screenPoint(160, 216));
  await expect(page.locator(".cyd-simulator-notice")).toContainText(
    "Wi-Fi connection is simulated",
  );

  // BOOT during the simulated connect must release cleanly and restart rather
  // than leaving two application loops or a permanently held button.
  await page.waitForTimeout(250);
  const bootBounds = await page.locator("#boot-button").boundingBox();
  expect(bootBounds).not.toBeNull();
  if (!bootBounds) {
    return;
  }
  await page.mouse.click(
    bootBounds.x + bootBounds.width / 2,
    bootBounds.y + bootBounds.height / 2,
  );
  await page.waitForTimeout(3_000);
  expect({ pageErrors, consoleErrors }).toEqual({ pageErrors: [], consoleErrors: [] });

  const latestCanvasBounds = await canvas.boundingBox();
  expect(latestCanvasBounds).not.toBeNull();
  if (!latestCanvasBounds) {
    return;
  }

  // ROT stores the selected orientation in the app's flash namespace.
  const orientationPoint = [
    latestCanvasBounds.x + latestCanvasBounds.width * (260 / 320),
    latestCanvasBounds.y + latestCanvasBounds.height * (216 / 240),
  ];
  await page.mouse.click(...orientationPoint);
  await page.waitForTimeout(3_000);
  await expect(canvas).toHaveAttribute("height", "320");
  await page.reload();
  await page.waitForTimeout(3_000);
  await expect(canvas).toHaveAttribute("height", "320");

  // CAL must show the browser policy notice, including after the dashboard has
  // been rotated into portrait presentation.
  const portraitCanvasBounds = await canvas.boundingBox();
  expect(portraitCanvasBounds).not.toBeNull();
  if (!portraitCanvasBounds) {
    return;
  }
  await page.mouse.click(
    portraitCanvasBounds.x + portraitCanvasBounds.width * (46 / 240),
    portraitCanvasBounds.y + portraitCanvasBounds.height * (294 / 320),
  );
  await expect(page.locator(".cyd-simulator-notice")).toContainText(
    "Calibration is not needed in the browser.",
  );
  expect({ pageErrors, consoleErrors }).toEqual({ pageErrors: [], consoleErrors: [] });
});

import { expect, test } from "@playwright/test";

test("DNS Tester simulates Wi-Fi startup, BOOT reset, and orientation persistence", async ({ page }) => {
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
  const canvas = page.locator("#screen");
  await expect(canvas).toHaveAttribute("width", "320");
  await expect(canvas).toHaveAttribute("height", "240");

  const canvasBounds = await canvas.boundingBox();
  expect(canvasBounds).not.toBeNull();
  if (!canvasBounds) {
    return;
  }

  const screenPoint = (x, y) => [
    canvasBounds.x + canvasBounds.width * (x / 320),
    canvasBounds.y + canvasBounds.height * (y / 240),
  ];

  // A fresh browser context has no calibration flash record. Complete the
  // real four-target flow before exercising the Wi-Fi startup path.
  await page.waitForTimeout(500);
  for (const target of [[40, 40], [279, 40], [279, 199], [40, 199]]) {
    await page.mouse.click(...screenPoint(...target));
    await page.waitForTimeout(500);
  }
  await page.mouse.click(...screenPoint(160, 120));
  await page.waitForTimeout(500);

  // Startup includes the shared simulated captive-portal and connecting phases.
  await page.waitForTimeout(5_000);
  expect({ pageErrors, consoleErrors }).toEqual({ pageErrors: [], consoleErrors: [] });

  // Start a lookup and immediately press BOOT. The shared DNS loop must
  // finish or safely leave the lookup, then return through calibration rather
  // than dropping the BOOT action or spawning a second dashboard loop.
  await page.mouse.click(...screenPoint(160, 120));
  const activeLookupBootBounds = await page.locator("#boot-button").boundingBox();
  expect(activeLookupBootBounds).not.toBeNull();
  if (!activeLookupBootBounds) {
    return;
  }
  await page.mouse.move(
    activeLookupBootBounds.x + activeLookupBootBounds.width / 2,
    activeLookupBootBounds.y + activeLookupBootBounds.height / 2,
  );
  await page.mouse.down();
  await page.waitForTimeout(60);
  await page.mouse.up();
  await page.waitForTimeout(1_000);
  expect({ pageErrors, consoleErrors }).toEqual({ pageErrors: [], consoleErrors: [] });

  // BOOT during the lookup must have entered the same calibration path as a
  // main-state BOOT. Complete it before continuing with the settings checks.
  for (const target of [[40, 40], [279, 40], [279, 199], [40, 199]]) {
    await page.mouse.click(...screenPoint(...target));
    await page.waitForTimeout(500);
  }
  await page.mouse.click(...screenPoint(160, 120));
  await page.waitForTimeout(5_000);

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

  // BOOT cleared calibration before restarting, so complete the real flow
  // again before testing the dashboard controls.
  for (const target of [[40, 40], [279, 40], [279, 199], [40, 199]]) {
    await page.mouse.click(...screenPoint(...target));
    await page.waitForTimeout(500);
  }
  await page.mouse.click(...screenPoint(160, 120));
  await page.waitForTimeout(500);

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

  // CAL must enter the same calibration transition as BOOT, including after
  // the dashboard has been rotated into portrait presentation.
  const portraitCanvasBounds = await canvas.boundingBox();
  expect(portraitCanvasBounds).not.toBeNull();
  if (!portraitCanvasBounds) {
    return;
  }
  await page.mouse.click(
    portraitCanvasBounds.x + portraitCanvasBounds.width * (46 / 240),
    portraitCanvasBounds.y + portraitCanvasBounds.height * (294 / 320),
  );
  await page.waitForTimeout(500);
  expect({ pageErrors, consoleErrors }).toEqual({ pageErrors: [], consoleErrors: [] });
});

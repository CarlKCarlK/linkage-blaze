import { expect, test } from "@playwright/test";

const demos = [
  { slug: "ballet", orientation: "portrait" },
  { slug: "clock", orientation: "landscape" },
  { slug: "skeleton-clock", orientation: "portrait" },
  { slug: "armatron", orientation: "landscape" },
];

for (const demo of demos) {
  test(`${demo.slug} uses the shared CYD shell`, async ({ page }) => {
    await page.goto(`/demos/${demo.slug}/v4/`);

    const simulator = page.locator(".simulator");
    await expect(simulator).toHaveCount(1);
    await expect(simulator.locator(".stage")).toHaveCount(1);
    await expect(simulator.locator("#screen")).toHaveCount(1);
    await expect(simulator.locator("#boot-button")).toHaveCount(1);
    await expect(simulator.locator(".stage")).toHaveAttribute(
      "data-orientation",
      demo.orientation,
    );
    await expect(page.locator(".demo-ux-zoom-reset")).toHaveCount(1);

    expect(await page.locator(".simulator").count()).toBe(1);
    expect(await page.locator(".stage").count()).toBe(1);
    expect(await page.locator(".case").count()).toBe(1);
    expect(await page.locator(".cord").count()).toBe(1);
  });
}

test("the shared shell provides bounded wheel zoom and reset", async ({ page }) => {
  await page.goto("/demos/clock/v4/");

  const simulator = page.locator(".simulator");
  const reset = page.locator(".demo-ux-zoom-reset");
  await expect(reset).toBeHidden();

  await simulator.hover();
  await page.mouse.wheel(0, -1000);
  await expect(reset).toBeVisible();

  const zoomedTransform = await simulator.evaluate(
    (element) => element.style.transform,
  );
  expect(zoomedTransform).toMatch(/^scale\(/);

  await reset.click();
  await expect(reset).toBeHidden();
});

test("the shared details card opens, focuses, and closes with Escape", async ({ page }) => {
  await page.goto("/demos/clock/v4/");

  const cardButton = page.locator(".demo-ux-card-tag");
  const dialog = page.locator(".demo-ux-card-dialog");
  await cardButton.focus();
  await cardButton.click();
  await expect(dialog).toBeVisible();
  await expect(dialog).toBeFocused();
  await expect(dialog.locator("a", { hasText: "Gallery" })).toHaveAttribute(
    "href",
    "../../",
  );

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(cardButton).toBeFocused();
});

for (const slug of ["clock", "skeleton-clock"]) {
  test(`${slug} time control supports override and live reset`, async ({ page }) => {
    await page.goto(`/demos/${slug}/v4/`);

    const timeChip = page.locator(".demo-ux-time-chip").first();
    const timeRange = page.locator(".demo-ux-time-range");
    const liveButton = page.getByRole("button", { name: "Live" });
    await expect(timeChip).toHaveCount(1);
    await timeChip.click();
    await expect(timeRange).toBeVisible();

    await timeRange.fill("43200");
    await expect(timeChip).toContainText("12:00 PM");
    await liveButton.click();
    await expect(timeChip).toContainText("LIVE");
  });

  test(`${slug} routes main-state BOOT back through simulated Wi-Fi setup`, async ({ page }) => {
    const pageErrors = [];
    page.on("pageerror", (error) => pageErrors.push(String(error)));
    await page.goto(`/demos/${slug}/v4/`);
    await page.waitForTimeout(2200);

    const canvas = page.locator("#screen");
    const frameBeforeBoot = await canvas.evaluate((element) => element.toDataURL());
    const bootBounds = await page.locator("#boot-button").boundingBox();
    expect(bootBounds).not.toBeNull();
    if (!bootBounds) {
      return;
    }
    const bootCenterX = bootBounds.x + bootBounds.width / 2;
    const bootCenterY = bootBounds.y + bootBounds.height / 2;
    await page.mouse.move(bootCenterX, bootCenterY);
    await page.mouse.down();
    await page.waitForTimeout(60);
    await page.mouse.up();
    await page.waitForTimeout(300);

    expect(pageErrors).toEqual([]);
    const frameAfterBoot = await canvas.evaluate((element) => element.toDataURL());
    expect(frameAfterBoot).not.toEqual(frameBeforeBoot);
  });
}

test("Ballet routes BOOT back to the start of its animation", async ({ page }) => {
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(String(error)));
  await page.goto("/demos/ballet/v4/");
  await page.waitForTimeout(250);

  const canvas = page.locator("#screen");
  const frameBeforeBoot = await canvas.evaluate((element) => element.toDataURL());
  const bootBounds = await page.locator("#boot-button").boundingBox();
  expect(bootBounds).not.toBeNull();
  if (!bootBounds) {
    return;
  }
  const bootCenterX = bootBounds.x + bootBounds.width / 2;
  const bootCenterY = bootBounds.y + bootBounds.height / 2;
  await page.mouse.move(bootCenterX, bootCenterY);
  await page.mouse.down();
  await page.waitForTimeout(60);
  await page.mouse.up();
  await page.waitForTimeout(300);

  expect(pageErrors).toEqual([]);
  const frameAfterBoot = await canvas.evaluate((element) => element.toDataURL());
  expect(frameAfterBoot).not.toEqual(frameBeforeBoot);
});

test("Armatron forwards canvas and BOOT input to WASM", async ({ page }) => {
  const consoleErrors = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      consoleErrors.push(message.text());
    }
  });
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(String(error)));
  await page.goto("/demos/armatron/v4/");

  const canvas = page.locator("#screen");
  const canvasBounds = await canvas.boundingBox();
  expect(canvasBounds).not.toBeNull();
  if (!canvasBounds) {
    return;
  }

  await page.mouse.click(
    canvasBounds.x + canvasBounds.width * (202 / 320),
    canvasBounds.y + canvasBounds.height * (24 / 240),
  );
  await page.waitForTimeout(100);
  const frameBeforeBoot = await canvas.evaluate((element) => element.toDataURL());

  // Drive BOOT with real mouse events (not locator.dispatchEvent), which
  // carry a genuine active pointer. A synthetic dispatchEvent's pointerId
  // has no active pointer, so `setPointerCapture` throws and BOOT silently
  // never reaches the WASM handle - exactly the failure mode this test
  // must catch.
  const bootBounds = await page.locator("#boot-button").boundingBox();
  expect(bootBounds).not.toBeNull();
  if (!bootBounds) {
    return;
  }
  const bootCenterX = bootBounds.x + bootBounds.width / 2;
  const bootCenterY = bootBounds.y + bootBounds.height / 2;
  await page.mouse.move(bootCenterX, bootCenterY);
  await page.mouse.down();
  await page.waitForTimeout(50);
  await page.mouse.up();
  await page.waitForTimeout(300);

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);

  // BOOT requests calibration, which re-enters the shared four-tap
  // calibration exercise rather than stopping the app silently. That
  // exercise's crosshair screen is mostly static (unlike the game loop's
  // continuously ticking FPS counter), so confirm the app actually
  // transitioned by comparing against the pre-BOOT frame instead of
  // polling for continuous animation.
  const frameAfterBoot = await canvas.evaluate((element) => element.toDataURL());
  expect(frameAfterBoot).not.toEqual(frameBeforeBoot);
});

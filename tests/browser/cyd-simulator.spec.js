import { expect, test } from "@playwright/test";

const demos = [
  {
    slug: "ballet",
    orientation: "portrait",
    title: "Ballet",
    preview: "A motion-captured pirouette replayed as a linkage skeleton.",
    description: "A motion-captured pirouette converted into a linkage skeleton and replayed full screen.",
    controls: "Sit back and watch.",
    coreCodeUrl: "linkage-blaze/blob/main/crates/linkage-blaze-core/src/examples/ballet.rs",
  },
  {
    slug: "clock",
    orientation: "landscape",
    title: "Clock",
    preview: "An analog linkage clock with a digital strip and WiFi status.",
    description: "An analog clock whose hands are a tiny linkage posed by the time of day.",
    controls: "It follows your local clock. Use the shared time control to scrub to any time of day.",
    coreCodeUrl: "linkage-blaze/blob/main/crates/linkage-blaze-core/src/examples/clock.rs",
  },
  {
    slug: "skeleton-clock",
    orientation: "portrait",
    title: "Skeleton Clock",
    preview: "A motion-captured figure holds the hour and minute on placards.",
    description: "A clock told by a motion-captured figure whose placards show the hour and minute.",
    controls: "It follows your local clock. Use the shared time control to scrub to any time of day.",
    coreCodeUrl: "linkage-blaze/blob/main/crates/linkage-blaze-core/src/examples/skeleton_clock.rs",
  },
  {
    slug: "armatron",
    orientation: "landscape",
    title: "Armatron",
    preview: "A six-joint robot arm driven by inverse kinematics.",
    description: "A robot arm with six joints, modeled as a linkage and driven by inverse kinematics.",
    controls: "Drag the controls on the panel to pose the arm or run the solver.",
    coreCodeUrl: "linkage-blaze/blob/main/crates/linkage-blaze-core/src/examples/armatron/main.rs",
  },
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
    await expect(page.locator(".demo-ux-card-tag strong")).toHaveText(demo.title);
    await expect(page.locator(".demo-ux-card-tag__preview")).toHaveText(demo.preview);
    await page.locator(".demo-ux-card-tag").click();
    await expect(page.locator(".demo-ux-card-dialog h2")).toHaveText(demo.title);
    await expect(page.locator(".demo-ux-card-dialog")).toContainText(demo.description);
    await expect(page.locator(".demo-ux-card-dialog")).toContainText(demo.controls);
    await expect(page.locator(".demo-ux-card-dialog a", { hasText: "Core code" }))
      .toHaveAttribute("href", new RegExp(demo.coreCodeUrl.replaceAll("/", "\\/")));
    await page.keyboard.press("Escape");

    expect(await page.locator(".simulator").count()).toBe(1);
    expect(await page.locator(".stage").count()).toBe(1);
    expect(await page.locator(".case").count()).toBe(1);
    expect(await page.locator(".cord").count()).toBe(1);
  });
}

for (const slug of ["ballet", "armatron"]) {
  test(`${slug} does not expose the clock control`, async ({ page }) => {
    await page.goto(`/demos/${slug}/v4/`);
    await expect(page.locator(".demo-ux-time-chip")).toHaveCount(0);
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

test("full-screen mode keeps the shared BOOT input available", async ({ page }) => {
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(String(error)));
  await page.goto("/demos/ballet/v4/");

  const deviceModeButton = page.locator(".demo-ux-device-button");
  const overlay = page.locator(".demo-ux-device-overlay");
  await deviceModeButton.click();
  await expect(overlay).toBeVisible();

  const fullScreenBoot = overlay.locator("#boot-button");
  await expect(fullScreenBoot).toBeVisible();
  await fullScreenBoot.click();
  expect(pageErrors).toEqual([]);

  await overlay.locator(".demo-ux-device-close").click();
  await expect(overlay).toBeHidden();
  await expect(page.locator(".stage > #boot-button")).toBeVisible();
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
    await page.waitForTimeout(1_200);
    const noonFrame = await page.locator("#screen").evaluate((screen) => screen.toDataURL());
    await timeRange.fill("21600");
    await expect(timeChip).toContainText("6:00 AM");
    await page.waitForTimeout(1_200);
    const morningFrame = await page.locator("#screen").evaluate((screen) => screen.toDataURL());
    expect(morningFrame).not.toEqual(noonFrame);
    await liveButton.click();
    await expect(timeChip).toContainText("LIVE");
    await page.waitForTimeout(1_200);
    const liveFrame = await page.locator("#screen").evaluate((screen) => screen.toDataURL());
    expect(liveFrame).not.toEqual(morningFrame);
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

test("Armatron opens directly and maps calibration exits to the browser policy", async ({ page }) => {
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(String(error)));
  await page.goto("/demos/armatron/v4/");
  await page.waitForTimeout(300);

  const canvas = page.locator("#screen");
  const bootBounds = await page.locator("#boot-button").boundingBox();
  expect(bootBounds).not.toBeNull();
  if (!bootBounds) {
    return;
  }
  await page.mouse.move(
    bootBounds.x + bootBounds.width / 2,
    bootBounds.y + bootBounds.height / 2,
  );
  await page.mouse.down();
  await page.waitForTimeout(700);
  await page.mouse.up();
  await page.waitForTimeout(300);

  expect(pageErrors).toEqual([]);
  await expect(canvas).toHaveAttribute("width", "320");
  await expect(canvas).toHaveAttribute("height", "240");
  await expect(page.locator(".cyd-simulator-notice")).toContainText(
    "Calibration is not needed in the browser.",
  );
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
  // Hold long enough to exercise the release/debounce path, not just the
  // short-click path used by the other simulator examples.
  await page.waitForTimeout(700);
  await page.mouse.up();
  await page.waitForTimeout(300);

  expect(pageErrors).toEqual([]);
  expect(consoleErrors).toEqual([]);

  // BOOT requests calibration from the shared core; the browser policy
  // reports that calibration is unnecessary and restarts the app.
  const frameAfterBoot = await canvas.evaluate((element) => element.toDataURL());
  expect(frameAfterBoot).not.toEqual(frameBeforeBoot);
});

test("Armatron on-screen controls accept target and solver actions", async ({ page }) => {
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(String(error)));
  await page.goto("/demos/armatron/v4/");
  await page.waitForTimeout(250);

  const canvas = page.locator("#screen");
  const canvasBounds = await canvas.boundingBox();
  expect(canvasBounds).not.toBeNull();
  if (!canvasBounds) {
    return;
  }

  const screenPoint = async (x, y) => {
    await page.mouse.click(
      canvasBounds.x + canvasBounds.width * (x / 320),
      canvasBounds.y + canvasBounds.height * (y / 240),
    );
    await page.waitForTimeout(100);
  };
  const frame = async () => canvas.evaluate((element) => element.toDataURL());

  const initialFrame = await frame();
  await screenPoint(86, 24); // prev
  const previousTargetFrame = await frame();
  expect(previousTargetFrame).not.toEqual(initialFrame);

  await screenPoint(202, 24); // next
  const nextTargetFrame = await frame();
  expect(nextTargetFrame).not.toEqual(previousTargetFrame);

  await screenPoint(36, 95); // play
  await page.waitForTimeout(250);
  const playingFrame = await frame();
  expect(playingFrame).not.toEqual(nextTargetFrame);

  await screenPoint(36, 95); // stop
  await screenPoint(64, 95); // step
  const steppedFrame = await frame();
  expect(steppedFrame).not.toEqual(playingFrame);

  expect(pageErrors).toEqual([]);
});

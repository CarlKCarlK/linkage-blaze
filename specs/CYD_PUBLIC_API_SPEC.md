<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

# Spec: Simplify the Public CYD API

## Status

Proposed. Replaces the earlier `CydUncalibrated::into_calibrated` design.

This is a **breaking public API change** on a development branch. We change the
public abstraction rather than exposing more calibration machinery.

## Goal

The public CYD API should describe only devices an application can use directly:

- **`Cyd`** — a complete display-plus-touch device whose touch is already calibrated.
- **`CydDisplay`** — a display with no touch.

Applications must not construct, own, split, join, or operate an *uncalibrated*
touch device through the supported API. Calibration, raw-touch access,
shared-bus ownership, and display/touch assembly all become private
implementation details of the concrete platform constructors.

The target experience mirrors `WifiAuto`: just as `WifiAuto` owns the policy and
machinery to obtain usable local Wi-Fi, a concrete CYD constructor owns the
policy and machinery to obtain a usable, calibrated display-plus-touch device
and hands back a ready device or an error — nothing in between.

The WASM/browser simulator stops simulating calibration entirely and constructs
a ready-to-use simulated `Cyd` directly.

## Non-goals

- **Touch coordinate semantics.** The `TouchEvent` coordinate contract is
  preserved unchanged (see [Touch coordinate contract](#touch-coordinate-contract)).
  A separate change may revise it later.
- **A shared `construct_board_cyd` wrapper.** Each platform keeps its own
  concrete constructor so hardware topology stays visible at the call site.
- **A permanent compatibility surface.** No long-lived deprecated
  `CydParts`/touch/uncalibrated types. A short, hidden, `TODO0000`-marked
  internal shim is acceptable only to keep the workspace compiling mid-migration.
- **A separate internal support crate.** Private calibration/raw-touch support
  stays in `device-envoy-core` behind a hidden module for now; extracting a
  crate can be a later cleanup.

## The `Cyd` trait

### Before

```rust
pub trait Cyd: Sized {
    type Error;
    type Display: CydDisplay<Error = Self::Error>;
    type Touch: CydTouch<Error = Self::Error>;

    fn parts(&mut self) -> (&mut Self::Display, &mut Self::Touch);
    fn orientation(&mut self) -> Orientation { /* measures the display */ }
    fn display(&mut self) -> &mut Self::Display { self.parts().0 }
    fn touch(&mut self) -> &mut Self::Touch { self.parts().1 }
}
```

### After

```rust
pub trait Cyd: Sized {
    type Error;
    type Display: CydDisplay<Error = Self::Error>;

    /// Borrow the display for the duration of a draw.
    fn display(&mut self) -> &mut Self::Display;

    /// Read the next calibrated touch event, or `None` when there is no touch.
    fn read_touch(&mut self) -> Result<Option<TouchEvent>, Self::Error>;

    /// The logical orientation of this complete device.
    fn orientation(&self) -> Orientation;
}
```

A type implementing `Cyd` guarantees that:

- it owns or controls a display and a touch source;
- the touch source is calibrated;
- display and touch belong to the same logical device;
- any shared hardware resource stays valid for the device's lifetime;
- `read_touch` yields application-level `TouchEvent` values; and
- `display` returns the display those touch events are aligned to.

**`orientation` takes `&self`.** The current `&mut self` exists only so the
default implementation can borrow the display to measure it. With no `Touch`
associated type and no `parts`, there is no default implementation: each
implementer stores its final orientation and returns it by value. Storing the
orientation is required, not optional.

### Touch coordinate contract

`read_touch` inherits today's `CydTouch::read` contract verbatim:

- Returned points use **fixed landscape-panel coordinates (320x240)**,
  regardless of display orientation.
- Consumers that render an oriented screen must apply
  `Orientation::map_landscape_point` **exactly once** before hit testing.
- `Ok(None)` means no pending touch; `Err` means a hardware/read failure.

Both the shared-application refactor and the WASM pointer-event mapping must
uphold this contract. In particular, WASM input normalization must produce
landscape-panel coordinates, not pre-oriented coordinates.

## Public surface changes

### Kept public

- `CydDisplay` — unchanged drawing API; also the display half of a `Cyd`.
- Standalone display-only types where they exist and are useful:
  `CydDisplayEsp`, `CydDisplayRp`, `CydDisplayWasm`, `CydDisplayMemory`.
- `TouchEvent` — applications consume it.
- Concrete calibrated devices implementing `Cyd`:
  `CydEsp`, `CydEspOneSpi`, `CydRp`, `CydRpOneSpi`, `CydWasm`, `CydMemory`.

### Removed from the public API

| Item | Reason |
| --- | --- |
| `Cyd::Touch`, `Cyd::touch`, `Cyd::parts` | Touch is no longer independently borrowable; use `read_touch`. |
| `CydParts`, `into_parts`, `from_parts` | A complete device must not be split or reassembled from parts. |
| `CydTouch`, `CydTouchUncalibrated`, `CydUncalibrated` | Touch and uncalibrated bundles are not application concepts. |

### Hidden (may remain internal during and after migration)

| Item | Notes |
| --- | --- |
| `CydTouchEsp`, `CydTouchRp`, and WASM/memory touch-only types | Internal only. |
| `CydTouchUncalibratedEsp`, `CydTouchUncalibratedRp`, equivalents | Internal only. |
| `CydEspUncalibrated`, `CydRpUncalibrated`, `CydEspOneSpiUncalibrated`, `CydRpOneSpiUncalibrated` | A platform may keep a *private* uncalibrated bundle for type-state and atomic ownership during construction; applications never name or receive it. |
| `CalibrationConfig`, `EnsureCalibrationOutcome`, `EnsureCalibrationSettings` | Calibration data/outcomes. |
| `ensure_calibration`, `ensure_calibration_with_settings` | Orchestration. |
| `display_orientation_for_calibration`, `orientation_after_calibration` | Orientation-during-calibration policy. |

Where cross-crate visibility is still technically required, expose these behind
a clearly unsupported boundary:

```rust
#[doc(hidden)]
pub mod __private {
    // Raw-touch and calibration implementation support. Not a supported API.
}
```

## Constructor contract

Every public complete-CYD constructor returns exactly one of:

- `Err(error)` — a failure serious enough to abort construction; or
- `Ok(cyd)` — a device ready for application use.

There is no public intermediate result. Constructors must not return
`Option<Cyd>`, `EnsureCalibrationOutcome`, a `RestartRequired`-style value, an
uncalibrated CYD, or a separate display/touch pair.

The error type is the crate's `Result` alias, e.g. `crate::Result<CydEsp>`.

### Hardware constructors own the full setup policy

A hardware constructor:

1. constructs display and raw-touch resources;
2. preserves atomic shared-bus ownership where required (one-SPI);
3. loads a saved calibration when one is available;
4. runs interactive calibration when none is available;
5. retries recoverable calibration attempts internally;
6. saves a validated calibration;
7. puts display and touch into the requested final orientation; and
8. returns the complete calibrated `Cyd`.

The caller supplies hardware resources, display style/policy, calibration
persistence, and any physical control (button) used by calibration. The caller
does **not** direct individual calibration states.

### Two-SPI ESP target

```rust
pub async fn new(/* hardware and policy */) -> crate::Result<CydEsp>;
```

The DNS tester calls it directly (exact parameter ordering stays
platform-specific):

```rust
let mut cyd = CydEsp::new(
    &CYD_STATIC,
    // Display SPI.
    p.SPI2, p.GPIO14, p.GPIO13, p.GPIO12, p.GPIO15, p.GPIO2, p.GPIO4, p.GPIO21,
    DEFAULT_DISPLAY_SPI_HZ,
    orientation,
    embedded_graphics::pixelcolor::Rgb888::new(10, 10, 12),   // near-black
    embedded_graphics::pixelcolor::Rgb888::new(230, 230, 230), // near-white
    &DEFAULT_FONT,
    // Touch SPI.
    p.SPI3, p.GPIO25, p.GPIO32, p.GPIO39, p.GPIO33, p.GPIO36,
    &mut calibration_flash_block,
    &mut *button,
)
.await?;
```

### One-SPI constructors

`CydEspOneSpi::new` and `CydRpOneSpi::new` follow the same contract and return
`crate::Result<Self>`. Internally they create the shared bus, display handle,
and raw-touch handle as one atomic device setup. They may use a private
uncalibrated bundle, but neither the bundle nor its parts are ever returned.

## Calibration policy

### Recoverable problems retry internally

Conditions where another attempt is reasonable stay inside the constructor and
must not surface as public errors or restart outcomes: degenerate calibration
geometry, residual error above the accepted limit, a missed verification
target, verification timeout, and other rejected attempts.

### Unrecoverable errors return `Err`

Return `Err` only when construction cannot reasonably continue: display init or
communication failure, raw-touch init or communication failure, inability to
persist a validated calibration, or another platform failure preventing a ready
`Cyd`.

### Calibration owns its own messages

Calibration knows its own instructions, rejection messages, verification
prompts, and completion messages. Remove caller-supplied message parameters
such as `Some("Touch calibrated")` and `confirmed_message: Option<&str>`.

### No calibration-specific caller reset

A fresh calibration must produce the same public result as a loaded one:
`Ok(cyd)`. The caller must not inspect whether calibration was loaded or saved,
wait for a calibration-button release, reset after saving, reconstruct display
or touch, or retry calibration itself. If the current implementation resets only
to switch from a temporary calibration orientation to the requested application
orientation, fix that below the constructor boundary. Do not expose a restart
result merely to preserve the current implementation.

## ESP DNS tester `inner_main`

The calibration-related portion becomes conceptually:

```rust
let [
    wifi_auto_flash_block,
    mut calibration_flash_block,
    mut orientation_flash_block,
] = FlashBlockEsp::new_array::<3>(p.FLASH)?;

let orientation = orientation_flash_block
    .load::<Orientation>()?
    .unwrap_or(Orientation::Landscape);

let button =
    DnsTesterButtonWatch::new(p.GPIO0, PressedTo::Ground, spawner).await?;

static CYD_STATIC: CydStaticEsp<STATUS_PIXEL_COUNT> = CydEsp::new_static();

let mut cyd = CydEsp::new(
    /* visible two-SPI hardware wiring */,
    orientation,
    /* display style */,
    &mut calibration_flash_block,
    &mut *button,
)
.await?;

info!("CYD initialized");
```

Remove from `inner_main`: `CalibrationConfig` flash probing,
`calibration_is_available`, `display_orientation_for_calibration`,
`CydEspUncalibrated`, `ensure_calibration`, `EnsureCalibrationOutcome`,
`was_saved`, calibration-specific button polling, calibration-specific software
reset, `CydEsp::from_parts`, and `CydParts` imports.

## Shared application changes

The old pattern borrows separate display and touch for a long time and must go:

```rust
// Old — remove.
let (display, touch) = cyd.parts();
let mut ui = Ui::new(display, ...);
```

Instead, ask the complete device for touch events and borrow the display only
while drawing:

```rust
let touch_event = cyd.read_touch()?;
let display = cyd.display();
```

### Refactor `Ui`

`Ui` must not permanently own `&mut Display`. It owns only application/UI state
(layout, bitmap selection, cached values, text state, redraw state). Drawing
methods receive a display borrow for the duration of the draw:

```rust
let mut ui = Ui::<16>::new(layout.bitmap);

loop {
    let touch_event = cyd.read_touch()?;
    ui.begin(touch_event, orientation);
    ui.status(cyd.display(), layout.status, status).await?;
}
```

The exact API may differ, but no shared application should need independently
owned or simultaneously long-lived display and touch references.

## WASM simulator

The simulator represents an already calibrated device. It no longer
demonstrates, tests, persists, or simulates calibration.

Remove from the browser startup path: calibration flash load/save, calibration
flow screens, synthetic raw calibration coordinates, simulated four-point
calibration, verification-target handling, calibration completion messages,
calibration outcomes, calibration-triggered reconstruction or reset, and any
startup delay used only to make calibration visible.

```rust
let mut cyd = CydWasm::new(
    /* canvas/simulator resources */,
    orientation,
    /* display style */,
)?;
```

`CydWasm` implements `Cyd::display`, `Cyd::read_touch`, and `Cyd::orientation`,
and exposes no public WASM touch object. Browser pointer events enter as
application-level `TouchEvent` values without passing through the raw-touch
calibration driver.

**Input normalization is not calibration.** The simulator may translate browser
coordinates into the fixed landscape-panel `TouchEvent` convention (see [Touch
coordinate contract](#touch-coordinate-contract)). Do not model this as
`CalibrationConfig` or route it through the calibration flow.

**Recalibration requests.** The WASM launcher must never enter a calibration
flow. Where a shared application can emit a `Calibrate` platform request, WASM
chooses a non-calibration policy: restart/recreate the already calibrated
simulator, or log that calibration is not simulated and continue. Prefer
preventing the simulator controls from generating the request at all. A later
cleanup may rename the shared request to separate hardware calibration from
generic application reset.

## Internal implementation structure

Private layers may remain: `RawTouchEsp`, `RawTouchRp`, internal uncalibrated
bundles, the internal calibration driver, and the `CalibrationConfig`
persistence format. They implement public constructors; they do not support
application composition. The intended internal flow:

```text
platform-specific atomic resource construction
  -> private display + private raw-touch bundle
  -> internal calibration policy
  -> public calibrated Cyd
```

One-SPI devices must continue to work without splitting the shared bus.

## Templates and generated examples

Search templates and generator inputs (not just generated files) for:
`CydTouch`, `CydTouchUncalibrated`, `CydParts`, `parts()`, `touch()`,
`into_parts`, `from_parts`, `ensure_calibration`, `EnsureCalibrationOutcome`,
`CydEspUncalibrated`, `CydRpUncalibrated`, and manual calibration flash probing.
Migrate anything that must compile against the new traits.

Where a launcher still uses a transitional support path, add one targeted
comment to the **template** (not scattered across a single logical startup
sequence, and not only in generated files):

```rust
// TODO0000 Simplify this startup around the calibrated-only public Cyd API.
```

Note the convention: this repo uses uppercase `TODO0000` for release-blocking
items, not `todo0000`.

## Migration phases

1. **Core model.** Reshape `Cyd` to `display` / `read_touch` / `orientation`;
   drop `Touch`, `parts`, `touch`; remove `CydParts`; move raw-touch and
   calibration traits behind `__private`; keep `TouchEvent` public.
2. **Implementations.** Make ESP two-SPI, ESP one-SPI, RP two-SPI, RP one-SPI,
   memory, and WASM implement the new `Cyd` directly.
3. **Ready-device constructors.** Move calibration policy below constructors;
   remove outcomes and caller confirmation messages; drop calibration-specific
   resets; preserve atomic one-SPI construction.
4. **WASM.** Delete simulated calibration; feed normalized pointer events
   straight into `CydWasm`; drop simulated persistence and reconstruction.
5. **Shared apps.** Replace `parts()` with `read_touch`; refactor `Ui` off a
   long-lived display borrow; update examples and tests.
6. **DNS tester.** Replace manual orchestration with `CydEsp::new(...).await?`;
   clean imports; regenerate board examples; add `TODO0000` template comments
   where not fully simplified.
7. **Cleanup.** Remove obsolete public exports and any temporary shim; ensure
   docs show only the intended public model.

## Tests

**Core `Cyd`.**

1. A `Cyd` exposes its display.
2. `read_touch` returns calibrated `TouchEvent` values in landscape-panel
   coordinates.
3. No public API is required to obtain a separate touch object.
4. An application can alternate `read_touch` and `display` borrows safely.

**Hardware-constructor policy.**

1. A valid saved calibration yields `Ok(cyd)`.
2. Missing or corrupt calibration enters the internal calibration flow.
3. Recoverable rejected attempts retry internally.
4. A validated fresh calibration is saved.
5. A fresh calibration yields `Ok(cyd)` with no caller reset handling.
6. Fatal device or persistence failures return `Err`.
7. The returned device uses the requested final orientation.

**One-SPI ownership.**

1. Shared display and touch handles are created atomically.
2. The public type cannot be split.
3. `display` and `read_touch` still arbitrate the same shared bus correctly.

**WASM.**

1. `CydWasm::new` returns a ready device with no calibration state.
2. Startup never shows calibration UI.
3. Browser pointer input becomes `TouchEvent` values directly.
4. No calibration data is loaded or saved.
5. A recalibration request does not enter a calibration flow.
6. Post-construction behavior matches hardware.

**Public-surface checks.** Documentation/compile tests confirm normal user code
neither needs nor sees `CydTouch`, `CydTouchUncalibrated`, `CydParts`, public
uncalibrated bundles, or `EnsureCalibrationOutcome`.

## Acceptance criteria

- `Cyd` has no associated touch type and no `parts`/`touch`; it exposes
  `display`, `read_touch` (calibrated), and `orientation(&self)`.
- `CydParts`, the public touch traits, and public uncalibrated bundle types are
  removed; touch-only and uncalibrated concrete types are hidden or removed.
- Public complete-CYD constructors return only `crate::Result<Self>`.
- Hardware callers handle no calibration outcomes, supply no completion
  messages, and perform no calibration-specific reset.
- One-SPI devices stay atomically owned and cannot be split.
- The ESP two-SPI DNS tester calls `CydEsp::new(...).await?` and gets a ready
  device.
- Shared applications use `read_touch`; UI code holds no long-lived mutable
  display borrow while reading touch.
- `CydWasm` starts ready and never loads, saves, displays, verifies, or
  restarts for calibration.
- Templates are updated before generated files are regenerated; any transitional
  code is hidden and marked `TODO0000`.
- `just check-all` passes (and Device Envoy's `cargo check-all` where the work
  touches that repo).

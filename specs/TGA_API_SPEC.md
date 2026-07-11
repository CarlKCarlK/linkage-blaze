# TGA API Specification

<!-- todo0 consider deleting this spec once the work below is implemented and released. -->

This specification records the current compile-time TGA API and the intended
replacement API. The implementation lives in the linked `device-envoy-core`
library, under `cyd::display`.

This API is not a compatibility boundary. The redesign may rename, remove, or
reshape existing types and macros directly; no deprecated wrappers, aliases, or
transitional compatibility layer is required.

## Goals

The TGA API should:

- decode embedded TGA files at compile time;
- remain `no_std`, allocation-free, and suitable for embedded targets;
- preserve source color and alpha information until an output representation is
  selected;
- make RGB565 a conversion target rather than the fundamental TGA format;
- represent binary visibility masks separately from image pixels;
- support owned compile-time assets and borrowed views without copying pixels;
- fail at compile time for unsupported or malformed input.

Partial-alpha rendering is not part of the mask API. A mask is strictly binary:
each pixel is either visible or not visible.

## Current API

### Input format

The decoder currently accepts this subset of TGA:

- uncompressed true-color images (image type `2`);
- no color map;
- 24-bit BGR and 32-bit BGRA pixels;
- top-left and bottom-left image origins;
- no RLE, palettes, grayscale, or right-to-left origin;
- dimensions matching the const-generic destination type.

The current decoder converts source pixels directly to RGB565 while decoding.
For 24-bit input, pixels are opaque. For 32-bit input, alpha can be used to
populate the binary mask, but it is not retained as a continuous alpha value.

### Image storage

```rust
Image565Fixed<W, H, N>
```

Owns `N` RGB565 pixels in a `[u16; N]`. `N` must equal `W * H`. This is the
primary type for compile-time image assets.

```rust
Image565View
```

Borrows RGB565 pixels with runtime dimensions. It supports full-image and
cropped views of fixed images and is used by the contiguous pixel renderer.

```rust
Image565Mask<W, H, N, MASK_N>
```

Owns both RGB565 pixels and a packed one-bit opacity mask:

```rust
pub pixels: [u16; N]
pub opaque: [u8; MASK_N]
```

It implements `Drawable`, drawing only pixels whose mask bit is set. It also
provides `is_opaque`, `at`, and direct access to both arrays. There is no
general `Image565MaskView`; `PlacedImage565Mask` is only a positioned drawable
wrapper.

### Constructors and macros

The current public convenience macros are:

```rust
tga565!(path)
tga565_mask!(path)
tga565_magenta_mask!(path)
tga565_white_mask!(path)
```

They infer dimensions from an expected destination type. Dimension-explicit
forms also exist for the mask macros and compute the pixel and packed-mask
storage sizes.

The current conversion methods are:

```rust
Image565Fixed::from_tga(bytes)
Image565Mask::from_tga(bytes)
Image565Mask::from_tga_magenta(bytes)
Image565Mask::from_tga_white(bytes)
```

`mask_byte_count(width, height)` is available as a `const fn` and computes the
number of bytes required for a one-bit mask, including the image dimensions in
the call site rather than requiring a repeated pixel-count expression.

### Current limitations

The current design:

- couples TGA decoding directly to RGB565;
- combines image pixels and mask pixels in `Image565Mask`;
- has no standalone binary mask type;
- has no mask view or masked contiguous-pixel path;
- exposes color-key behavior through separate macro and constructor names;
- does not preserve source RGBA data for later conversions;
- cannot represent partial alpha as a separate output type.

## Ideal API

### Generic decoded TGA image

Introduce a compile-time decoded source type:

```rust
TgaImageFixed<W, H, N>
```

It should own canonical RGBA8888 pixels, regardless of whether the source was
24-bit BGR or 32-bit BGRA. A 24-bit source receives alpha `255`; a 32-bit
source preserves its alpha byte.

The generic macro should be the only TGA reader macro. It should support both
type-directed and explicitly dimensioned forms:

```rust
const SOURCE: TgaImageFixed<45, 73, { 45 * 73 }> =
    tga!("hours.small.tga");

let source = tga!("hours.small.tga", 45, 73);
```

The no-dimension form is preferred when an expected type supplies `W`, `H`,
and `N`. The explicit-dimension form is required for an unannotated `let`,
because a declarative macro cannot infer const-generic dimensions from the TGA
header while expanding `include_bytes!`.

The source type may be an intermediate compile-time value. The API must not
require a runtime allocation or retain an unnecessary source buffer in the
final embedded image.

### Explicit output conversions

The decoded source should provide concise conversion methods:

```rust
const IMAGE: Image565Fixed<45, 73, { 45 * 73 }> =
    tga!("hours.small.tga").to_565();

const MASK: MaskFixed<45, 73, { mask_byte_count(45, 73) }> =
    tga!("hours.small.tga").to_mask_magenta();
```

The preferred user-facing vocabulary is:

- `tga!` — decode a generic TGA source;
- `to_565()` — convert pixels to RGB565;
- `to_mask_magenta()` — derive a standalone binary magenta color-key mask.

For now, the only supported mask conversion is the magenta color key. Do not
add alpha-, white-, or arbitrary-color mask policies yet.

Example:

```rust
tga!("image.tga").to_mask_magenta()
```

`to_mask_magenta` must always return a binary mask. The magenta policy should preserve
the current threshold behavior, including the anti-aliased fringe. Source
alpha may remain preserved in `TgaImageFixed`, but it is not used by the first
mask conversion.

### Separate image and mask storage

Replace the combined `Image565Mask` storage model with independent types:

```rust
Image565Fixed<W, H, N>
MaskFixed<W, H, MASK_N>
```

`MaskFixed` owns only packed visibility bits. It should provide:

- `is_set(index)` or equivalent;
- positioned drawing/compositing support;
- direct binary-mask drawing/compositing support;
- no color or alpha channel.

Do not add `MaskView` initially. The current consumers use fixed-size masks as
complete sign assets, drawn at different positions, so a fixed mask is enough.
Add `MaskView` only if a concrete consumer needs a cropped mask, a
runtime-sized mask, or a borrowed subregion. Contiguous compositing alone does
not require a view; it can consume a `MaskFixed` with coordinates.

Image/mask composition should be explicit at draw time, for example:

```rust
image.at(top_left).draw_masked(&mask, target)?;
```

The composition wrapper may be an ephemeral positioned view; it should not
require combining the image and mask into a new owned allocation.

### Contiguous rendering

Opaque `Image565Fixed` assets should continue to support the existing bulk
`copy_from_565` path.

Masked images should support an SPI-optimized run-based stream. For each image
row, the renderer can scan the binary mask into horizontal opaque spans, then:

1. set the display address window to one span;
2. stream that span's RGB565 pixels contiguously;
3. skip transparent gaps without transmitting their pixels.

An entirely opaque row becomes one bulk transfer. A sparse row becomes several
short transfers, so the implementation should account for address-window
command overhead and may use a fallback for masks with many tiny spans.

The generic fallback should leave destination pixels unchanged where the mask
bit is clear. It must not treat a binary mask as alpha blending.

This is contiguous streaming of opaque runs, not one uninterrupted stream for
the whole masked image: transparent gaps necessarily require either skipped
address ranges or separate display windows.

The first implementation may use the ordinary `DrawTarget` path. A masked
contiguous path should be added only when the renderer can preserve the
performance benefit without copying or expanding the mask unnecessarily.

## Migration plan

1. Add `TgaImageFixed` and `tga!`, retaining the current accepted TGA subset.
2. Move header parsing and orientation handling to the generic source decoder.
3. Add `to_565()` and `to_mask_magenta()` as `const` conversions.
4. Add `MaskFixed`; defer `MaskView` until a concrete cropping or
   runtime-sized use case requires it.
5. Implement explicit image-plus-mask drawing/compositing, including an SPI
   path that streams opaque mask runs without transmitting transparent pixels.
6. Replace all specialized TGA macros with `tga!` and direct conversions.
7. Replace `Image565Mask` with separate image and mask values.
8. Migrate examples and tests in one direct redesign.

## Non-goals

This work does not initially include:

- RLE, palettes, grayscale, or other unsupported TGA variants;
- runtime image loading;
- heap allocation;
- partial-alpha compositing;
- a general-purpose image-processing framework.

## Open decisions

- Whether the canonical source pixel should be a public `Rgba8888` type or an
  internal fixed byte representation.
- Whether the method should be named `to_565()` or the more explicit
  `to_rgb565()`.
- Whether repeated `tga!(...).to_565()` and `tga!(...).to_mask_magenta()` expressions
  should be optimized through a shared source constant in examples.

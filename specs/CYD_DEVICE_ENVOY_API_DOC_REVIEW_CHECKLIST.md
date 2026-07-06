<!-- todo0 consider deleting this spec once the CYD device-envoy API docs have been reviewed. -->

# Carl's todo list

* Look at the top of core for cyd-related exports that shouldn't be there

/home/carlk/programs/mcu/device-envoy/target/doc/device_envoy_core/index.html

* Esp
* Rp

# CYD Device-Envoy Docs Review Links

Run these commands from:

```bash
cd /home/carlk/programs/mcu/device-envoy
```

## Generate Docs

Core docs:

```bash
just show-docs-core
```

ESP docs:

```bash
just show-docs-esp
```

RP docs:

```bash
just show-docs-rp
```

Fast rebuild for ESP and RP docs:

```bash
just update-docs-fast
```

## Core Pages

Core crate root:

/home/carlk/programs/mcu/device-envoy/target/doc/device_envoy_core/index.html

Core CYD module:

/home/carlk/programs/mcu/device-envoy/target/doc/device_envoy_core/cyd/index.html

`Cyd`:

/home/carlk/programs/mcu/device-envoy/target/doc/device_envoy_core/cyd/trait.Cyd.html

`CydDisplay`:

/home/carlk/programs/mcu/device-envoy/target/doc/device_envoy_core/cyd/trait.CydDisplay.html

`CydTouch`:

/home/carlk/programs/mcu/device-envoy/target/doc/device_envoy_core/cyd/trait.CydTouch.html

`CydFrame`:

/home/carlk/programs/mcu/device-envoy/target/doc/device_envoy_core/cyd/trait.CydFrame.html

`DrawItem`:

/home/carlk/programs/mcu/device-envoy/target/doc/device_envoy_core/cyd/enum.DrawItem.html

`ContiguousPixels`:

/home/carlk/programs/mcu/device-envoy/target/doc/device_envoy_core/cyd/struct.ContiguousPixels.html

`Image565View`:

/home/carlk/programs/mcu/device-envoy/target/doc/device_envoy_core/cyd/struct.Image565View.html

## ESP Pages

ESP crate root:

/home/carlk/programs/mcu/device-envoy/target/riscv32imac-unknown-none-elf/doc/device_envoy_esp/index.html

ESP CYD module:

/home/carlk/programs/mcu/device-envoy/target/riscv32imac-unknown-none-elf/doc/device_envoy_esp/cyd/index.html

`CydEsp`:

/home/carlk/programs/mcu/device-envoy/target/riscv32imac-unknown-none-elf/doc/device_envoy_esp/cyd/struct.CydEsp.html

`PixelBuffer`:

/home/carlk/programs/mcu/device-envoy/target/riscv32imac-unknown-none-elf/doc/device_envoy_esp/cyd/struct.PixelBuffer.html

## RP Pages

RP crate root:

/home/carlk/programs/mcu/device-envoy/target/thumbv8m.main-none-eabihf/doc/device_envoy_rp/index.html

RP CYD module:

/home/carlk/programs/mcu/device-envoy/target/thumbv8m.main-none-eabihf/doc/device_envoy_rp/cyd/index.html

`CydRp`:

/home/carlk/programs/mcu/device-envoy/target/thumbv8m.main-none-eabihf/doc/device_envoy_rp/cyd/struct.CydRp.html

`PixelBuffer`:

/home/carlk/programs/mcu/device-envoy/target/thumbv8m.main-none-eabihf/doc/device_envoy_rp/cyd/struct.PixelBuffer.html

## Suggested Reading Order

1. Core crate root
2. Core CYD module
3. Core CYD traits and types
4. ESP crate root
5. ESP CYD module and `CydEsp`
6. RP crate root
7. RP CYD module and `CydRp`

## Suggested Commit Message

```text
add CYD docs review links
```

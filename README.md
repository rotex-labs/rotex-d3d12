# rotex-d3d12

Direct3D 12 on Windows. Other platforms return unsupported.

> [!WARNING]
> **Beta, pre-1.0.** Rotex is under active development. APIs, behavior, and crate
> boundaries can change in any 0.x release. Do not depend on it for production work yet.

## Overview

`rotex-d3d12` implements `GpuPlatform` and `GpuBackend` with the same `rotex-types` inputs as the other backends. The translator is compiled only on Windows. Elsewhere the crate still builds, and device creation fails.

## Responsibilities

- `D3d12GpuPlatform` and `D3d12Bridge`
- Windows resource creation, barriers, and command translation
- An unsupported stub so non-Windows workspace checks still compile

## Dependencies

`rotex-core` and `rotex-types`. Tests use `rotex-conformance`.

## Status

Version 0.2.0. Off Windows every core capability is absent and `GpuPlatform::new` returns unsupported. The Windows translator is not built by a Linux `cargo check`.

## Part of Rotex

Rotex is a graphics HAL workspace. See [rotex-dev](https://github.com/rotex-labs/rotex-dev) for the architecture and the other crates.

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
* MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

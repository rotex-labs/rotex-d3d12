<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/rotex-labs/rotex-assets/55fa2474cb788a445c33b4ccf615eb8bbb9dbe64/brand/docs/crate-headers/dark/rotex-d3d12-header.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/rotex-labs/rotex-assets/55fa2474cb788a445c33b4ccf615eb8bbb9dbe64/brand/docs/crate-headers/light/rotex-d3d12-header.svg">
    <img alt="rotex-d3d12" src="https://raw.githubusercontent.com/rotex-labs/rotex-assets/55fa2474cb788a445c33b4ccf615eb8bbb9dbe64/brand/docs/crate-headers/light/rotex-d3d12-header.svg" width="1200">
  </picture>
</p>

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

Version 0.3.0. Active on Windows: device, swapchain, compute, graphics, resize, and shutdown run the shared command list, and `PlatformDescriptor.debug` turns on the debug layer. Off Windows every core capability is absent and `GpuPlatform::new` returns unsupported.

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
* MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

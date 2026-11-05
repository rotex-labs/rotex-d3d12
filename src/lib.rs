#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

//! Direct3D 12 backend.
//!
//! [`D3d12GpuPlatform`] and [`D3d12Bridge`] accept the same platform and command
//! inputs as the other backends. The translator is compiled on Windows. On
//! other targets the crate still builds, and creating a device returns
//! unsupported.

mod caps;
mod gpu_platform;
mod recorder;

#[cfg(windows)]
mod native;

pub use gpu_platform::D3d12GpuPlatform;
pub use recorder::D3d12Bridge;

#[cfg(test)]
mod conformance_test {
    use super::D3d12GpuPlatform;
    use rotex_core::GpuPlatform;
    use rotex_types::{DeviceDescriptor, PlatformDescriptor};

    #[test]
    fn backend_matches_reported_capabilities() {
        pollster::block_on(async {
            let gpu = match D3d12GpuPlatform::new(PlatformDescriptor::default()).await {
                Ok(gpu) => gpu,
                Err(_) => return,
            };
            let adapters = match gpu.enumerate_adapters().await {
                Ok(adapters) => adapters,
                Err(_) => return,
            };
            let Some(adapter) = adapters.first() else {
                return;
            };
            let mut backend = match gpu
                .create_backend(adapter.id, DeviceDescriptor::default())
                .await
            {
                Ok(backend) => backend,
                Err(_) => return,
            };
            rotex_conformance::run(&mut backend).expect("conformance");
        });
    }
}

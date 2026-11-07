use async_trait::async_trait;
use rotex_core::{Error, GpuPlatform};
use rotex_types::{AdapterId, DeviceDescriptor, PlatformDescriptor};

use crate::recorder::D3d12Bridge;

/// Direct3D 12 implementation of [`GpuPlatform`](rotex_core::GpuPlatform).
///
/// On non-Windows targets [`GpuPlatform::new`](rotex_core::GpuPlatform::new)
/// returns unsupported.
pub struct D3d12GpuPlatform {
    #[cfg(windows)]
    adapter: crate::native::Adapter,
    #[cfg(windows)]
    debug: bool,
}

#[async_trait]
impl GpuPlatform for D3d12GpuPlatform {
    type Backend = D3d12Bridge;

    async fn new(platform_descriptor: PlatformDescriptor) -> Result<Self, Error> {
        #[cfg(windows)]
        {
            let debug = platform_descriptor.debug == rotex_types::DebugMode::Validation;
            if debug {
                crate::native::enable_debug_layer();
            }
            Ok(Self {
                adapter: crate::native::Adapter::first()?,
                debug,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = platform_descriptor;
            Err(Error::unsupported("d3d12 is only available on windows"))
        }
    }

    async fn enumerate_adapters(&self) -> Result<Vec<rotex_types::AdapterInfo>, Error> {
        #[cfg(windows)]
        {
            Ok(vec![self.adapter.info()])
        }
        #[cfg(not(windows))]
        {
            Err(Error::unsupported("d3d12 is only available on windows"))
        }
    }

    async fn create_backend(
        self,
        adapter_id: AdapterId,
        device_descriptor: DeviceDescriptor,
    ) -> Result<Self::Backend, Error> {
        #[cfg(windows)]
        {
            if adapter_id != self.adapter.id() {
                return Err(Error::unsupported("d3d12 adapter id is out of range"));
            }
            let _ = device_descriptor;
            Ok(D3d12Bridge::from_device(crate::native::Device::open(
                &self.adapter,
                self.debug,
            )?))
        }
        #[cfg(not(windows))]
        {
            let _ = (adapter_id, device_descriptor);
            Err(Error::unsupported("d3d12 is only available on windows"))
        }
    }
}

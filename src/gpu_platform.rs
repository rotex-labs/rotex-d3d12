use async_trait::async_trait;
use rotex_core::{Error, GpuPlatform};
use rotex_types::{AdapterId, DeviceDescriptor, PlatformDescriptor};

use crate::recorder::D3d12Bridge;

pub struct D3d12GpuPlatform {
    #[cfg(windows)]
    adapter: crate::native::Adapter,
}

#[async_trait]
impl GpuPlatform for D3d12GpuPlatform {
    type Backend = D3d12Bridge;

    async fn new(platform_descriptor: PlatformDescriptor) -> Result<Self, Error> {
        #[cfg(windows)]
        {
            let _ = platform_descriptor;
            Ok(Self {
                adapter: crate::native::Adapter::first()?,
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
            )?))
        }
        #[cfg(not(windows))]
        {
            let _ = (adapter_id, device_descriptor);
            Err(Error::unsupported("d3d12 is only available on windows"))
        }
    }
}

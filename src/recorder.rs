use rotex_core::{Error, GpuBackend};
use rotex_types::{
    BackendCapabilities, CreatedResources, Extent2D, ResourceBatchCreate, ResourceBatchUpdate,
    RhiCommand, SurfaceDescriptor, TextureId, TextureReadback,
};

#[cfg(windows)]
use crate::caps;

pub struct D3d12Bridge {
    capabilities: BackendCapabilities,
    #[cfg(windows)]
    device: crate::native::Device,
}

impl D3d12Bridge {
    #[cfg(windows)]
    pub(crate) fn from_device(device: crate::native::Device) -> Self {
        Self {
            capabilities: caps::capabilities(),
            device,
        }
    }
}

impl GpuBackend for D3d12Bridge {
    fn capabilities(&self) -> BackendCapabilities {
        self.capabilities
    }

    fn attach_surface(&mut self, surface_descriptor: SurfaceDescriptor) -> Result<(), Error> {
        #[cfg(windows)]
        {
            self.device.attach_surface(surface_descriptor)
        }
        #[cfg(not(windows))]
        {
            let _ = surface_descriptor;
            Err(Error::unsupported("d3d12 is only available on windows"))
        }
    }

    fn create_resources(
        &mut self,
        descriptor: ResourceBatchCreate,
    ) -> Result<CreatedResources, Error> {
        #[cfg(windows)]
        {
            self.device.create_resources(descriptor)
        }
        #[cfg(not(windows))]
        {
            let _ = descriptor;
            Err(Error::unsupported("d3d12 is only available on windows"))
        }
    }

    fn update_resources(&mut self, descriptor: ResourceBatchUpdate) -> Result<(), Error> {
        #[cfg(windows)]
        {
            self.device.update_resources(descriptor)
        }
        #[cfg(not(windows))]
        {
            let _ = descriptor;
            Err(Error::unsupported("d3d12 is only available on windows"))
        }
    }

    fn execute(&mut self, commands: &[RhiCommand]) -> Result<(), Error> {
        #[cfg(windows)]
        {
            self.device.execute(commands)
        }
        #[cfg(not(windows))]
        {
            let _ = commands;
            Err(Error::unsupported("d3d12 is only available on windows"))
        }
    }

    fn resize(&mut self, extent: Extent2D) -> Result<(), Error> {
        #[cfg(windows)]
        {
            self.device.resize(extent)
        }
        #[cfg(not(windows))]
        {
            let _ = extent;
            Err(Error::unsupported("d3d12 is only available on windows"))
        }
    }

    fn read_texture(&mut self, id: TextureId) -> Result<TextureReadback, Error> {
        #[cfg(windows)]
        {
            self.device.read_texture(id)
        }
        #[cfg(not(windows))]
        {
            let _ = id;
            Err(Error::unsupported("d3d12 is only available on windows"))
        }
    }

    fn destroy(self: Box<Self>) {
        #[cfg(windows)]
        {
            self.device.destroy();
        }
    }
}

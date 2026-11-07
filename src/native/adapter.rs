use super::prelude::*;

pub(crate) struct Adapter {
    pub(crate) adapter: IDXGIAdapter1,
}
impl Adapter {
    pub(crate) fn first() -> Result<Self, Error> {
        unsafe {
            let factory: IDXGIFactory4 = CreateDXGIFactory2(Default::default())
                .map_err(|_| Error::unsupported("CreateDXGIFactory2 failed"))?;
            let adapter = factory
                .EnumAdapters1(0)
                .map_err(|_| Error::unsupported("no d3d12 adapter"))?;
            Ok(Self { adapter })
        }
    }

    pub(crate) fn id(&self) -> AdapterId {
        AdapterId(0)
    }

    pub(crate) fn info(&self) -> AdapterInfo {
        AdapterInfo {
            id: AdapterId(0),
            name: "Direct3D 12".to_string(),
            hardware_family: HardwareFamily::Other,
            vendor_id: 0,
            device_id: 0,
            driver_name: "d3d12".to_string(),
            driver_info: String::new(),
            api_backend: GpuApiBackend::Dx12,
            capabilities: AdapterCapabilities {
                supports_window_presentation: true,
                queue_support: AdapterQueueSupport {
                    graphics_queue_count: 1,
                    compute_queue_count: 1,
                    transfer_queue_count: 1,
                },
                available_features: rotex_types::DeviceFeatures::default(),
                limits: Limits::CORE_MIN,
                extensions: rotex_types::FeatureSet::INDIRECT_DRAW,
                core: caps::CAPS,
            },
        }
    }
}

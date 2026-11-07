use super::prelude::*;
use super::types::*;

impl super::device::Device {
    pub(crate) fn allocate_rtv(
        &mut self,
        resource: &ID3D12Resource,
    ) -> Result<D3D12_CPU_DESCRIPTOR_HANDLE, Error> {
        unsafe {
            let mut cpu = self.rtv_heap.GetCPUDescriptorHandleForHeapStart();
            cpu.ptr += (self.rtv_cursor * self.rtv_size) as usize;
            self.rtv_cursor += 1;
            self.device.CreateRenderTargetView(resource, None, cpu);
            Ok(cpu)
        }
    }

    pub(crate) fn rtv_at(&self, index: u32) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        let mut cpu = unsafe { self.rtv_heap.GetCPUDescriptorHandleForHeapStart() };
        cpu.ptr += (index * self.rtv_size) as usize;
        cpu
    }

    pub(crate) fn create_swapchain(&mut self, hwnd: HWND) -> Result<(), Error> {
        self.submit()?;
        self.swap = None;
        let factory: IDXGIFactory2 = unsafe {
            CreateDXGIFactory2(Default::default())
                .map_err(|_| Error::unsupported("CreateDXGIFactory2 failed"))?
        };
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: self.extent.width,
            Height: self.extent.height,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            Stereo: false.into(),
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            Flags: 0,
        };
        let swap = unsafe {
            factory
                .CreateSwapChainForHwnd(&self.queue, hwnd, &desc, None, None)
                .map_err(|_| Error::unsupported("CreateSwapChainForHwnd failed"))?
        };
        let chain: IDXGISwapChain3 = swap
            .cast()
            .map_err(|_| Error::unsupported("IDXGISwapChain3 cast failed"))?;
        self.swap = Some(Swapchain {
            chain,
            buffers: Vec::new(),
            rtvs: Vec::new(),
            states: Vec::new(),
        });
        self.collect_backbuffers()
    }

    pub(crate) fn collect_backbuffers(&mut self) -> Result<(), Error> {
        let chain = self
            .swap
            .as_ref()
            .ok_or_else(|| Error::unsupported("swapchain missing"))?
            .chain
            .clone();
        let mut buffers = Vec::with_capacity(2);
        let mut rtvs = Vec::with_capacity(2);
        let mut states = Vec::with_capacity(2);
        for index in 0..2u32 {
            let buffer: ID3D12Resource = unsafe {
                chain
                    .GetBuffer(index)
                    .map_err(|_| Error::unsupported("GetBuffer failed"))?
            };
            let rtv = self.rtv_at(index);
            unsafe {
                self.device.CreateRenderTargetView(&buffer, None, rtv);
            }
            buffers.push(buffer);
            rtvs.push(rtv);
            states.push(D3D12_RESOURCE_STATE_PRESENT);
        }
        if let Some(swap) = self.swap.as_mut() {
            swap.buffers = buffers;
            swap.rtvs = rtvs;
            swap.states = states;
        }
        Ok(())
    }

    pub(crate) fn srv_cpu(&self, index: u32) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        let mut cpu = unsafe { self.srv_heap.GetCPUDescriptorHandleForHeapStart() };
        cpu.ptr += (index * self.srv_size) as usize;
        cpu
    }

    pub(crate) fn srv_gpu(&self, index: u32) -> D3D12_GPU_DESCRIPTOR_HANDLE {
        let mut gpu = unsafe { self.srv_heap.GetGPUDescriptorHandleForHeapStart() };
        gpu.ptr += u64::from(index) * u64::from(self.srv_size);
        gpu
    }
}

impl Swapchain {
    pub(crate) fn chain_index(&self) -> Result<usize, Error> {
        let index = unsafe { self.chain.GetCurrentBackBufferIndex() } as usize;
        if index >= self.buffers.len() {
            return Err(Error::unsupported("swapchain buffer index out of range"));
        }
        Ok(index)
    }
}

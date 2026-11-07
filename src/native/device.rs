use super::adapter::Adapter;
use super::debug::*;
use super::pipeline::{empty_root_signature, indexed_indirect_signature};
use super::prelude::*;
use super::resources::texture_location;
use super::types::*;

pub(crate) struct Device {
    pub(crate) queue: ID3D12CommandQueue,
    pub(crate) allocator: ID3D12CommandAllocator,
    pub(crate) list: ID3D12GraphicsCommandList,
    pub(crate) cmd_list: ID3D12CommandList,
    pub(crate) fence: ID3D12Fence,
    pub(crate) fence_event: HANDLE,
    pub(crate) fence_value: u64,
    pub(crate) debug: bool,
    pub(crate) root_signature: ID3D12RootSignature,
    pub(crate) command_signature: ID3D12CommandSignature,
    pub(crate) rtv_heap: ID3D12DescriptorHeap,
    pub(crate) rtv_size: u32,
    pub(crate) rtv_cursor: u32,
    pub(crate) srv_heap: ID3D12DescriptorHeap,
    pub(crate) srv_size: u32,
    pub(crate) srv_cursor: u32,
    pub(crate) recording: bool,
    pub(crate) buffers: HashMap<BufferId, GpuBuffer>,
    pub(crate) textures: HashMap<TextureId, GpuTexture>,
    pub(crate) pipelines: HashMap<GraphicsPipelineId, GpuPipeline>,
    pub(crate) compute_pipelines: HashMap<ComputePipelineId, GpuCompute>,
    pub(crate) layouts: HashMap<BindGroupLayoutId, u32>,
    pub(crate) groups: HashMap<BindGroupId, GpuGroup>,
    pub(crate) shaders: HashMap<ShaderId, ShaderArtifact>,
    pub(crate) bound_graphics: Option<GraphicsPipelineId>,
    pub(crate) bound_compute: Option<ComputePipelineId>,
    pub(crate) compute_bound: bool,
    pub(crate) root_bound: bool,
    pub(crate) swap: Option<Swapchain>,
    pub(crate) extent: Extent2D,
    pub(crate) back_index: Option<usize>,
    pub(crate) device: ID3D12Device,
}

impl Device {
    pub(crate) fn open(adapter: &Adapter) -> Result<Self, Error> {
        unsafe {
            let debug = debug_requested();
            if debug {
                enable_debug_layer();
            }
            let mut device: Option<ID3D12Device> = None;
            D3D12CreateDevice(&adapter.adapter, D3D_FEATURE_LEVEL_11_0, &mut device)
                .map_err(|_| Error::unsupported("D3D12CreateDevice failed"))?;
            let device = device.ok_or_else(|| Error::unsupported("D3D12CreateDevice failed"))?;
            let queue_desc = D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                ..Default::default()
            };
            let queue: ID3D12CommandQueue = device
                .CreateCommandQueue(&queue_desc)
                .map_err(|_| Error::unsupported("CreateCommandQueue failed"))?;
            let allocator: ID3D12CommandAllocator = device
                .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
                .map_err(|_| Error::unsupported("CreateCommandAllocator failed"))?;
            let list: ID3D12GraphicsCommandList = device
                .CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)
                .map_err(|_| Error::unsupported("CreateCommandList failed"))?;
            list.Close()
                .map_err(|_| Error::unsupported("command list close failed"))?;
            let cmd_list = list
                .cast::<ID3D12CommandList>()
                .map_err(|_| Error::unsupported("command list cast failed"))?;
            let fence: ID3D12Fence = device
                .CreateFence(0, D3D12_FENCE_FLAG_NONE)
                .map_err(|_| Error::unsupported("CreateFence failed"))?;
            let fence_event = CreateEventW(None, false, false, None)
                .map_err(|_| Error::unsupported("CreateEventW failed"))?;
            let root_signature = empty_root_signature(&device)?;
            let command_signature = indexed_indirect_signature(&device)?;
            let heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: 64,
                ..Default::default()
            };
            let rtv_heap: ID3D12DescriptorHeap = device
                .CreateDescriptorHeap(&heap_desc)
                .map_err(|_| Error::unsupported("CreateDescriptorHeap failed"))?;
            let rtv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV);
            let srv_desc = D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                NumDescriptors: 128,
                Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                NodeMask: 0,
            };
            let srv_heap: ID3D12DescriptorHeap = device
                .CreateDescriptorHeap(&srv_desc)
                .map_err(|_| Error::unsupported("CreateDescriptorHeap failed"))?;
            let srv_size =
                device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV);
            Ok(Self {
                queue,
                allocator,
                list,
                cmd_list,
                fence,
                fence_event,
                fence_value: 0,
                debug,
                root_signature,
                command_signature,
                rtv_heap,
                rtv_size,
                rtv_cursor: 2,
                srv_heap,
                srv_size,
                srv_cursor: 0,
                recording: false,
                buffers: HashMap::new(),
                textures: HashMap::new(),
                pipelines: HashMap::new(),
                compute_pipelines: HashMap::new(),
                layouts: HashMap::new(),
                groups: HashMap::new(),
                shaders: HashMap::new(),
                bound_graphics: None,
                bound_compute: None,
                compute_bound: false,
                root_bound: false,
                swap: None,
                extent: Extent2D {
                    width: 1,
                    height: 1,
                },
                back_index: None,
                device,
            })
        }
    }

    pub(crate) fn attach_surface(&mut self, surface: SurfaceDescriptor) -> Result<(), Error> {
        let RawWindowHandle::Win32(window) = surface.window_handle else {
            return Err(Error::unsupported("d3d12 surface requires a win32 window"));
        };
        let hwnd = HWND(window.hwnd.get() as *mut _);
        self.extent = surface.extent.clamped();
        self.create_swapchain(hwnd)
    }

    pub(crate) fn resize(&mut self, extent: Extent2D) -> Result<(), Error> {
        let extent = extent.clamped();
        if self.swap.is_none() {
            self.extent = extent;
            return Ok(());
        }
        if self.extent.width == extent.width && self.extent.height == extent.height {
            return Ok(());
        }
        // Present is queued after the frame fence, so the queue can still be
        // using the back buffers when a resize arrives. Wait for that work,
        // then reset the command list so it drops its buffer references.
        self.wait_idle()?;
        self.release_list_references()?;
        self.back_index = None;
        if self.debug {
            clear_debug_messages(&self.device);
            clear_dxgi_messages();
        }
        if let Some(swap) = self.swap.as_mut() {
            swap.buffers.clear();
        }
        let chain = self
            .swap
            .as_ref()
            .ok_or_else(|| Error::unsupported("swapchain missing"))?
            .chain
            .clone();
        let resized = unsafe {
            chain.ResizeBuffers(
                2,
                extent.width,
                extent.height,
                DXGI_FORMAT_R8G8B8A8_UNORM,
                Default::default(),
            )
        };
        if resized.is_ok() {
            self.extent = extent;
        }
        self.collect_backbuffers()?;
        resized.map_err(|err| {
            let outstanding = self
                .swap
                .as_ref()
                .map(|swap| swap.buffers.len())
                .unwrap_or(0);
            let detail = if self.debug {
                format!(
                    " | restored {outstanding} buffers | {} | {}",
                    debug_messages(&self.device),
                    dxgi_debug_messages()
                )
            } else {
                " | re-run with ROTEX_D3D12_DEBUG=1 for the validation log".to_string()
            };
            Error::fatal(rotex_core::ErrorKind::Backend(format!(
                "ResizeBuffers failed: {err}{detail}"
            )))
        })
    }

    pub(crate) fn destroy(mut self) {
        // The close arrives on the window thread while the last Present may
        // still be queued. Release the swapchain only after that work finishes,
        // and let the device field drop last.
        let _ = self.wait_idle();
        let _ = self.release_list_references();
        unsafe {
            let _ = CloseHandle(self.fence_event);
        }
        self.back_index = None;
        self.bound_graphics = None;
        self.bound_compute = None;
        if let Some(swap) = self.swap.as_mut() {
            swap.buffers.clear();
        }
        self.swap = None;
        self.textures.clear();
        self.buffers.clear();
        self.pipelines.clear();
        self.compute_pipelines.clear();
        self.groups.clear();
        self.layouts.clear();
        self.shaders.clear();
    }

    pub(crate) fn create_resources(
        &mut self,
        batch: ResourceBatchCreate,
    ) -> Result<CreatedResources, Error> {
        let mut handles = Vec::new();
        for resource in batch.resources {
            handles.push(self.create_one(resource)?);
        }
        Ok(CreatedResources { handles })
    }

    pub(crate) fn update_resources(&mut self, batch: ResourceBatchUpdate) -> Result<(), Error> {
        for update in batch.updates {
            match update {
                ResourceUpdateDescriptor::Destroy { handle } => {
                    self.destroy_handle(handle);
                }
                ResourceUpdateDescriptor::Buffer { id, data } => {
                    self.write_buffer(id, 0, &data)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(crate) fn execute(&mut self, commands: &[RhiCommand]) -> Result<(), Error> {
        for command in commands {
            self.record(command)?;
        }
        if self.recording {
            self.submit()?;
        }
        Ok(())
    }

    pub(crate) fn read_texture(&mut self, id: TextureId) -> Result<TextureReadback, Error> {
        let (width, height, format, resource) = {
            let texture = self
                .textures
                .get(&id)
                .ok_or_else(|| Error::unsupported("texture not found"))?;
            (
                texture.width,
                texture.height,
                texture.format,
                texture.resource.clone(),
            )
        };
        let byte_count = (width * height * 4) as u64;
        let readback = self.create_buffer_resource(
            byte_count,
            D3D12_HEAP_TYPE_READBACK,
            D3D12_RESOURCE_FLAG_NONE,
        )?;
        self.begin()?;
        let mut destination = texture_location(&readback, 0);
        let mut source = texture_location(&resource, 0);
        unsafe {
            self.list
                .CopyTextureRegion(&destination, 0, 0, 0, &source, None);
            std::mem::ManuallyDrop::drop(&mut destination.pResource);
            std::mem::ManuallyDrop::drop(&mut source.pResource);
        }
        self.submit()?;
        let mut data = vec![0u8; byte_count as usize];
        unsafe {
            let mut ptr = std::ptr::null_mut();
            readback
                .Map(0, None, Some(&mut ptr))
                .map_err(|_| Error::unsupported("readback map failed"))?;
            std::ptr::copy_nonoverlapping(ptr as *const u8, data.as_mut_ptr(), data.len());
            readback.Unmap(0, None);
        }
        Ok(TextureReadback {
            data,
            format,
            width,
            height,
        })
    }
}

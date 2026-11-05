//! Direct3D 12 translator. This module is compiled only on Windows.

use std::collections::HashMap;

use rotex_core::Error;
use rotex_types::{
    AdapterCapabilities, AdapterId, AdapterInfo, AdapterQueueSupport, BufferId, CreatedResources,
    D3dRegisterClass, Extent2D, GpuApiBackend, GraphicsPipelineId, HardwareFamily, Limits,
    PassColorTarget, PhysicalRemap, ResourceBatchCreate, ResourceBatchUpdate,
    ResourceCreateDescriptor, ResourceHandle, ResourceState, ResourceUpdateDescriptor, RhiCommand,
    ShaderArtifact, ShaderId, SurfaceDescriptor, TargetId, TextureId, TextureReadback,
};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory2, IDXGIAdapter1, IDXGIFactory4};
use windows::core::Interface;

use crate::caps;

const INDIRECT_OPCODE: u32 = 1;

pub(crate) struct Adapter {
    adapter: IDXGIAdapter1,
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

struct GpuBuffer {
    resource: ID3D12Resource,
    size: u64,
}

struct GpuTexture {
    resource: ID3D12Resource,
    width: u32,
    height: u32,
    format: rotex_types::TextureFormat,
    state: D3D12_RESOURCE_STATES,
    rtv: Option<D3D12_CPU_DESCRIPTOR_HANDLE>,
}

struct GpuPipeline {
    state: ID3D12PipelineState,
    root_signature: ID3D12RootSignature,
}

pub(crate) struct Device {
    device: ID3D12Device,
    queue: ID3D12CommandQueue,
    allocator: ID3D12CommandAllocator,
    list: ID3D12GraphicsCommandList,
    fence: ID3D12Fence,
    fence_value: u64,
    root_signature: ID3D12RootSignature,
    command_signature: ID3D12CommandSignature,
    rtv_heap: ID3D12DescriptorHeap,
    rtv_size: u32,
    rtv_cursor: u32,
    recording: bool,
    buffers: HashMap<BufferId, GpuBuffer>,
    textures: HashMap<TextureId, GpuTexture>,
    pipelines: HashMap<GraphicsPipelineId, GpuPipeline>,
    shaders: HashMap<ShaderId, ShaderArtifact>,
    bound_signature: Option<ID3D12RootSignature>,
}

impl Device {
    pub(crate) fn open(adapter: &Adapter) -> Result<Self, Error> {
        unsafe {
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
            let fence: ID3D12Fence = device
                .CreateFence(0, D3D12_FENCE_FLAG_NONE)
                .map_err(|_| Error::unsupported("CreateFence failed"))?;
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
            Ok(Self {
                device,
                queue,
                allocator,
                list,
                fence,
                fence_value: 0,
                root_signature,
                command_signature,
                rtv_heap,
                rtv_size,
                rtv_cursor: 0,
                recording: false,
                buffers: HashMap::new(),
                textures: HashMap::new(),
                pipelines: HashMap::new(),
                shaders: HashMap::new(),
                bound_signature: None,
            })
        }
    }

    pub(crate) fn attach_surface(&mut self, _surface: SurfaceDescriptor) -> Result<(), Error> {
        Ok(())
    }

    pub(crate) fn resize(&mut self, _extent: Extent2D) -> Result<(), Error> {
        Ok(())
    }

    pub(crate) fn destroy(self) {}

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
        let readback = self.create_buffer_resource(byte_count, D3D12_HEAP_TYPE_READBACK)?;
        self.begin()?;
        unsafe {
            self.list.CopyTextureRegion(
                &texture_location(&readback, 0),
                0,
                0,
                0,
                &texture_location(&resource, 0),
                None,
            );
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

    fn create_one(&mut self, resource: ResourceCreateDescriptor) -> Result<ResourceHandle, Error> {
        match resource {
            ResourceCreateDescriptor::Buffer { id, buffer } => {
                let id = BufferId(id);
                if self.buffers.contains_key(&id) {
                    return Err(Error::unsupported("resource id already exists"));
                }
                let resource =
                    self.create_buffer_resource(buffer.size.max(4), D3D12_HEAP_TYPE_UPLOAD)?;
                if let Some(data) = buffer.initial_data.as_ref() {
                    upload_bytes(&resource, data)?;
                }
                self.buffers.insert(
                    id,
                    GpuBuffer {
                        resource,
                        size: buffer.size,
                    },
                );
                Ok(ResourceHandle::Buffer(id))
            }
            ResourceCreateDescriptor::Texture { id, texture } => {
                let id = TextureId(id);
                if self.textures.contains_key(&id) {
                    return Err(Error::unsupported("resource id already exists"));
                }
                let flags = if texture.render_attachment {
                    D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET
                } else {
                    D3D12_RESOURCE_FLAG_NONE
                };
                let desc = D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                    Width: u64::from(texture.width.max(1)),
                    Height: texture.height.max(1),
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
                    Flags: flags,
                    ..Default::default()
                };
                let heap = heap(D3D12_HEAP_TYPE_DEFAULT);
                let mut created: Option<ID3D12Resource> = None;
                unsafe {
                    self.device
                        .CreateCommittedResource(
                            &heap,
                            D3D12_HEAP_FLAG_NONE,
                            &desc,
                            D3D12_RESOURCE_STATE_COMMON,
                            None,
                            &mut created,
                        )
                        .map_err(|_| Error::unsupported("texture creation failed"))?;
                }
                let resource =
                    created.ok_or_else(|| Error::unsupported("texture creation failed"))?;
                let rtv = if texture.render_attachment {
                    Some(self.allocate_rtv(&resource)?)
                } else {
                    None
                };
                self.textures.insert(
                    id,
                    GpuTexture {
                        resource,
                        width: texture.width.max(1),
                        height: texture.height.max(1),
                        format: texture.format,
                        state: D3D12_RESOURCE_STATE_COMMON,
                        rtv,
                    },
                );
                Ok(ResourceHandle::Texture(id))
            }
            ResourceCreateDescriptor::GraphicsPipeline { id, pipeline } => {
                let id = GraphicsPipelineId(id);
                if self.pipelines.contains_key(&id) {
                    return Err(Error::unsupported("resource id already exists"));
                }
                let (state, root_signature) = create_pipeline_state(
                    &self.device,
                    &pipeline.shaders.layout,
                    &pipeline.shaders.vertex,
                    &pipeline.shaders.fragment,
                )?;
                self.pipelines.insert(
                    id,
                    GpuPipeline {
                        state,
                        root_signature,
                    },
                );
                Ok(ResourceHandle::GraphicsPipeline(id))
            }
            ResourceCreateDescriptor::Shader { id, artifact } => {
                let id = ShaderId(id);
                if self.shaders.contains_key(&id) {
                    return Err(Error::unsupported("resource id already exists"));
                }
                if artifact.payload(TargetId::D3d12Dxil).is_none() {
                    return Err(Error::unsupported("shader artifact has no DXIL payload"));
                }
                if let Some(cap) = artifact.unmet_capability(&["compute", "storage_buffer"]) {
                    let message = match cap {
                        "compute" => "shader requires compute",
                        "storage_buffer" => "shader requires storage buffers",
                        _ => "shader requires an unsupported capability",
                    };
                    return Err(Error::unsupported(message));
                }
                self.shaders.insert(id, artifact);
                Ok(ResourceHandle::Shader(id))
            }
            _ => Err(Error::unsupported("resource is not implemented on d3d12")),
        }
    }

    fn destroy_handle(&mut self, handle: ResourceHandle) {
        match handle {
            ResourceHandle::Buffer(id) => {
                self.buffers.remove(&id);
            }
            ResourceHandle::Texture(id) => {
                self.textures.remove(&id);
            }
            ResourceHandle::GraphicsPipeline(id) => {
                self.pipelines.remove(&id);
            }
            ResourceHandle::Shader(id) => {
                self.shaders.remove(&id);
            }
            _ => {}
        }
    }

    fn active_signature(&self) -> ID3D12RootSignature {
        self.bound_signature
            .clone()
            .unwrap_or_else(|| self.root_signature.clone())
    }

    fn record(&mut self, command: &RhiCommand) -> Result<(), Error> {
        match command {
            RhiCommand::BeginFrame { .. } => self.begin(),
            RhiCommand::SubmitFrame { .. } => self.submit(),
            RhiCommand::AcquirePresentationTarget => Ok(()),
            RhiCommand::Barrier { resource, state } => self.barrier(resource, *state),
            RhiCommand::WriteBuffer {
                buffer,
                offset,
                data,
            } => {
                let resource = self
                    .buffers
                    .get(buffer)
                    .ok_or_else(|| Error::unsupported("buffer not found"))?
                    .resource
                    .clone();
                upload_bytes_at(&resource, *offset, data)
            }
            RhiCommand::CopyBufferToBuffer {
                src,
                src_offset,
                dst,
                dst_offset,
                size,
            } => {
                self.begin()?;
                let src = self
                    .buffers
                    .get(src)
                    .ok_or_else(|| Error::unsupported("source buffer not found"))?
                    .resource
                    .clone();
                let dst = self
                    .buffers
                    .get(dst)
                    .ok_or_else(|| Error::unsupported("destination buffer not found"))?
                    .resource
                    .clone();
                unsafe {
                    self.list
                        .CopyBufferRegion(&dst, *dst_offset, &src, *src_offset, *size);
                }
                Ok(())
            }
            RhiCommand::BeginRenderPass { pass } => {
                self.begin()?;
                let PassColorTarget::Texture(id) = pass.color_target else {
                    return Ok(());
                };
                let texture = self
                    .textures
                    .get(&id)
                    .ok_or_else(|| Error::unsupported("render target texture not found"))?;
                let Some(rtv) = texture.rtv else {
                    return Err(Error::unsupported("texture is not a render target"));
                };
                let resource = texture.resource.clone();
                let before = texture.state;
                self.transition(&resource, before, D3D12_RESOURCE_STATE_RENDER_TARGET)?;
                if let Some(texture) = self.textures.get_mut(&id) {
                    texture.state = D3D12_RESOURCE_STATE_RENDER_TARGET;
                }
                unsafe {
                    self.list.OMSetRenderTargets(1, Some(&rtv), false, None);
                    if matches!(pass.color_load, rotex_types::ColorAttachmentLoad::Clear) {
                        self.list
                            .ClearRenderTargetView(rtv, &pass.clear_color, None);
                    }
                }
                Ok(())
            }
            RhiCommand::EndRenderPass => Ok(()),
            RhiCommand::Dispatch { workgroup_count } => {
                self.begin()?;
                unsafe {
                    self.list
                        .Dispatch(workgroup_count[0], workgroup_count[1], workgroup_count[2]);
                }
                Ok(())
            }
            RhiCommand::PushConstants { offset, data, .. } => {
                self.begin()?;
                let signature = self.active_signature();
                unsafe {
                    self.list.SetGraphicsRootSignature(&signature);
                    self.list.SetGraphicsRoot32BitConstants(
                        0,
                        (data.len() / 4) as u32,
                        data.as_ptr().cast(),
                        *offset / 4,
                    );
                }
                Ok(())
            }
            RhiCommand::SetGraphicsPipeline { pipeline } => {
                self.begin()?;
                let (state, signature) = {
                    let pipeline = self
                        .pipelines
                        .get(pipeline)
                        .ok_or_else(|| Error::unsupported("graphics pipeline not found"))?;
                    (pipeline.state.clone(), pipeline.root_signature.clone())
                };
                self.bound_signature = Some(signature.clone());
                unsafe {
                    self.list.SetGraphicsRootSignature(&signature);
                    self.list.SetPipelineState(&state);
                }
                Ok(())
            }
            RhiCommand::DrawIndexed {
                index_count,
                instance_count,
                first_index,
                vertex_offset,
                first_instance,
            } => {
                self.begin()?;
                unsafe {
                    self.list.DrawIndexedInstanced(
                        *index_count,
                        *instance_count,
                        *first_index,
                        *vertex_offset,
                        *first_instance,
                    );
                }
                Ok(())
            }
            RhiCommand::Extension {
                feature,
                opcode,
                payload,
            } => {
                use rotex_core::Feature;
                use rotex_types::IndirectDraw;
                if *feature != IndirectDraw::ID || *opcode != INDIRECT_OPCODE {
                    return Err(Error::unsupported("extension command is not implemented"));
                }
                let (buffer_id, offset) = decode_indirect(payload)?;
                let buffer = BufferId(buffer_id);
                let resource = self
                    .buffers
                    .get(&buffer)
                    .ok_or_else(|| Error::unsupported("indirect buffer not found"))?
                    .resource
                    .clone();
                self.begin()?;
                unsafe {
                    self.list.ExecuteIndirect(
                        &self.command_signature,
                        1,
                        &resource,
                        offset,
                        None,
                        0,
                    );
                }
                Ok(())
            }
            RhiCommand::SetViewport {
                x,
                y,
                width,
                height,
                min_depth,
                max_depth,
            } => {
                self.begin()?;
                let viewport = D3D12_VIEWPORT {
                    TopLeftX: *x,
                    TopLeftY: *y,
                    Width: *width,
                    Height: *height,
                    MinDepth: *min_depth,
                    MaxDepth: *max_depth,
                };
                unsafe {
                    self.list.RSSetViewports(&[viewport]);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn barrier(
        &mut self,
        resource: &rotex_types::BarrierResource,
        state: ResourceState,
    ) -> Result<(), Error> {
        let after = d3d_state(state);
        match resource {
            rotex_types::BarrierResource::Texture(id) => {
                let texture = self
                    .textures
                    .get(id)
                    .ok_or_else(|| Error::unsupported("texture not found"))?;
                let before = texture.state;
                let resource = texture.resource.clone();
                self.transition(&resource, before, after)?;
                if let Some(texture) = self.textures.get_mut(id) {
                    texture.state = after;
                }
                Ok(())
            }
            rotex_types::BarrierResource::Buffer(_) => Ok(()),
        }
    }

    fn transition(
        &mut self,
        resource: &ID3D12Resource,
        before: D3D12_RESOURCE_STATES,
        after: D3D12_RESOURCE_STATES,
    ) -> Result<(), Error> {
        if before == after {
            return Ok(());
        }
        self.begin()?;
        let barrier = D3D12_RESOURCE_BARRIER {
            Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
            Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
            Anonymous: D3D12_RESOURCE_BARRIER_0 {
                Transition: std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                    pResource: std::mem::ManuallyDrop::new(Some(resource.clone())),
                    Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                    StateBefore: before,
                    StateAfter: after,
                }),
            },
        };
        unsafe {
            self.list.ResourceBarrier(&[barrier]);
        }
        Ok(())
    }

    fn begin(&mut self) -> Result<(), Error> {
        if self.recording {
            return Ok(());
        }
        unsafe {
            self.allocator
                .Reset()
                .map_err(|_| Error::unsupported("command allocator reset failed"))?;
            self.list
                .Reset(&self.allocator, None)
                .map_err(|_| Error::unsupported("command list reset failed"))?;
        }
        self.recording = true;
        Ok(())
    }

    fn submit(&mut self) -> Result<(), Error> {
        if !self.recording {
            return Ok(());
        }
        unsafe {
            self.list
                .Close()
                .map_err(|_| Error::unsupported("command list close failed"))?;
            let lists: [Option<ID3D12CommandList>; 1] =
                [Some(self.list.cast::<ID3D12CommandList>().map_err(
                    |_| Error::unsupported("command list cast failed"),
                )?)];
            self.queue.ExecuteCommandLists(&lists);
            self.fence_value += 1;
            self.queue
                .Signal(&self.fence, self.fence_value)
                .map_err(|_| Error::unsupported("fence signal failed"))?;
            while self.fence.GetCompletedValue() < self.fence_value {
                std::thread::yield_now();
            }
        }
        self.recording = false;
        Ok(())
    }

    fn write_buffer(&mut self, id: BufferId, offset: u64, data: &[u8]) -> Result<(), Error> {
        let resource = self
            .buffers
            .get(&id)
            .ok_or_else(|| Error::unsupported("buffer not found"))?
            .resource
            .clone();
        upload_bytes_at(&resource, offset, data)
    }

    fn create_buffer_resource(
        &self,
        size: u64,
        heap_type: D3D12_HEAP_TYPE,
    ) -> Result<ID3D12Resource, Error> {
        let desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Width: size.max(4),
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_UNKNOWN,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            ..Default::default()
        };
        let state = if heap_type == D3D12_HEAP_TYPE_UPLOAD {
            D3D12_RESOURCE_STATE_GENERIC_READ
        } else if heap_type == D3D12_HEAP_TYPE_READBACK {
            D3D12_RESOURCE_STATE_COPY_DEST
        } else {
            D3D12_RESOURCE_STATE_COMMON
        };
        let mut resource: Option<ID3D12Resource> = None;
        unsafe {
            self.device
                .CreateCommittedResource(
                    &heap(heap_type),
                    D3D12_HEAP_FLAG_NONE,
                    &desc,
                    state,
                    None,
                    &mut resource,
                )
                .map_err(|_| Error::unsupported("buffer creation failed"))?;
        }
        resource.ok_or_else(|| Error::unsupported("buffer creation failed"))
    }

    fn allocate_rtv(
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
}

fn heap(kind: D3D12_HEAP_TYPE) -> D3D12_HEAP_PROPERTIES {
    D3D12_HEAP_PROPERTIES {
        Type: kind,
        ..Default::default()
    }
}

fn upload_bytes(resource: &ID3D12Resource, data: &[u8]) -> Result<(), Error> {
    upload_bytes_at(resource, 0, data)
}

fn upload_bytes_at(resource: &ID3D12Resource, offset: u64, data: &[u8]) -> Result<(), Error> {
    unsafe {
        let mut ptr = std::ptr::null_mut();
        resource
            .Map(0, None, Some(&mut ptr))
            .map_err(|_| Error::unsupported("buffer map failed"))?;
        std::ptr::copy_nonoverlapping(
            data.as_ptr(),
            ptr.add(offset as usize) as *mut u8,
            data.len(),
        );
        resource.Unmap(0, None);
    }
    Ok(())
}

fn texture_location(resource: &ID3D12Resource, offset: u64) -> D3D12_TEXTURE_COPY_LOCATION {
    D3D12_TEXTURE_COPY_LOCATION {
        pResource: std::mem::ManuallyDrop::new(Some(resource.clone())),
        Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
        Anonymous: unsafe { std::mem::zeroed() },
    }
    .with_offset(offset)
}

trait WithOffset {
    fn with_offset(self, _offset: u64) -> Self;
}

impl WithOffset for D3D12_TEXTURE_COPY_LOCATION {
    fn with_offset(self, _offset: u64) -> Self {
        self
    }
}

fn d3d_state(state: ResourceState) -> D3D12_RESOURCE_STATES {
    match state {
        ResourceState::CopySrc => D3D12_RESOURCE_STATE_COPY_SOURCE,
        ResourceState::CopyDst => D3D12_RESOURCE_STATE_COPY_DEST,
        ResourceState::ShaderRead => D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
        ResourceState::ShaderWrite => D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
        ResourceState::ColorTarget => D3D12_RESOURCE_STATE_RENDER_TARGET,
        ResourceState::DepthTarget => D3D12_RESOURCE_STATE_DEPTH_WRITE,
        ResourceState::VertexBuffer | ResourceState::IndexBuffer | ResourceState::Uniform => {
            D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER
        }
        ResourceState::VertexShaderRead => {
            D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER
                | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE
        }
        ResourceState::IndirectArg => D3D12_RESOURCE_STATE_INDIRECT_ARGUMENT,
        ResourceState::Present => D3D12_RESOURCE_STATE_PRESENT,
        _ => D3D12_RESOURCE_STATE_COMMON,
    }
}

fn empty_root_signature(device: &ID3D12Device) -> Result<ID3D12RootSignature, Error> {
    unsafe {
        let constants = D3D12_ROOT_CONSTANTS {
            ShaderRegister: 0,
            RegisterSpace: 0,
            Num32BitValues: 16,
        };
        let parameter = D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                Constants: constants,
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
        };
        let desc = D3D12_ROOT_SIGNATURE_DESC {
            NumParameters: 1,
            pParameters: &parameter,
            NumStaticSamplers: 0,
            pStaticSamplers: std::ptr::null(),
            Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        };
        let mut blob: Option<windows::Win32::Graphics::Direct3D::ID3DBlob> = None;
        D3D12SerializeRootSignature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut blob, None)
            .map_err(|_| Error::unsupported("D3D12SerializeRootSignature failed"))?;
        let blob = blob.ok_or_else(|| Error::unsupported("root signature blob missing"))?;
        let bytes =
            std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize());
        device
            .CreateRootSignature(0, bytes)
            .map_err(|_| Error::unsupported("CreateRootSignature failed"))
    }
}

fn indexed_indirect_signature(device: &ID3D12Device) -> Result<ID3D12CommandSignature, Error> {
    unsafe {
        let argument = D3D12_INDIRECT_ARGUMENT_DESC {
            Type: D3D12_INDIRECT_ARGUMENT_TYPE_DRAW_INDEXED,
            Anonymous: std::mem::zeroed(),
        };
        let desc = D3D12_COMMAND_SIGNATURE_DESC {
            ByteStride: 20,
            NumArgumentDescs: 1,
            pArgumentDescs: &argument,
            NodeMask: 0,
        };
        let mut signature: Option<ID3D12CommandSignature> = None;
        device
            .CreateCommandSignature(&desc, None, &mut signature)
            .map_err(|_| Error::unsupported("CreateCommandSignature failed"))?;
        signature.ok_or_else(|| Error::unsupported("CreateCommandSignature failed"))
    }
}

fn create_pipeline_state(
    device: &ID3D12Device,
    layout: &rotex_types::AbstractPipelineLayout,
    vertex: &rotex_types::ShaderPackage,
    fragment: &rotex_types::ShaderPackage,
) -> Result<(ID3D12PipelineState, ID3D12RootSignature), Error> {
    let vs = dxil(vertex).ok_or_else(|| Error::unsupported("vertex shader has no DXIL payload"))?;
    let ps =
        dxil(fragment).ok_or_else(|| Error::unsupported("fragment shader has no DXIL payload"))?;
    let remap = PhysicalRemap::from_layout(TargetId::D3d12Dxil, layout);
    let root = root_signature_from_remap(device, &remap)?;
    unsafe {
        let mut desc: D3D12_GRAPHICS_PIPELINE_STATE_DESC = std::mem::zeroed();
        desc.pRootSignature = std::mem::ManuallyDrop::new(Some(root.clone()));
        desc.VS = D3D12_SHADER_BYTECODE {
            pShaderBytecode: vs.as_ptr().cast(),
            BytecodeLength: vs.len(),
        };
        desc.PS = D3D12_SHADER_BYTECODE {
            pShaderBytecode: ps.as_ptr().cast(),
            BytecodeLength: ps.len(),
        };
        desc.BlendState.RenderTarget[0].RenderTargetWriteMask = 0x0f;
        desc.SampleMask = u32::MAX;
        desc.RasterizerState.FillMode = D3D12_FILL_MODE_SOLID;
        desc.RasterizerState.CullMode = D3D12_CULL_MODE_BACK;
        desc.DepthStencilState.DepthEnable = false.into();
        desc.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
        desc.NumRenderTargets = 1;
        desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
        desc.SampleDesc.Count = 1;
        let state = device
            .CreateGraphicsPipelineState(&desc)
            .map_err(|_| Error::unsupported("CreateGraphicsPipelineState failed"))?;
        Ok((state, root))
    }
}

fn root_signature_from_remap(
    device: &ID3D12Device,
    remap: &PhysicalRemap,
) -> Result<ID3D12RootSignature, Error> {
    let PhysicalRemap::D3d12 {
        bindings,
        root_constants,
    } = remap
    else {
        return Err(Error::unsupported("root signature requires a D3D12 remap"));
    };
    let constant_count = root_constants
        .as_ref()
        .map(|constants| constants.num_32bit_values)
        .unwrap_or(16)
        .max(1);
    let mut parameters = Vec::with_capacity(bindings.len() + 1);
    parameters.push(D3D12_ROOT_PARAMETER {
        ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
        Anonymous: D3D12_ROOT_PARAMETER_0 {
            Constants: D3D12_ROOT_CONSTANTS {
                ShaderRegister: root_constants
                    .as_ref()
                    .map(|constants| constants.register)
                    .unwrap_or(0),
                RegisterSpace: root_constants
                    .as_ref()
                    .map(|constants| constants.space)
                    .unwrap_or(0),
                Num32BitValues: constant_count,
            },
        },
        ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
    });
    for binding in bindings {
        let parameter_type = match binding.class {
            D3dRegisterClass::B => D3D12_ROOT_PARAMETER_TYPE_CBV,
            D3dRegisterClass::T => D3D12_ROOT_PARAMETER_TYPE_SRV,
            D3dRegisterClass::U => D3D12_ROOT_PARAMETER_TYPE_UAV,
            D3dRegisterClass::S => {
                return Err(Error::unsupported(
                    "sampler root signature requires a descriptor table",
                ));
            }
        };
        parameters.push(D3D12_ROOT_PARAMETER {
            ParameterType: parameter_type,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                Descriptor: D3D12_ROOT_DESCRIPTOR {
                    ShaderRegister: binding.register,
                    RegisterSpace: binding.space,
                },
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
        });
    }
    unsafe {
        let desc = D3D12_ROOT_SIGNATURE_DESC {
            NumParameters: parameters.len() as u32,
            pParameters: if parameters.is_empty() {
                std::ptr::null()
            } else {
                parameters.as_ptr()
            },
            NumStaticSamplers: 0,
            pStaticSamplers: std::ptr::null(),
            Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        };
        let mut blob: Option<windows::Win32::Graphics::Direct3D::ID3DBlob> = None;
        D3D12SerializeRootSignature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut blob, None)
            .map_err(|_| Error::unsupported("D3D12SerializeRootSignature failed"))?;
        let blob = blob.ok_or_else(|| Error::unsupported("root signature blob missing"))?;
        let bytes =
            std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize());
        device
            .CreateRootSignature(0, bytes)
            .map_err(|_| Error::unsupported("CreateRootSignature failed"))
    }
}

fn dxil(package: &rotex_types::ShaderPackage) -> Option<&[u8]> {
    match package.variants.dxil.as_ref() {
        Some(rotex_types::ShaderPayload::Dxil(bytes)) if !bytes.is_empty() => Some(bytes),
        _ => None,
    }
}

fn decode_indirect(payload: &[u8]) -> Result<(u64, u64), Error> {
    if payload.len() < 16 {
        return Err(Error::unsupported("indirect payload is too short"));
    }
    let mut buffer_bytes = [0u8; 8];
    let mut offset_bytes = [0u8; 8];
    buffer_bytes.copy_from_slice(&payload[0..8]);
    offset_bytes.copy_from_slice(&payload[8..16]);
    Ok((
        u64::from_le_bytes(buffer_bytes),
        u64::from_le_bytes(offset_bytes),
    ))
}

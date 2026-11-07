use super::format::*;
use super::pipeline::{create_compute_pipeline, create_graphics_pipeline};
use super::prelude::*;
use super::types::*;

impl super::device::Device {
    pub(crate) fn create_one(
        &mut self,
        resource: ResourceCreateDescriptor,
    ) -> Result<ResourceHandle, Error> {
        match resource {
            ResourceCreateDescriptor::Buffer { id, buffer } => {
                let id = BufferId(id);
                if self.buffers.contains_key(&id) {
                    return Err(Error::unsupported("resource id already exists"));
                }
                let storage = buffer.usages.contains(BufferUsages::STORAGE)
                    || buffer.usage == BufferUsage::Storage;
                let resource = if storage {
                    self.create_buffer_resource(
                        buffer.size.max(4),
                        D3D12_HEAP_TYPE_DEFAULT,
                        D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
                    )?
                } else {
                    self.create_buffer_resource(
                        buffer.size.max(4),
                        D3D12_HEAP_TYPE_UPLOAD,
                        D3D12_RESOURCE_FLAG_NONE,
                    )?
                };
                if let Some(data) = buffer.initial_data.as_ref() {
                    if storage {
                        self.upload_default_buffer(&resource, data)?;
                    } else {
                        upload_bytes(&resource, data)?;
                    }
                }
                let gpu_address = unsafe { resource.GetGPUVirtualAddress() };
                self.buffers.insert(
                    id,
                    GpuBuffer {
                        resource,
                        gpu_address,
                        size: buffer.size,
                        state: if storage {
                            D3D12_RESOURCE_STATE_COMMON
                        } else {
                            D3D12_RESOURCE_STATE_GENERIC_READ
                        },
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
                let format = dxgi_format(texture.format);
                let desc = D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                    Width: u64::from(texture.width.max(1)),
                    Height: texture.height.max(1),
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    Format: format,
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
                if texture.render_attachment
                    && texture.data.iter().all(|byte| *byte == 0)
                    && let Some(rtv) = rtv
                {
                    self.clear_target(&resource, rtv)?;
                }
                self.textures.insert(
                    id,
                    GpuTexture {
                        resource,
                        width: texture.width.max(1),
                        height: texture.height.max(1),
                        format: texture.format,
                        state: D3D12_RESOURCE_STATE_COMMON,
                        rtv,
                        srv: None,
                    },
                );
                Ok(ResourceHandle::Texture(id))
            }
            ResourceCreateDescriptor::GraphicsPipeline { id, pipeline } => {
                let id = GraphicsPipelineId(id);
                if self.pipelines.contains_key(&id) {
                    return Err(Error::unsupported("resource id already exists"));
                }
                let (state, root_signature, views, topology, strides) =
                    create_graphics_pipeline(&self.device, &pipeline, self.debug)?;
                self.pipelines.insert(
                    id,
                    GpuPipeline {
                        state,
                        root_signature,
                        views,
                        topology,
                        strides,
                    },
                );
                Ok(ResourceHandle::GraphicsPipeline(id))
            }
            ResourceCreateDescriptor::ComputePipeline { id, pipeline } => {
                let id = ComputePipelineId(id);
                if self.compute_pipelines.contains_key(&id) {
                    return Err(Error::unsupported("resource id already exists"));
                }
                let (state, root_signature, views) =
                    create_compute_pipeline(&self.device, &pipeline)?;
                self.compute_pipelines.insert(
                    id,
                    GpuCompute {
                        state,
                        root_signature,
                        views,
                    },
                );
                Ok(ResourceHandle::ComputePipeline(id))
            }
            ResourceCreateDescriptor::BindGroupLayout { id, layout } => {
                let id = BindGroupLayoutId(id);
                if self.layouts.contains_key(&id) {
                    return Err(Error::unsupported("resource id already exists"));
                }
                self.layouts.insert(id, layout.set);
                Ok(ResourceHandle::BindGroupLayout(id))
            }
            ResourceCreateDescriptor::BindGroup { id, group } => {
                let id = BindGroupId(id);
                if self.groups.contains_key(&id) {
                    return Err(Error::unsupported("resource id already exists"));
                }
                let set = self.layouts.get(&group.layout).copied().unwrap_or(0);
                let slots = group
                    .entries
                    .iter()
                    .map(|entry| match entry {
                        rotex_types::BindGroupEntryDescriptor::Buffer {
                            binding,
                            buffer,
                            offset,
                            ..
                        } => GroupSlot {
                            binding: *binding,
                            buffer: Some(*buffer),
                            offset: *offset,
                            texture: None,
                        },
                        rotex_types::BindGroupEntryDescriptor::Texture { binding, texture } => {
                            GroupSlot {
                                binding: *binding,
                                buffer: None,
                                offset: 0,
                                texture: Some(*texture),
                            }
                        }
                    })
                    .collect();
                self.groups.insert(id, GpuGroup { set, slots });
                Ok(ResourceHandle::BindGroup(id))
            }
            ResourceCreateDescriptor::Sampler { id, .. } => {
                Ok(ResourceHandle::Sampler(SamplerId(id)))
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

    pub(crate) fn destroy_handle(&mut self, handle: ResourceHandle) {
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
            ResourceHandle::ComputePipeline(id) => {
                self.compute_pipelines.remove(&id);
            }
            ResourceHandle::BindGroupLayout(id) => {
                self.layouts.remove(&id);
            }
            ResourceHandle::BindGroup(id) => {
                self.groups.remove(&id);
            }
            ResourceHandle::Shader(id) => {
                self.shaders.remove(&id);
            }
            _ => {}
        }
    }

    pub(crate) fn write_buffer(
        &mut self,
        id: BufferId,
        offset: u64,
        data: &[u8],
    ) -> Result<(), Error> {
        let resource = self
            .buffers
            .get(&id)
            .ok_or_else(|| Error::unsupported("buffer not found"))?
            .resource
            .clone();
        upload_bytes_at(&resource, offset, data)
    }

    pub(crate) fn create_buffer_resource(
        &self,
        size: u64,
        heap_type: D3D12_HEAP_TYPE,
        flags: D3D12_RESOURCE_FLAGS,
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
            Flags: flags,
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

    pub(crate) fn clear_target(
        &mut self,
        resource: &ID3D12Resource,
        rtv: D3D12_CPU_DESCRIPTOR_HANDLE,
    ) -> Result<(), Error> {
        self.begin()?;
        self.transition(
            resource,
            D3D12_RESOURCE_STATE_COMMON,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        )?;
        unsafe {
            self.list
                .ClearRenderTargetView(rtv, &[0.0, 0.0, 0.0, 0.0], None);
        }
        self.transition(
            resource,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_COMMON,
        )?;
        self.submit()
    }

    pub(crate) fn upload_default_buffer(
        &mut self,
        resource: &ID3D12Resource,
        data: &[u8],
    ) -> Result<(), Error> {
        let staging = self.create_buffer_resource(
            data.len() as u64,
            D3D12_HEAP_TYPE_UPLOAD,
            D3D12_RESOURCE_FLAG_NONE,
        )?;
        upload_bytes(&staging, data)?;
        self.begin()?;
        self.transition(
            resource,
            D3D12_RESOURCE_STATE_COMMON,
            D3D12_RESOURCE_STATE_COPY_DEST,
        )?;
        unsafe {
            self.list
                .CopyBufferRegion(resource, 0, &staging, 0, data.len() as u64);
        }
        self.transition(
            resource,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_COMMON,
        )?;
        self.submit()
    }
}

pub(crate) fn heap(kind: D3D12_HEAP_TYPE) -> D3D12_HEAP_PROPERTIES {
    D3D12_HEAP_PROPERTIES {
        Type: kind,
        ..Default::default()
    }
}
pub(crate) fn upload_bytes(resource: &ID3D12Resource, data: &[u8]) -> Result<(), Error> {
    upload_bytes_at(resource, 0, data)
}
pub(crate) fn upload_bytes_at(
    resource: &ID3D12Resource,
    offset: u64,
    data: &[u8],
) -> Result<(), Error> {
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
pub(crate) fn texture_location(
    resource: &ID3D12Resource,
    offset: u64,
) -> D3D12_TEXTURE_COPY_LOCATION {
    D3D12_TEXTURE_COPY_LOCATION {
        pResource: std::mem::ManuallyDrop::new(Some(resource.clone())),
        Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
        Anonymous: unsafe { std::mem::zeroed() },
    }
    .with_offset(offset)
}
pub(crate) trait WithOffset {
    fn with_offset(self, _offset: u64) -> Self;
}
impl WithOffset for D3D12_TEXTURE_COPY_LOCATION {
    fn with_offset(self, _offset: u64) -> Self {
        self
    }
}

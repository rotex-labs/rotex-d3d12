use super::format::*;
use super::prelude::*;
use super::resources::upload_bytes_at;
use super::types::*;

impl super::device::Device {
    pub(crate) fn ensure_root(&mut self) -> Result<(), Error> {
        if self.root_bound {
            return Ok(());
        }
        unsafe {
            if self.compute_bound {
                if let Some(id) = self.bound_compute {
                    let pipeline = self.compute_pipelines.get(&id).ok_or_else(|| {
                        Error::unsupported("compute pipeline not found")
                    })?;
                    self.list
                        .SetComputeRootSignature(&pipeline.root_signature);
                } else {
                    self.list.SetComputeRootSignature(&self.root_signature);
                }
            } else if let Some(id) = self.bound_graphics {
                let pipeline = self
                    .pipelines
                    .get(&id)
                    .ok_or_else(|| Error::unsupported("graphics pipeline not found"))?;
                self.list
                    .SetGraphicsRootSignature(&pipeline.root_signature);
            } else {
                self.list.SetGraphicsRootSignature(&self.root_signature);
            }
        }
        self.root_bound = true;
        Ok(())
    }

    pub(crate) fn record(&mut self, command: &RhiCommand) -> Result<(), Error> {
        match command {
            RhiCommand::BeginFrame { .. } => self.begin(),
            RhiCommand::SubmitFrame { present } => self.finish_frame(*present),
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
                let (rtv, width, height) = match pass.color_target {
                    PassColorTarget::Texture(id) => {
                        let texture = self
                            .textures
                            .get(&id)
                            .ok_or_else(|| Error::unsupported("render target texture not found"))?;
                        let Some(rtv) = texture.rtv else {
                            return Err(Error::unsupported("texture is not a render target"));
                        };
                        let resource = texture.resource.clone();
                        let before = texture.state;
                        let width = texture.width;
                        let height = texture.height;
                        self.transition(&resource, before, D3D12_RESOURCE_STATE_RENDER_TARGET)?;
                        if let Some(texture) = self.textures.get_mut(&id) {
                            texture.state = D3D12_RESOURCE_STATE_RENDER_TARGET;
                        }
                        self.back_index = None;
                        (rtv, width, height)
                    }
                    PassColorTarget::Presentation => {
                        let index = self
                            .swap
                            .as_ref()
                            .ok_or_else(|| Error::unsupported("swapchain missing"))?
                            .chain_index()?;
                        let swap = self
                            .swap
                            .as_ref()
                            .ok_or_else(|| Error::unsupported("swapchain missing"))?;
                        let resource = swap.buffers[index].clone();
                        let before = swap.states[index];
                        let rtv = swap.rtvs[index];
                        self.transition(&resource, before, D3D12_RESOURCE_STATE_RENDER_TARGET)?;
                        if let Some(swap) = self.swap.as_mut() {
                            swap.states[index] = D3D12_RESOURCE_STATE_RENDER_TARGET;
                        }
                        self.back_index = Some(index);
                        (rtv, self.extent.width, self.extent.height)
                    }
                };
                self.set_viewport(width, height);
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
                let count = (data.len() / 4) as u32;
                if count == 0 {
                    return Ok(());
                }
                self.ensure_root()?;
                let compute = self.compute_bound;
                unsafe {
                    if compute {
                        self.list.SetComputeRoot32BitConstants(
                            0,
                            count,
                            data.as_ptr().cast(),
                            *offset / 4,
                        );
                    } else {
                        self.list.SetGraphicsRoot32BitConstants(
                            0,
                            count,
                            data.as_ptr().cast(),
                            *offset / 4,
                        );
                    }
                }
                Ok(())
            }
            RhiCommand::SetGraphicsPipeline { pipeline } => {
                self.begin()?;
                let id = *pipeline;
                let topology = {
                    let pipeline = self
                        .pipelines
                        .get(&id)
                        .ok_or_else(|| Error::unsupported("graphics pipeline not found"))?;
                    let topology = pipeline.topology;
                    unsafe {
                        self.list
                            .SetGraphicsRootSignature(&pipeline.root_signature);
                        self.list.SetPipelineState(&pipeline.state);
                        self.list.IASetPrimitiveTopology(topology);
                    }
                    topology
                };
                let _ = topology;
                self.bound_graphics = Some(id);
                self.bound_compute = None;
                self.compute_bound = false;
                self.root_bound = true;
                Ok(())
            }
            RhiCommand::SetComputePipeline { pipeline } => {
                self.begin()?;
                let id = *pipeline;
                {
                    let pipeline = self
                        .compute_pipelines
                        .get(&id)
                        .ok_or_else(|| Error::unsupported("compute pipeline not found"))?;
                    unsafe {
                        self.list
                            .SetComputeRootSignature(&pipeline.root_signature);
                        self.list.SetPipelineState(&pipeline.state);
                    }
                }
                self.bound_compute = Some(id);
                self.bound_graphics = None;
                self.compute_bound = true;
                self.root_bound = true;
                Ok(())
            }
            RhiCommand::BindGroups { first_slot, bind_groups, .. } => {
                self.bind_groups(*first_slot, bind_groups)
            }
            RhiCommand::Draw {
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
            } => {
                self.begin()?;
                unsafe {
                    self.list.DrawInstanced(
                        *vertex_count,
                        *instance_count,
                        *first_vertex,
                        *first_instance,
                    );
                }
                Ok(())
            }
            RhiCommand::BindVertexBuffer {
                buffer,
                binding,
                offset,
            } => self.bind_vertex(*buffer, *binding, *offset),
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

    pub(crate) fn barrier(
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
            rotex_types::BarrierResource::Buffer(id) => {
                let buffer = self
                    .buffers
                    .get(id)
                    .ok_or_else(|| Error::unsupported("buffer not found"))?;
                if buffer.state == D3D12_RESOURCE_STATE_GENERIC_READ {
                    return Ok(());
                }
                let before = buffer.state;
                let resource = buffer.resource.clone();
                self.transition(&resource, before, after)?;
                if let Some(buffer) = self.buffers.get_mut(id) {
                    buffer.state = after;
                }
                Ok(())
            }
        }
    }

    pub(crate) fn transition(
        &mut self,
        resource: &ID3D12Resource,
        before: D3D12_RESOURCE_STATES,
        after: D3D12_RESOURCE_STATES,
    ) -> Result<(), Error> {
        if before == after {
            return Ok(());
        }
        self.begin()?;
        let mut barrier = D3D12_RESOURCE_BARRIER {
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
            self.list.ResourceBarrier(std::slice::from_ref(&barrier));
            // The windows binding never drops this COM pointer. Leaving the
            // clone alive adds a reference on every barrier, and ResizeBuffers
            // then fails because the back buffers are still held.
            std::mem::ManuallyDrop::drop(&mut (*barrier.Anonymous.Transition).pResource);
        }
        Ok(())
    }

    pub(crate) fn begin(&mut self) -> Result<(), Error> {
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
            self.list
                .SetDescriptorHeaps(&[Some(self.srv_heap.clone())]);
        }
        self.recording = true;
        self.root_bound = false;
        Ok(())
    }

    pub(crate) fn wait_value(&self, value: u64) -> Result<(), Error> {
        unsafe {
            self.fence
                .SetEventOnCompletion(value, self.fence_event)
                .map_err(|err| {
                    Error::fatal(rotex_core::ErrorKind::Backend(format!(
                        "fence wait failed: {err}"
                    )))
                })?;
            WaitForSingleObject(self.fence_event, 0xFFFF_FFFF);
        }
        Ok(())
    }

    pub(crate) fn wait_idle(&mut self) -> Result<(), Error> {
        self.submit()?;
        unsafe {
            self.fence_value += 1;
            self.queue
                .Signal(&self.fence, self.fence_value)
                .map_err(|err| {
                    Error::fatal(rotex_core::ErrorKind::Backend(format!(
                        "queue idle signal failed: {err}"
                    )))
                })?;
        }
        self.wait_value(self.fence_value)
    }

    pub(crate) fn release_list_references(&mut self) -> Result<(), Error> {
        unsafe {
            self.allocator.Reset().map_err(|err| {
                Error::fatal(rotex_core::ErrorKind::Backend(format!(
                    "command allocator reset failed: {err}"
                )))
            })?;
            self.list.Reset(&self.allocator, None).map_err(|err| {
                Error::fatal(rotex_core::ErrorKind::Backend(format!(
                    "command list reset failed: {err}"
                )))
            })?;
            self.list.Close().map_err(|err| {
                Error::fatal(rotex_core::ErrorKind::Backend(format!(
                    "command list close failed: {err}"
                )))
            })?;
        }
        self.recording = false;
        Ok(())
    }

    pub(crate) fn submit(&mut self) -> Result<(), Error> {
        if !self.recording {
            return Ok(());
        }
        unsafe {
            self.list
                .Close()
                .map_err(|_| Error::unsupported("command list close failed"))?;
            let lists = [Some(self.cmd_list.clone())];
            self.queue.ExecuteCommandLists(&lists);
            self.fence_value += 1;
            self.queue
                .Signal(&self.fence, self.fence_value)
                .map_err(|_| Error::unsupported("fence signal failed"))?;
        }
        let value = self.fence_value;
        self.recording = false;
        self.wait_value(value)
    }

    pub(crate) fn finish_frame(&mut self, present: bool) -> Result<(), Error> {
        if present {
            if let Some(index) = self.back_index {
                let resource = self
                    .swap
                    .as_ref()
                    .ok_or_else(|| Error::unsupported("swapchain missing"))?
                    .buffers[index]
                    .clone();
                let before = self
                    .swap
                    .as_ref()
                    .ok_or_else(|| Error::unsupported("swapchain missing"))?
                    .states[index];
                self.transition(&resource, before, D3D12_RESOURCE_STATE_PRESENT)?;
                if let Some(swap) = self.swap.as_mut() {
                    swap.states[index] = D3D12_RESOURCE_STATE_PRESENT;
                }
                self.back_index = None;
            }
        }
        self.submit()?;
        if present {
            if let Some(swap) = &self.swap {
                unsafe {
                    swap.chain
                        .Present(1, DXGI_PRESENT(0))
                        .ok()
                        .map_err(|_| Error::unsupported("Present failed"))?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn set_viewport(&self, width: u32, height: u32) {
        let width = width.max(1) as f32;
        let height = height.max(1) as f32;
        let viewport = D3D12_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: height,
            Width: width,
            Height: -height,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        let scissor = RECT {
            left: 0,
            top: 0,
            right: width as i32,
            bottom: height as i32,
        };
        unsafe {
            self.list.RSSetViewports(&[viewport]);
            self.list.RSSetScissorRects(&[scissor]);
        }
    }

    pub(crate) fn bind_groups(&mut self, first_slot: u32, bind_groups: &[BindGroupId]) -> Result<(), Error> {
        self.begin()?;
        self.ensure_root()?;
        let compute = self.compute_bound;
        let mut views = [ViewBinding {
            root_param: 0,
            group: 0,
            slot: 0,
            uav: false,
            table: false,
        }; 16];
        let view_count = if compute {
            let id = self
                .bound_compute
                .ok_or_else(|| Error::unsupported("compute pipeline not found"))?;
            let pipeline = self
                .compute_pipelines
                .get(&id)
                .ok_or_else(|| Error::unsupported("compute pipeline not found"))?;
            copy_prefix(&pipeline.views, &mut views)?
        } else {
            let id = self
                .bound_graphics
                .ok_or_else(|| Error::unsupported("graphics pipeline not found"))?;
            let pipeline = self
                .pipelines
                .get(&id)
                .ok_or_else(|| Error::unsupported("graphics pipeline not found"))?;
            copy_prefix(&pipeline.views, &mut views)?
        };
        for (index, group_id) in bind_groups.iter().enumerate() {
            let set = first_slot + index as u32;
            let mut slots = [GroupSlot {
                binding: 0,
                buffer: None,
                offset: 0,
                texture: None,
            }; 16];
            let (group_set, slot_count) = {
                let group = self
                    .groups
                    .get(group_id)
                    .ok_or_else(|| Error::unsupported("bind group not found"))?;
                if group.set != set && group.set != 0 {
                    continue;
                }
                (group.set, copy_prefix(&group.slots, &mut slots)?)
            };
            for view in views[..view_count]
                .iter()
                .filter(|view| view.group == group_set || view.group == set)
            {
                let Some(slot) = slots[..slot_count]
                    .iter()
                    .find(|slot| slot.binding == view.slot)
                else {
                    continue;
                };
                if view.table {
                    let Some(texture_id) = slot.texture else {
                        continue;
                    };
                    self.bind_texture_table(view.root_param, texture_id)?;
                } else if let Some(buffer_id) = slot.buffer {
                    self.bind_buffer_view(*view, buffer_id, slot.offset, compute)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn bind_texture_table(
        &mut self,
        root_param: u32,
        texture_id: TextureId,
    ) -> Result<(), Error> {
        let (before, existing) = {
            let texture = self
                .textures
                .get(&texture_id)
                .ok_or_else(|| Error::unsupported("texture not found"))?;
            (texture.state, texture.srv)
        };
        if before != D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE {
            let resource = self
                .textures
                .get(&texture_id)
                .ok_or_else(|| Error::unsupported("texture not found"))?
                .resource
                .clone();
            self.transition(
                &resource,
                before,
                D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
            )?;
            if let Some(texture) = self.textures.get_mut(&texture_id) {
                texture.state = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
            }
        }
        let index = if let Some(index) = existing {
            index
        } else {
            if self.srv_cursor >= 128 {
                return Err(Error::unsupported("shader resource heap is full"));
            }
            let index = self.srv_cursor;
            self.srv_cursor += 1;
            let cpu = self.srv_cpu(index);
            let resource = &self
                .textures
                .get(&texture_id)
                .ok_or_else(|| Error::unsupported("texture not found"))?
                .resource;
            unsafe {
                self.device
                    .CreateShaderResourceView(resource, None, cpu);
            }
            if let Some(texture) = self.textures.get_mut(&texture_id) {
                texture.srv = Some(index);
            }
            index
        };
        let gpu = self.srv_gpu(index);
        unsafe {
            self.list.SetGraphicsRootDescriptorTable(root_param, gpu);
        }
        Ok(())
    }

    pub(crate) fn bind_buffer_view(
        &mut self,
        view: ViewBinding,
        buffer_id: BufferId,
        offset: u64,
        compute: bool,
    ) -> Result<(), Error> {
        let (before, address) = {
            let buffer = self
                .buffers
                .get(&buffer_id)
                .ok_or_else(|| Error::unsupported("buffer not found"))?;
            (buffer.state, buffer.gpu_address)
        };
        let after = if before == D3D12_RESOURCE_STATE_GENERIC_READ {
            before
        } else if view.uav {
            D3D12_RESOURCE_STATE_UNORDERED_ACCESS
        } else {
            D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE
        };
        if before != after {
            let resource = self
                .buffers
                .get(&buffer_id)
                .ok_or_else(|| Error::unsupported("buffer not found"))?
                .resource
                .clone();
            self.transition(&resource, before, after)?;
            if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
                buffer.state = after;
            }
        }
        let address = address + offset;
        unsafe {
            if compute {
                if view.uav {
                    self.list
                        .SetComputeRootUnorderedAccessView(view.root_param, address);
                } else {
                    self.list
                        .SetComputeRootShaderResourceView(view.root_param, address);
                }
            } else if view.uav {
                self.list
                    .SetGraphicsRootUnorderedAccessView(view.root_param, address);
            } else {
                self.list
                    .SetGraphicsRootShaderResourceView(view.root_param, address);
            }
        }
        Ok(())
    }

    pub(crate) fn bind_vertex(&mut self, buffer: BufferId, binding: u32, offset: u64) -> Result<(), Error> {
        self.begin()?;
        let (size, state, address) = {
            let buffer = self
                .buffers
                .get(&buffer)
                .ok_or_else(|| Error::unsupported("vertex buffer not found"))?;
            (buffer.size, buffer.state, buffer.gpu_address)
        };
        if state != D3D12_RESOURCE_STATE_GENERIC_READ {
            let after = D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER
                | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE;
            if state != after {
                let resource = self
                    .buffers
                    .get(&buffer)
                    .ok_or_else(|| Error::unsupported("vertex buffer not found"))?
                    .resource
                    .clone();
                self.transition(&resource, state, after)?;
                if let Some(found) = self.buffers.get_mut(&buffer) {
                    found.state = after;
                }
            }
        }
        let stride = self
            .bound_graphics
            .and_then(|id| self.pipelines.get(&id))
            .and_then(|pipeline| pipeline.strides.get(binding as usize))
            .copied()
            .unwrap_or(16);
        let view = D3D12_VERTEX_BUFFER_VIEW {
            BufferLocation: address + offset,
            SizeInBytes: size.saturating_sub(offset) as u32,
            StrideInBytes: stride,
        };
        unsafe {
            self.list.IASetVertexBuffers(binding, Some(&[view]));
        }
        Ok(())
    }

}

use super::debug::*;
use super::format::*;
use super::prelude::*;
use super::types::*;

pub(crate) fn empty_root_signature(device: &ID3D12Device) -> Result<ID3D12RootSignature, Error> {
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
pub(crate) fn indexed_indirect_signature(
    device: &ID3D12Device,
) -> Result<ID3D12CommandSignature, Error> {
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
type GraphicsPipelineParts = (
    ID3D12PipelineState,
    ID3D12RootSignature,
    Vec<ViewBinding>,
    D3D_PRIMITIVE_TOPOLOGY,
    Vec<u32>,
);

pub(crate) fn create_graphics_pipeline(
    device: &ID3D12Device,
    pipeline: &rotex_types::GraphicsPipelineDescriptor,
) -> Result<GraphicsPipelineParts, Error> {
    let vs = dxil(&pipeline.shaders.vertex)
        .ok_or_else(|| Error::unsupported("vertex shader has no DXIL payload"))?;
    let ps = dxil(&pipeline.shaders.fragment)
        .ok_or_else(|| Error::unsupported("fragment shader has no DXIL payload"))?;
    let (root, views) = graphics_root_signature(device, &pipeline.shaders.layout)?;
    let (elements, strides) = input_layout(&pipeline.vertex_buffers);
    let (topology_type, topology) = primitive_topology(pipeline.topology);
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
        desc.InputLayout = D3D12_INPUT_LAYOUT_DESC {
            pInputElementDescs: if elements.is_empty() {
                std::ptr::null()
            } else {
                elements.as_ptr()
            },
            NumElements: elements.len() as u32,
        };
        let blend = &mut desc.BlendState.RenderTarget[0];
        blend.RenderTargetWriteMask = 0x0f;
        let blend_on = pipeline.blend.enabled || pipeline.blend.mode != BlendMode::Replace;
        blend.BlendEnable = blend_on.into();
        blend.BlendOp = D3D12_BLEND_OP_ADD;
        blend.BlendOpAlpha = D3D12_BLEND_OP_ADD;
        match pipeline.blend.mode {
            BlendMode::Additive => {
                blend.SrcBlend = D3D12_BLEND_ONE;
                blend.DestBlend = D3D12_BLEND_ONE;
                blend.SrcBlendAlpha = D3D12_BLEND_ONE;
                blend.DestBlendAlpha = D3D12_BLEND_ONE;
            }
            BlendMode::Alpha => {
                blend.SrcBlend = D3D12_BLEND_SRC_ALPHA;
                blend.DestBlend = D3D12_BLEND_INV_SRC_ALPHA;
                blend.SrcBlendAlpha = D3D12_BLEND_ONE;
                blend.DestBlendAlpha = D3D12_BLEND_INV_SRC_ALPHA;
            }
            BlendMode::Replace => {
                blend.SrcBlend = D3D12_BLEND_ONE;
                blend.DestBlend = D3D12_BLEND_ZERO;
                blend.SrcBlendAlpha = D3D12_BLEND_ONE;
                blend.DestBlendAlpha = D3D12_BLEND_ZERO;
            }
        }
        desc.SampleMask = u32::MAX;
        desc.RasterizerState.FillMode = D3D12_FILL_MODE_SOLID;
        desc.RasterizerState.CullMode = match pipeline.cull_mode {
            CullMode::None => D3D12_CULL_MODE_NONE,
            CullMode::Front => D3D12_CULL_MODE_FRONT,
            CullMode::Back => D3D12_CULL_MODE_BACK,
        };
        desc.RasterizerState.DepthClipEnable = true.into();
        desc.DepthStencilState.DepthEnable = pipeline.depth_enabled.into();
        desc.DepthStencilState.DepthWriteMask = if pipeline.depth_write {
            D3D12_DEPTH_WRITE_MASK_ALL
        } else {
            D3D12_DEPTH_WRITE_MASK_ZERO
        };
        desc.DepthStencilState.DepthFunc = D3D12_COMPARISON_FUNC_LESS;
        desc.DepthStencilState.StencilReadMask = 0xff;
        desc.DepthStencilState.StencilWriteMask = 0xff;
        let stencil = D3D12_DEPTH_STENCILOP_DESC {
            StencilFailOp: D3D12_STENCIL_OP_KEEP,
            StencilDepthFailOp: D3D12_STENCIL_OP_KEEP,
            StencilPassOp: D3D12_STENCIL_OP_KEEP,
            StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS,
        };
        desc.DepthStencilState.FrontFace = stencil;
        desc.DepthStencilState.BackFace = stencil;
        desc.PrimitiveTopologyType = topology_type;
        desc.NumRenderTargets = 1;
        desc.RTVFormats[0] = pipeline
            .targets
            .first()
            .map(|format| dxgi_format(*format))
            .unwrap_or(DXGI_FORMAT_R8G8B8A8_UNORM);
        desc.SampleDesc.Count = 1;
        if debug_requested() {
            clear_debug_messages(device);
        }
        let state = device.CreateGraphicsPipelineState(&desc);
        std::mem::ManuallyDrop::drop(&mut desc.pRootSignature);
        let state = state.map_err(|err| {
            Error::fatal(rotex_core::ErrorKind::Backend(format!(
                "CreateGraphicsPipelineState failed: {err} | {}",
                debug_messages(device)
            )))
        })?;
        Ok((state, root, views, topology, strides))
    }
}
pub(crate) fn create_compute_pipeline(
    device: &ID3D12Device,
    pipeline: &rotex_types::ComputePipelineDescriptor,
) -> Result<(ID3D12PipelineState, ID3D12RootSignature, Vec<ViewBinding>), Error> {
    let dxil = pipeline
        .dxil
        .as_deref()
        .filter(|bytes| !bytes.is_empty())
        .ok_or_else(|| Error::unsupported("compute shader has no DXIL payload"))?;
    let (root, views) = compute_root_signature(device, pipeline)?;
    unsafe {
        let mut desc = D3D12_COMPUTE_PIPELINE_STATE_DESC {
            pRootSignature: std::mem::ManuallyDrop::new(Some(root.clone())),
            CS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: dxil.as_ptr().cast(),
                BytecodeLength: dxil.len(),
            },
            NodeMask: 0,
            CachedPSO: std::mem::zeroed(),
            Flags: D3D12_PIPELINE_STATE_FLAG_NONE,
        };
        let state = device.CreateComputePipelineState(&desc);
        std::mem::ManuallyDrop::drop(&mut desc.pRootSignature);
        let state = state.map_err(|_| Error::unsupported("CreateComputePipelineState failed"))?;
        Ok((state, root, views))
    }
}
pub(crate) fn graphics_root_signature(
    device: &ID3D12Device,
    layout: &rotex_types::AbstractPipelineLayout,
) -> Result<(ID3D12RootSignature, Vec<ViewBinding>), Error> {
    let mut tables = Vec::new();
    let mut buffers = Vec::new();
    let mut srv_register = 0u32;
    let mut uav_register = 0u32;
    let mut has_sampler = false;
    for group in &layout.bind_groups {
        for entry in &group.entries {
            match entry.ty {
                BindingType::Sampler => has_sampler = true,
                BindingType::SampledTexture | BindingType::CombinedImageSampler => {
                    tables.push((
                        group.set,
                        entry.binding,
                        D3D12_DESCRIPTOR_RANGE {
                            RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
                            NumDescriptors: 1,
                            BaseShaderRegister: srv_register,
                            RegisterSpace: 0,
                            OffsetInDescriptorsFromTableStart: D3D12_DESCRIPTOR_RANGE_OFFSET_APPEND,
                        },
                    ));
                    srv_register += 1;
                }
                BindingType::StorageBuffer | BindingType::StorageTexture => {
                    let uav = entry.ty == BindingType::StorageTexture || !entry.readonly;
                    let register = if uav {
                        let register = uav_register;
                        uav_register += 1;
                        register
                    } else {
                        let register = srv_register;
                        srv_register += 1;
                        register
                    };
                    buffers.push((group.set, entry.binding, uav, register));
                }
                _ => {}
            }
        }
    }
    let constant_count = layout
        .push_constants
        .iter()
        .map(|range| range.size / 4)
        .max()
        .unwrap_or(0);
    let sampler = static_linear_sampler();
    unsafe {
        let mut parameters = Vec::new();
        if constant_count > 0 {
            parameters.push(root_constants(constant_count));
        }
        let mut views = Vec::new();
        for (group, slot, range) in &tables {
            let root_param = parameters.len() as u32;
            parameters.push(D3D12_ROOT_PARAMETER {
                ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
                Anonymous: D3D12_ROOT_PARAMETER_0 {
                    DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                        NumDescriptorRanges: 1,
                        pDescriptorRanges: range,
                    },
                },
                ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
            });
            views.push(ViewBinding {
                root_param,
                group: *group,
                slot: *slot,
                uav: false,
                table: true,
            });
        }
        for (group, slot, uav, register) in buffers {
            let root_param = parameters.len() as u32;
            parameters.push(D3D12_ROOT_PARAMETER {
                ParameterType: if uav {
                    D3D12_ROOT_PARAMETER_TYPE_UAV
                } else {
                    D3D12_ROOT_PARAMETER_TYPE_SRV
                },
                Anonymous: D3D12_ROOT_PARAMETER_0 {
                    Descriptor: D3D12_ROOT_DESCRIPTOR {
                        ShaderRegister: register,
                        RegisterSpace: 0,
                    },
                },
                ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
            });
            views.push(ViewBinding {
                root_param,
                group,
                slot,
                uav,
                table: false,
            });
        }
        let samplers = if has_sampler {
            vec![sampler]
        } else {
            Vec::new()
        };
        let root = serialize_root(device, &parameters, &samplers)?;
        Ok((root, views))
    }
}
pub(crate) fn compute_root_signature(
    device: &ID3D12Device,
    pipeline: &rotex_types::ComputePipelineDescriptor,
) -> Result<(ID3D12RootSignature, Vec<ViewBinding>), Error> {
    let mut bindings = pipeline.bindings.clone();
    bindings.sort_by_key(|binding| (binding.set, binding.binding));
    let constant_count = pipeline
        .push_constants
        .iter()
        .map(|range| range.size / 4)
        .max()
        .unwrap_or(0)
        .max(1);
    let mut srv_register = 0u32;
    let mut uav_register = 0u32;
    unsafe {
        let mut parameters = vec![root_constants(constant_count)];
        let mut views = Vec::new();
        for binding in bindings {
            let uav = !binding.readonly;
            let register = if uav {
                let register = uav_register;
                uav_register += 1;
                register
            } else {
                let register = srv_register;
                srv_register += 1;
                register
            };
            let root_param = parameters.len() as u32;
            parameters.push(D3D12_ROOT_PARAMETER {
                ParameterType: if uav {
                    D3D12_ROOT_PARAMETER_TYPE_UAV
                } else {
                    D3D12_ROOT_PARAMETER_TYPE_SRV
                },
                Anonymous: D3D12_ROOT_PARAMETER_0 {
                    Descriptor: D3D12_ROOT_DESCRIPTOR {
                        ShaderRegister: register,
                        RegisterSpace: 0,
                    },
                },
                ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
            });
            views.push(ViewBinding {
                root_param,
                group: binding.set,
                slot: binding.binding,
                uav,
                table: false,
            });
        }
        let root = serialize_root(device, &parameters, &[])?;
        Ok((root, views))
    }
}
pub(crate) unsafe fn root_constants(count: u32) -> D3D12_ROOT_PARAMETER {
    D3D12_ROOT_PARAMETER {
        ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
        Anonymous: D3D12_ROOT_PARAMETER_0 {
            Constants: D3D12_ROOT_CONSTANTS {
                ShaderRegister: 0,
                RegisterSpace: 0,
                Num32BitValues: count.max(1),
            },
        },
        ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL,
    }
}
pub(crate) fn static_linear_sampler() -> D3D12_STATIC_SAMPLER_DESC {
    D3D12_STATIC_SAMPLER_DESC {
        Filter: D3D12_FILTER_MIN_MAG_MIP_LINEAR,
        AddressU: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        AddressV: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        AddressW: D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        MipLODBias: 0.0,
        MaxAnisotropy: 1,
        ComparisonFunc: D3D12_COMPARISON_FUNC_NEVER,
        BorderColor: D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK,
        MinLOD: 0.0,
        MaxLOD: f32::MAX,
        ShaderRegister: 0,
        RegisterSpace: 0,
        ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
    }
}
pub(crate) unsafe fn serialize_root(
    device: &ID3D12Device,
    parameters: &[D3D12_ROOT_PARAMETER],
    samplers: &[D3D12_STATIC_SAMPLER_DESC],
) -> Result<ID3D12RootSignature, Error> {
    unsafe {
        let desc = D3D12_ROOT_SIGNATURE_DESC {
            NumParameters: parameters.len() as u32,
            pParameters: if parameters.is_empty() {
                std::ptr::null()
            } else {
                parameters.as_ptr()
            },
            NumStaticSamplers: samplers.len() as u32,
            pStaticSamplers: if samplers.is_empty() {
                std::ptr::null()
            } else {
                samplers.as_ptr()
            },
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
pub(crate) fn input_layout(
    layouts: &[rotex_types::VertexBufferLayout],
) -> (Vec<D3D12_INPUT_ELEMENT_DESC>, Vec<u32>) {
    let mut elements = Vec::new();
    let mut strides = Vec::new();
    for (slot, layout) in layouts.iter().enumerate() {
        strides.push(layout.array_stride as u32);
        let per_instance = layout.step_mode == VertexStepMode::Instance;
        for attribute in &layout.attributes {
            elements.push(D3D12_INPUT_ELEMENT_DESC {
                SemanticName: s!("TEXCOORD"),
                SemanticIndex: attribute.location,
                Format: vertex_dxgi(attribute.format),
                InputSlot: slot as u32,
                AlignedByteOffset: attribute.offset as u32,
                InputSlotClass: if per_instance {
                    D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA
                } else {
                    D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA
                },
                InstanceDataStepRate: if per_instance { 1 } else { 0 },
            });
        }
    }
    (elements, strides)
}

//! Direct3D 12 translator. This module is compiled only on Windows.

pub(crate) use std::collections::HashMap;
pub(crate) use std::sync::Mutex;

pub(crate) use raw_window_handle::RawWindowHandle;
pub(crate) use rotex_core::Error;
pub(crate) use rotex_types::{
    AdapterCapabilities, AdapterId, AdapterInfo, AdapterQueueSupport, BindGroupId,
    BindGroupLayoutId, BindingType, BlendMode, BufferId, BufferUsage, BufferUsages,
    ComputePipelineId, CreatedResources, CullMode, Extent2D, GpuApiBackend, GraphicsPipelineId,
    HardwareFamily, Limits, PassColorTarget, PrimitiveTopology,
    ResourceBatchCreate, ResourceBatchUpdate, ResourceCreateDescriptor, ResourceHandle,
    ResourceState, ResourceUpdateDescriptor, RhiCommand, SamplerId, ShaderArtifact, ShaderId,
    SurfaceDescriptor, TargetId, TextureFormat, TextureId, TextureReadback, VertexFormat,
    VertexStepMode,
};
pub(crate) use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, RECT};
pub(crate) use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
pub(crate) use windows::Win32::Graphics::Direct3D::{
    D3D_FEATURE_LEVEL_11_0, D3D_PRIMITIVE_TOPOLOGY, D3D_PRIMITIVE_TOPOLOGY_LINELIST,
    D3D_PRIMITIVE_TOPOLOGY_POINTLIST, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
    D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
};
pub(crate) use windows::Win32::Graphics::Direct3D12::*;
pub(crate) use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_R16G16_FLOAT,
    DXGI_FORMAT_R32G32B32A32_FLOAT, DXGI_FORMAT_R32G32B32_FLOAT, DXGI_FORMAT_R32G32_FLOAT,
    DXGI_FORMAT_R32_FLOAT, DXGI_FORMAT_R32_SINT, DXGI_FORMAT_R32_UINT, DXGI_FORMAT_R8G8B8A8_UNORM,
    DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, DXGI_FORMAT_UNKNOWN, DXGI_FORMAT, DXGI_SAMPLE_DESC,
};
pub(crate) use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGIGetDebugInterface1, DXGI_DEBUG_ALL, DXGI_INFO_QUEUE_MESSAGE,
    DXGI_PRESENT, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_DISCARD,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIAdapter1, IDXGIFactory2, IDXGIFactory4, IDXGIInfoQueue,
    IDXGISwapChain3,
};
pub(crate) use windows::core::{Interface, s};

pub(crate) use crate::caps;

pub(crate) const INDIRECT_OPCODE: u32 = 1;


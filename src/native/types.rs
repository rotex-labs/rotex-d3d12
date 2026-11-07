use super::prelude::*;

pub(crate) struct GpuBuffer {
    pub(crate) resource: ID3D12Resource,
    pub(crate) gpu_address: u64,
    pub(crate) size: u64,
    pub(crate) state: D3D12_RESOURCE_STATES,
}
pub(crate) struct GpuTexture {
    pub(crate) resource: ID3D12Resource,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) format: rotex_types::TextureFormat,
    pub(crate) state: D3D12_RESOURCE_STATES,
    pub(crate) rtv: Option<D3D12_CPU_DESCRIPTOR_HANDLE>,
    pub(crate) srv: Option<u32>,
}
#[derive(Clone, Copy)]
pub(crate) struct ViewBinding {
    pub(crate) root_param: u32,
    pub(crate) group: u32,
    pub(crate) slot: u32,
    pub(crate) uav: bool,
    pub(crate) table: bool,
}
pub(crate) struct GpuPipeline {
    pub(crate) state: ID3D12PipelineState,
    pub(crate) root_signature: ID3D12RootSignature,
    pub(crate) views: Vec<ViewBinding>,
    pub(crate) topology: D3D_PRIMITIVE_TOPOLOGY,
    pub(crate) strides: Vec<u32>,
}
pub(crate) struct GpuCompute {
    pub(crate) state: ID3D12PipelineState,
    pub(crate) root_signature: ID3D12RootSignature,
    pub(crate) views: Vec<ViewBinding>,
}
#[derive(Clone, Copy)]
pub(crate) struct GroupSlot {
    pub(crate) binding: u32,
    pub(crate) buffer: Option<BufferId>,
    pub(crate) offset: u64,
    pub(crate) texture: Option<TextureId>,
}
#[derive(Clone)]
pub(crate) struct GpuGroup {
    pub(crate) set: u32,
    pub(crate) slots: Vec<GroupSlot>,
}
pub(crate) struct Swapchain {
    pub(crate) chain: IDXGISwapChain3,
    pub(crate) buffers: Vec<ID3D12Resource>,
    pub(crate) rtvs: Vec<D3D12_CPU_DESCRIPTOR_HANDLE>,
    pub(crate) states: Vec<D3D12_RESOURCE_STATES>,
}

use super::prelude::*;

pub(crate) fn d3d_state(state: ResourceState) -> D3D12_RESOURCE_STATES {
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
pub(crate) fn primitive_topology(
    topology: PrimitiveTopology,
) -> (D3D12_PRIMITIVE_TOPOLOGY_TYPE, D3D_PRIMITIVE_TOPOLOGY) {
    match topology {
        PrimitiveTopology::PointList => (
            D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT,
            D3D_PRIMITIVE_TOPOLOGY_POINTLIST,
        ),
        PrimitiveTopology::LineList => (
            D3D12_PRIMITIVE_TOPOLOGY_TYPE_LINE,
            D3D_PRIMITIVE_TOPOLOGY_LINELIST,
        ),
        PrimitiveTopology::TriangleStrip => (
            D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
        ),
        PrimitiveTopology::TriangleList => (
            D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        ),
    }
}
pub(crate) fn dxgi_format(format: TextureFormat) -> DXGI_FORMAT {
    match format {
        TextureFormat::Rgba16Float => DXGI_FORMAT_R16G16B16A16_FLOAT,
        TextureFormat::Rgba32Float => DXGI_FORMAT_R32G32B32A32_FLOAT,
        TextureFormat::R32Float => DXGI_FORMAT_R32_FLOAT,
        TextureFormat::Rgba8UnormSrgb | TextureFormat::Bgra8UnormSrgb => {
            DXGI_FORMAT_R8G8B8A8_UNORM_SRGB
        }
        _ => DXGI_FORMAT_R8G8B8A8_UNORM,
    }
}
pub(crate) fn vertex_dxgi(format: VertexFormat) -> DXGI_FORMAT {
    match format {
        VertexFormat::Float32 => DXGI_FORMAT_R32_FLOAT,
        VertexFormat::Float32x2 => DXGI_FORMAT_R32G32_FLOAT,
        VertexFormat::Float32x3 => DXGI_FORMAT_R32G32B32_FLOAT,
        VertexFormat::Float32x4 => DXGI_FORMAT_R32G32B32A32_FLOAT,
        VertexFormat::Uint32 => DXGI_FORMAT_R32_UINT,
        VertexFormat::Sint32 => DXGI_FORMAT_R32_SINT,
        VertexFormat::Float16x2 => DXGI_FORMAT_R16G16_FLOAT,
        VertexFormat::Unorm8x4 => DXGI_FORMAT_R8G8B8A8_UNORM,
    }
}
pub(crate) fn copy_prefix<T: Copy>(source: &[T], destination: &mut [T]) -> Result<usize, Error> {
    if source.len() > destination.len() {
        return Err(Error::unsupported("too many bindings"));
    }
    destination[..source.len()].copy_from_slice(source);
    Ok(source.len())
}
pub(crate) fn dxil(package: &rotex_types::ShaderPackage) -> Option<&[u8]> {
    match package.variants.dxil.as_ref() {
        Some(rotex_types::ShaderPayload::Dxil(bytes)) if !bytes.is_empty() => Some(bytes),
        _ => None,
    }
}
pub(crate) fn decode_indirect(payload: &[u8]) -> Result<(u64, u64), Error> {
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

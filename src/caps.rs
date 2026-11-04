use rotex_core::StaticCaps;
use rotex_types::{BackendCapabilities, CoreCaps, FeatureSet, Limits, Support};

use crate::recorder::D3d12Bridge;

#[cfg(windows)]
pub(crate) const CAPS: CoreCaps = CoreCaps {
    compute: Support::Native,
    storage_buffers: Support::Native,
    push_constants: Support::Native,
    barriers: Support::Native,
    queries: Support::Native,
    copies: Support::Native,
    readback: Support::Native,
    render_to_texture: Support::Native,
    dynamic_offsets: Support::Native,
    explicit_pipelines: Support::Native,
};

#[cfg(not(windows))]
pub(crate) const CAPS: CoreCaps = CoreCaps {
    compute: Support::Absent,
    storage_buffers: Support::Absent,
    push_constants: Support::Absent,
    barriers: Support::Absent,
    queries: Support::Absent,
    copies: Support::Absent,
    readback: Support::Absent,
    render_to_texture: Support::Absent,
    dynamic_offsets: Support::Absent,
    explicit_pipelines: Support::Absent,
};

impl StaticCaps for D3d12Bridge {
    const CAPS: CoreCaps = CAPS;
}

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn capabilities() -> BackendCapabilities {
    BackendCapabilities {
        core: CAPS,
        limits: Limits::CORE_MIN,
        extensions: if cfg!(windows) {
            FeatureSet::INDIRECT_DRAW
        } else {
            FeatureSet::empty()
        },
    }
}

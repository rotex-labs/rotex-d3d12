//! Direct3D 12 translator. This module is compiled only on Windows.

mod adapter;
mod commands;
mod debug;
mod device;
mod format;
mod pipeline;
mod prelude;
mod resources;
mod swapchain;
mod types;

pub(crate) use adapter::Adapter;
pub(crate) use device::Device;

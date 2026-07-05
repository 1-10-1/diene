//! Vulkan renderer backend.

#![feature(error_generic_member_access)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::missing_safety_doc)]

/// Vulkan renderer construction and frame operations.
pub mod renderer;

pub use renderer::{VulkanRenderer, VulkanRendererBuilder, VulkanRendererError};

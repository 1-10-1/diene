use ash::vk;
use engine_renderer_api::RenderTransform;
use thiserror::Error;

use crate::renderer::backend::{
    allocator::VulkanAllocator,
    buffer::{VulkanBuffer, VulkanBufferError},
    command::VulkanCommand,
    device::VulkanDevice,
};

#[derive(Debug, Error)]
pub(super) enum VulkanTransformError {
    #[error(transparent)]
    Buffer(#[from] VulkanBufferError),

    #[error("transform buffer device address is null")]
    NullDeviceAddress,

    #[error("transform table must contain at least one transform")]
    EmptyTransforms,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TransformIndex(u32);

impl TransformIndex {
    pub(super) const fn index(self) -> u32 {
        self.0
    }
}

pub(super) struct TransformTable {
    buffer: VulkanBuffer,
    device_address: vk::DeviceAddress,
    transform_count: usize,
}

impl std::fmt::Debug for TransformTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransformTable")
            .field("buffer", &self.buffer)
            .field("device_address", &self.device_address)
            .field("transform_count", &self.transform_count)
            .finish()
    }
}

impl TransformTable {
    pub(super) fn new(
        allocator: &VulkanAllocator,
        command: &VulkanCommand,
        device: &VulkanDevice,
        transforms: &[RenderTransform],
    ) -> core::result::Result<Self, VulkanTransformError> {
        if transforms.is_empty() {
            return Err(VulkanTransformError::EmptyTransforms);
        }

        let buffer = VulkanBuffer::from_staged_bytes(
            device.logical(),
            allocator.handle(),
            command,
            device.graphics_queue(),
            c"transform buffer",
            as_bytes(transforms),
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
        )?;
        let device_address = buffer.device_address(device.logical());

        if device_address == 0 {
            return Err(VulkanTransformError::NullDeviceAddress);
        }

        Ok(Self { buffer, device_address, transform_count: transforms.len() })
    }

    pub(super) fn device_address(&self) -> vk::DeviceAddress {
        self.device_address
    }

    pub(super) fn transform_index(&self, index: usize) -> Option<TransformIndex> {
        (index < self.transform_count)
            .then(|| u32::try_from(index).ok())
            .flatten()
            .map(TransformIndex)
    }
}

fn as_bytes<T>(slice: &[T]) -> &[u8] {
    // SAFETY: Upload data here is POD and copied byte-for-byte into GPU
    // buffers.
    unsafe {
        core::slice::from_raw_parts(slice.as_ptr().cast::<u8>(), core::mem::size_of_val(slice))
    }
}

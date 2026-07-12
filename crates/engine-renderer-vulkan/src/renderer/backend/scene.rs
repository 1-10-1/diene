use std::time::Duration;

use ash::vk;
use thiserror::Error;
use vk_mem::{AllocationCreateFlags, MemoryUsage};

use crate::renderer::backend::{
    allocator::VulkanAllocator,
    buffer::{VulkanBuffer, VulkanBufferError},
    device::VulkanDevice,
};

#[derive(Debug, Error)]
pub(super) enum VulkanSceneError {
    #[error(transparent)]
    Buffer(#[from] VulkanBufferError),

    #[error("scene buffer device address is null")]
    NullDeviceAddress,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug)]
struct SceneData {
    transform_rows: [[f32; 4]; 4],
}

impl SceneData {
    fn from_elapsed(elapsed: Duration) -> Self {
        let (sin, cos) = elapsed.as_secs_f32().sin_cos();

        Self {
            transform_rows: [
                [cos, -sin, 0.0, 0.0],
                [sin, cos, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    fn as_bytes(&self) -> &[u8] {
        // SAFETY: `SceneData` is a repr(C) POD payload mirrored by the shader
        // scene struct.
        unsafe {
            core::slice::from_raw_parts(
                core::ptr::from_ref(self).cast::<u8>(),
                core::mem::size_of::<Self>(),
            )
        }
    }
}

pub(super) struct SceneBuffer {
    buffer: VulkanBuffer,
    device_address: vk::DeviceAddress,
}

impl std::fmt::Debug for SceneBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneBuffer")
            .field("buffer", &self.buffer)
            .field("device_address", &self.device_address)
            .finish()
    }
}

impl SceneBuffer {
    pub(super) fn new(
        allocator: &VulkanAllocator,
        device: &VulkanDevice,
    ) -> core::result::Result<Self, VulkanSceneError> {
        let mut buffer = VulkanBuffer::new(
            device.logical(),
            allocator.handle(),
            c"scene buffer",
            core::mem::size_of::<SceneData>() as vk::DeviceSize,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            MemoryUsage::AutoPreferHost,
            AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE,
        )?;

        buffer.write_bytes(SceneData::from_elapsed(Duration::ZERO).as_bytes())?;

        let device_address = buffer.device_address(device.logical());

        if device_address == 0 {
            return Err(VulkanSceneError::NullDeviceAddress);
        }

        Ok(Self { buffer, device_address })
    }

    pub(super) fn update(
        &mut self,
        elapsed: Duration,
    ) -> core::result::Result<(), VulkanSceneError> {
        self.buffer.write_bytes(SceneData::from_elapsed(elapsed).as_bytes())?;
        Ok(())
    }

    pub(super) fn device_address(&self) -> vk::DeviceAddress {
        self.device_address
    }
}

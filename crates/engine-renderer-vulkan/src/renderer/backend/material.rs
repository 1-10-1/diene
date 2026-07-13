use ash::vk;
use thiserror::Error;
use vk_mem::{AllocationCreateFlags, MemoryUsage};

use crate::renderer::backend::{
    allocator::VulkanAllocator,
    buffer::{VulkanBuffer, VulkanBufferError},
    device::VulkanDevice,
    texture::TextureHandle,
};

#[derive(Debug, Error)]
pub(super) enum VulkanMaterialError {
    #[error(transparent)]
    Buffer(#[from] VulkanBufferError),

    #[error("material buffer device address is null")]
    NullDeviceAddress,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug)]
struct MaterialData {
    albedo_texture_index: u32,
    _padding: [u32; 3],
}

impl MaterialData {
    fn new(albedo: TextureHandle) -> Self {
        Self { albedo_texture_index: albedo.index(), _padding: [0; 3] }
    }

    fn as_bytes(&self) -> &[u8] {
        // SAFETY: `MaterialData` is a repr(C) POD payload mirrored by the
        // shader material struct.
        unsafe {
            core::slice::from_raw_parts(
                core::ptr::from_ref(self).cast::<u8>(),
                core::mem::size_of::<Self>(),
            )
        }
    }
}

pub(super) struct MaterialBuffer {
    buffer: VulkanBuffer,
    device_address: vk::DeviceAddress,
}

impl std::fmt::Debug for MaterialBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialBuffer")
            .field("buffer", &self.buffer)
            .field("device_address", &self.device_address)
            .finish()
    }
}

impl MaterialBuffer {
    pub(super) fn new(
        allocator: &VulkanAllocator,
        device: &VulkanDevice,
        albedo: TextureHandle,
    ) -> core::result::Result<Self, VulkanMaterialError> {
        let mut buffer = VulkanBuffer::new(
            device.logical(),
            allocator.handle(),
            c"material buffer",
            core::mem::size_of::<MaterialData>() as vk::DeviceSize,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            MemoryUsage::AutoPreferHost,
            AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE,
        )?;

        buffer.write_bytes(MaterialData::new(albedo).as_bytes())?;

        let device_address = buffer.device_address(device.logical());

        if device_address == 0 {
            return Err(VulkanMaterialError::NullDeviceAddress);
        }

        Ok(Self { buffer, device_address })
    }

    pub(super) fn device_address(&self) -> vk::DeviceAddress {
        self.device_address
    }
}

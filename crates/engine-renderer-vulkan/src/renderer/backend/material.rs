use ash::vk;
use thiserror::Error;

use crate::renderer::backend::{
    allocator::VulkanAllocator,
    buffer::{VulkanBuffer, VulkanBufferError},
    command::VulkanCommand,
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

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MaterialIndex(u32);

impl MaterialIndex {
    pub(super) const fn index(self) -> u32 {
        self.0
    }
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
}

pub(super) struct MaterialTable {
    buffer: VulkanBuffer,
    device_address: vk::DeviceAddress,
    default_material: MaterialIndex,
}

impl std::fmt::Debug for MaterialTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialTable")
            .field("buffer", &self.buffer)
            .field("device_address", &self.device_address)
            .field("default_material", &self.default_material)
            .finish()
    }
}

impl MaterialTable {
    pub(super) fn new(
        allocator: &VulkanAllocator,
        command: &VulkanCommand,
        device: &VulkanDevice,
        albedo: TextureHandle,
    ) -> core::result::Result<Self, VulkanMaterialError> {
        let materials = [MaterialData::new(albedo)];
        let buffer = VulkanBuffer::from_staged_bytes(
            device.logical(),
            allocator.handle(),
            command,
            device.graphics_queue(),
            c"material buffer",
            as_bytes(&materials),
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
        )?;

        let device_address = buffer.device_address(device.logical());

        if device_address == 0 {
            return Err(VulkanMaterialError::NullDeviceAddress);
        }

        Ok(Self { buffer, device_address, default_material: MaterialIndex(0) })
    }

    pub(super) fn device_address(&self) -> vk::DeviceAddress {
        self.device_address
    }

    pub(super) fn default_material(&self) -> MaterialIndex {
        self.default_material
    }
}

fn as_bytes<T>(slice: &[T]) -> &[u8] {
    // SAFETY: Upload data here is POD and copied byte-for-byte into GPU
    // buffers.
    unsafe {
        core::slice::from_raw_parts(slice.as_ptr().cast::<u8>(), core::mem::size_of_val(slice))
    }
}

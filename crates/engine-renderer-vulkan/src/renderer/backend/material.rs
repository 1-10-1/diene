use ash::vk;
use engine_renderer_api::MaterialData;
use thiserror::Error;

use crate::renderer::backend::{
    allocator::VulkanAllocator,
    buffer::{VulkanBuffer, VulkanBufferError},
    command::VulkanCommand,
    device::VulkanDevice,
    texture::{TextureHandle, VulkanTextureError},
};

#[derive(Debug, Error)]
pub(super) enum VulkanMaterialError {
    #[error(transparent)]
    Buffer(#[from] VulkanBufferError),

    #[error(transparent)]
    Texture(#[from] VulkanTextureError),

    #[error("material buffer device address is null")]
    NullDeviceAddress,

    #[error("material table must contain at least one material")]
    EmptyMaterials,
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
struct GpuMaterialData {
    albedo_texture_index: u32,
    _padding: [u32; 3],
    tint: [f32; 4],
}

impl GpuMaterialData {
    fn new(albedo: TextureHandle, tint: [f32; 4]) -> Self {
        Self { albedo_texture_index: albedo.index(), _padding: [0; 3], tint }
    }
}

pub(super) struct MaterialTable {
    buffer: VulkanBuffer,
    device_address: vk::DeviceAddress,
    default_material: MaterialIndex,
    material_count: usize,
}

impl std::fmt::Debug for MaterialTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialTable")
            .field("buffer", &self.buffer)
            .field("device_address", &self.device_address)
            .field("default_material", &self.default_material)
            .field("material_count", &self.material_count)
            .finish()
    }
}

impl MaterialTable {
    pub(super) fn new(
        allocator: &VulkanAllocator,
        command: &VulkanCommand,
        device: &VulkanDevice,
        texture_heap: &mut crate::renderer::backend::texture::BindlessTextureHeap,
        materials: &[MaterialData],
    ) -> core::result::Result<Self, VulkanMaterialError> {
        if materials.is_empty() {
            return Err(VulkanMaterialError::EmptyMaterials);
        }

        let materials = materials
            .iter()
            .map(|material| {
                let albedo = material
                    .albedo_texture()
                    .map_or(Ok(texture_heap.default_handle()), |texture| {
                        texture_heap.insert(allocator, command, device, texture)
                    })?;

                Ok(GpuMaterialData::new(albedo, material.tint()))
            })
            .collect::<core::result::Result<Vec<_>, VulkanMaterialError>>()?;
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

        Ok(Self {
            buffer,
            device_address,
            default_material: MaterialIndex(0),
            material_count: materials.len(),
        })
    }

    pub(super) fn device_address(&self) -> vk::DeviceAddress {
        self.device_address
    }

    pub(super) fn material_index(&self, index: usize) -> Option<MaterialIndex> {
        (index < self.material_count)
            .then(|| u32::try_from(index).ok())
            .flatten()
            .map(MaterialIndex)
    }
}

fn as_bytes<T>(slice: &[T]) -> &[u8] {
    // SAFETY: Upload data here is POD and copied byte-for-byte into GPU
    // buffers.
    unsafe {
        core::slice::from_raw_parts(slice.as_ptr().cast::<u8>(), core::mem::size_of_val(slice))
    }
}

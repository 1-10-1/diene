use ash::vk;
use thiserror::Error;

use crate::renderer::backend::{
    allocator::VulkanAllocator,
    buffer::{VulkanBuffer, VulkanBufferError},
    command::VulkanCommand,
    device::VulkanDevice,
    material::MaterialIndex,
    mesh::GpuQuadMesh,
};

pub(super) const DRAW_PUSH_CONSTANT_SIZE: u32 = 24;
const DRAW_INDIRECT_COMMAND_SIZE: u32 = 16;

#[derive(Debug, Error)]
pub(super) enum VulkanDrawError {
    #[error(transparent)]
    Buffer(#[from] VulkanBufferError),

    #[error("draw buffer device address is null")]
    NullDeviceAddress,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug)]
struct DrawItem {
    vertices: vk::DeviceAddress,
    indices: vk::DeviceAddress,
    material_index: u32,
    _padding: [u32; 3],
}

impl DrawItem {
    fn new(mesh: &GpuQuadMesh, material: MaterialIndex) -> Self {
        Self {
            vertices: mesh.vertex_address(),
            indices: mesh.index_address(),
            material_index: material.index(),
            _padding: [0; 3],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(super) struct DrawPushConstants {
    draws: vk::DeviceAddress,
    materials: vk::DeviceAddress,
    scene: vk::DeviceAddress,
}

impl DrawPushConstants {
    pub(super) fn as_bytes(&self) -> &[u8] {
        // SAFETY: `Self` is a repr(C) POD push-constant payload containing
        // only device addresses.
        unsafe {
            core::slice::from_raw_parts(
                core::ptr::from_ref(self).cast::<u8>(),
                core::mem::size_of::<Self>(),
            )
        }
    }
}

pub(super) struct GpuDrawList {
    draws: VulkanBuffer,
    indirect: VulkanBuffer,
    draw_address: vk::DeviceAddress,
    draw_count: u32,
}

impl std::fmt::Debug for GpuDrawList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuDrawList")
            .field("draws", &self.draws)
            .field("indirect", &self.indirect)
            .field("draw_address", &self.draw_address)
            .field("draw_count", &self.draw_count)
            .finish()
    }
}

impl GpuDrawList {
    pub(super) fn for_quad(
        allocator: &VulkanAllocator,
        command: &VulkanCommand,
        device: &VulkanDevice,
        mesh: &GpuQuadMesh,
        material: MaterialIndex,
    ) -> core::result::Result<Self, VulkanDrawError> {
        let draw_items = [DrawItem::new(mesh, material)];
        let indirect_commands = [vk::DrawIndirectCommand {
            vertex_count: mesh.index_count(),
            instance_count: 1,
            first_vertex: 0,
            first_instance: 0,
        }];

        let draws = VulkanBuffer::from_staged_bytes(
            device.logical(),
            allocator.handle(),
            command,
            device.graphics_queue(),
            c"draw item buffer",
            as_bytes(&draw_items),
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
        )?;
        let indirect = VulkanBuffer::from_staged_bytes(
            device.logical(),
            allocator.handle(),
            command,
            device.graphics_queue(),
            c"draw indirect buffer",
            as_bytes(&indirect_commands),
            vk::BufferUsageFlags::INDIRECT_BUFFER,
        )?;
        let draw_address = draws.device_address(device.logical());

        if draw_address == 0 {
            return Err(VulkanDrawError::NullDeviceAddress);
        }

        Ok(Self { draws, indirect, draw_address, draw_count: 1 })
    }

    pub(super) fn push_constants(
        &self,
        materials: vk::DeviceAddress,
        scene: vk::DeviceAddress,
    ) -> DrawPushConstants {
        DrawPushConstants { draws: self.draw_address, materials, scene }
    }

    pub(super) fn indirect_buffer(&self) -> vk::Buffer {
        self.indirect.handle()
    }

    pub(super) fn draw_count(&self) -> u32 {
        self.draw_count
    }

    pub(super) fn indirect_stride(&self) -> u32 {
        DRAW_INDIRECT_COMMAND_SIZE
    }
}

fn as_bytes<T>(slice: &[T]) -> &[u8] {
    // SAFETY: Upload data here is POD and copied byte-for-byte into GPU
    // buffers.
    unsafe {
        core::slice::from_raw_parts(slice.as_ptr().cast::<u8>(), core::mem::size_of_val(slice))
    }
}

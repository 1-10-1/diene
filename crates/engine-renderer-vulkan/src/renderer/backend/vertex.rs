use ash::vk;
use thiserror::Error;

use crate::renderer::backend::{
    allocator::VulkanAllocator,
    buffer::{VulkanBuffer, VulkanBufferError},
    command::VulkanCommand,
    device::VulkanDevice,
};

const TRIANGLE_VERTICES: [Vertex; 3] = [
    Vertex { position: [0.0, -0.5, 0.0, 1.0], color: [1.0, 0.0, 0.0, 1.0] },
    Vertex { position: [0.5, 0.5, 0.0, 1.0], color: [0.0, 1.0, 0.0, 1.0] },
    Vertex { position: [-0.5, 0.5, 0.0, 1.0], color: [0.0, 0.0, 1.0, 1.0] },
];

pub(super) const VERTEX_BUFFER_ADDRESS_PUSH_CONSTANT_SIZE: u32 = 8;

#[derive(Debug, Error)]
pub(super) enum VulkanVertexBufferError {
    #[error(transparent)]
    Buffer(#[from] VulkanBufferError),

    #[error("vertex count {count} does not fit u32")]
    VertexCountTooLarge { count: usize },

    #[error("vertex buffer device address is null")]
    NullDeviceAddress,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(super) struct Vertex {
    position: [f32; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(super) struct VertexBufferAddressPushConstants {
    vertices: vk::DeviceAddress,
}

impl VertexBufferAddressPushConstants {
    pub(super) fn as_bytes(&self) -> &[u8] {
        // SAFETY: `Self` is a plain repr(C) POD push-constant payload.
        unsafe {
            core::slice::from_raw_parts(
                core::ptr::from_ref(self).cast::<u8>(),
                core::mem::size_of::<Self>(),
            )
        }
    }
}

pub(super) struct TriangleVertexBuffer {
    buffer: VulkanBuffer,
    device_address: vk::DeviceAddress,
    vertex_count: u32,
}

impl std::fmt::Debug for TriangleVertexBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TriangleVertexBuffer")
            .field("buffer", &self.buffer)
            .field("device_address", &self.device_address)
            .field("vertex_count", &self.vertex_count)
            .finish()
    }
}

impl TriangleVertexBuffer {
    pub(super) fn new(
        allocator: &VulkanAllocator,
        command: &VulkanCommand,
        device: &VulkanDevice,
    ) -> core::result::Result<Self, VulkanVertexBufferError> {
        let vertex_count = u32::try_from(TRIANGLE_VERTICES.len()).map_err(|_| {
            VulkanVertexBufferError::VertexCountTooLarge { count: TRIANGLE_VERTICES.len() }
        })?;

        let bytes = vertices_as_bytes(&TRIANGLE_VERTICES);
        let buffer = VulkanBuffer::from_staged_bytes(
            device.logical(),
            allocator.handle(),
            command,
            device.graphics_queue(),
            c"triangle vertex buffer",
            bytes,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
        )?;

        let device_address = buffer.device_address(device.logical());

        if device_address == 0 {
            return Err(VulkanVertexBufferError::NullDeviceAddress);
        }

        Ok(Self { buffer, device_address, vertex_count })
    }

    pub(super) fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    pub(super) fn push_constants(&self) -> VertexBufferAddressPushConstants {
        VertexBufferAddressPushConstants { vertices: self.device_address }
    }
}

fn vertices_as_bytes(vertices: &[Vertex]) -> &[u8] {
    // SAFETY: `Vertex` is `repr(C)` and contains only plain `f32` arrays,
    // so viewing a contiguous vertex slice as bytes is valid for
    // upload.
    unsafe {
        core::slice::from_raw_parts(
            vertices.as_ptr().cast::<u8>(),
            core::mem::size_of_val(vertices),
        )
    }
}

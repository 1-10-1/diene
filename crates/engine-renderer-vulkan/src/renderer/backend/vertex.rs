use ash::vk;
use thiserror::Error;

use crate::renderer::backend::{
    allocator::VulkanAllocator,
    buffer::{VulkanBuffer, VulkanBufferError},
    command::VulkanCommand,
    device::VulkanDevice,
};

const TRIANGLE_VERTICES: [Vertex; 3] = [
    Vertex { position: [0.0, -0.5], color: [1.0, 0.0, 0.0] },
    Vertex { position: [0.5, 0.5], color: [0.0, 1.0, 0.0] },
    Vertex { position: [-0.5, 0.5], color: [0.0, 0.0, 1.0] },
];

#[derive(Debug, Error)]
pub(super) enum VulkanVertexBufferError {
    #[error(transparent)]
    Buffer(#[from] VulkanBufferError),

    #[error("vertex count {count} does not fit u32")]
    VertexCountTooLarge { count: usize },
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(super) struct Vertex {
    position: [f32; 2],
    color: [f32; 3],
}

impl Vertex {
    #[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
    pub(super) fn binding_descriptions() -> [vk::VertexInputBindingDescription; 1] {
        [vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(core::mem::size_of::<Self>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)]
    }

    #[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
    pub(super) fn attribute_descriptions() -> [vk::VertexInputAttributeDescription; 2] {
        [
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(0)
                .format(vk::Format::R32G32_SFLOAT)
                .offset(core::mem::offset_of!(Self, position) as u32),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(1)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(core::mem::offset_of!(Self, color) as u32),
        ]
    }
}

pub(super) struct TriangleVertexBuffer {
    buffer: VulkanBuffer,
    vertex_count: u32,
}

impl std::fmt::Debug for TriangleVertexBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TriangleVertexBuffer")
            .field("buffer", &self.buffer)
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
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )?;

        Ok(Self { buffer, vertex_count })
    }

    pub(super) fn handle(&self) -> vk::Buffer {
        self.buffer.handle()
    }

    pub(super) fn vertex_count(&self) -> u32 {
        self.vertex_count
    }
}

fn vertices_as_bytes(vertices: &[Vertex]) -> &[u8] {
    // SAFETY: `Vertex` is `repr(C)` and contains only plain `f32` arrays, so viewing a contiguous
    // vertex slice as bytes is valid for upload.
    unsafe {
        core::slice::from_raw_parts(
            vertices.as_ptr().cast::<u8>(),
            core::mem::size_of_val(vertices),
        )
    }
}

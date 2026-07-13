use ash::vk;
use thiserror::Error;

use crate::renderer::backend::{
    allocator::VulkanAllocator,
    buffer::{VulkanBuffer, VulkanBufferError},
    command::VulkanCommand,
    device::VulkanDevice,
};

const QUAD_VERTICES: [Vertex; 4] = [
    Vertex {
        position: [-0.5, -0.5, 0.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        uv: [0.0, 1.0, 0.0, 0.0],
    },
    Vertex {
        position: [0.5, -0.5, 0.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        uv: [1.0, 1.0, 0.0, 0.0],
    },
    Vertex {
        position: [0.5, 0.5, 0.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        uv: [1.0, 0.0, 0.0, 0.0],
    },
    Vertex {
        position: [-0.5, 0.5, 0.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        uv: [0.0, 0.0, 0.0, 0.0],
    },
];

const QUAD_INDICES: [u32; 6] = [0, 1, 2, 2, 3, 0];

#[derive(Debug, Error)]
pub(super) enum VulkanMeshError {
    #[error(transparent)]
    Buffer(#[from] VulkanBufferError),

    #[error("index count {count} does not fit u32")]
    IndexCountTooLarge { count: usize },

    #[error("{buffer} buffer device address is null")]
    NullDeviceAddress { buffer: &'static str },
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug)]
struct Vertex {
    position: [f32; 4],
    color: [f32; 4],
    uv: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(super) struct DrawPushConstants {
    vertices: vk::DeviceAddress,
    indices: vk::DeviceAddress,
    scene: vk::DeviceAddress,
    material: vk::DeviceAddress,
}

pub(super) const DRAW_PUSH_CONSTANT_SIZE: u32 = 32;

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

pub(super) struct GpuQuadMesh {
    vertices: VulkanBuffer,
    indices: VulkanBuffer,
    vertex_address: vk::DeviceAddress,
    index_address: vk::DeviceAddress,
    index_count: u32,
}

impl std::fmt::Debug for GpuQuadMesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuQuadMesh")
            .field("vertices", &self.vertices)
            .field("indices", &self.indices)
            .field("vertex_address", &self.vertex_address)
            .field("index_address", &self.index_address)
            .field("index_count", &self.index_count)
            .finish()
    }
}

impl GpuQuadMesh {
    pub(super) fn new(
        allocator: &VulkanAllocator,
        command: &VulkanCommand,
        device: &VulkanDevice,
    ) -> core::result::Result<Self, VulkanMeshError> {
        let index_count = u32::try_from(QUAD_INDICES.len())
            .map_err(|_| VulkanMeshError::IndexCountTooLarge { count: QUAD_INDICES.len() })?;

        let vertices = VulkanBuffer::from_staged_bytes(
            device.logical(),
            allocator.handle(),
            command,
            device.graphics_queue(),
            c"quad vertex buffer",
            as_bytes(&QUAD_VERTICES),
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
        )?;

        let indices = VulkanBuffer::from_staged_bytes(
            device.logical(),
            allocator.handle(),
            command,
            device.graphics_queue(),
            c"quad index buffer",
            as_bytes(&QUAD_INDICES),
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
        )?;

        let vertex_address =
            require_device_address("vertex", vertices.device_address(device.logical()))?;
        let index_address =
            require_device_address("index", indices.device_address(device.logical()))?;

        Ok(Self { vertices, indices, vertex_address, index_address, index_count })
    }

    pub(super) fn index_count(&self) -> u32 {
        self.index_count
    }

    pub(super) fn push_constants(
        &self,
        scene: vk::DeviceAddress,
        material: vk::DeviceAddress,
    ) -> DrawPushConstants {
        DrawPushConstants {
            vertices: self.vertex_address,
            indices: self.index_address,
            scene,
            material,
        }
    }
}

fn require_device_address(
    buffer: &'static str,
    address: vk::DeviceAddress,
) -> core::result::Result<vk::DeviceAddress, VulkanMeshError> {
    if address == 0 {
        Err(VulkanMeshError::NullDeviceAddress { buffer })
    } else {
        Ok(address)
    }
}

fn as_bytes<T>(slice: &[T]) -> &[u8] {
    // SAFETY: Upload data here is POD and copied byte-for-byte into GPU
    // buffers.
    unsafe {
        core::slice::from_raw_parts(slice.as_ptr().cast::<u8>(), core::mem::size_of_val(slice))
    }
}

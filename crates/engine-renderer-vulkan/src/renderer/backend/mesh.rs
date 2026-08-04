use ash::vk;
use engine_renderer_api::{MeshData, MeshVertex};
use thiserror::Error;

use crate::renderer::backend::{
    allocator::VulkanAllocator,
    buffer::{VulkanBuffer, VulkanBufferError},
    command::VulkanCommand,
    device::VulkanDevice,
};

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
struct GpuMeshVertex {
    position: [f32; 4],
    color: [f32; 4],
    uv: [f32; 4],
}

impl From<MeshVertex> for GpuMeshVertex {
    fn from(vertex: MeshVertex) -> Self {
        let [u, v] = vertex.uv.into();

        Self {
            position: vertex.position.into(),
            color: vertex.color.into(),
            uv: [u, v, 0.0, 0.0],
        }
    }
}

pub(super) struct GpuMesh {
    vertices: VulkanBuffer,
    indices: VulkanBuffer,
    vertex_address: vk::DeviceAddress,
    index_address: vk::DeviceAddress,
    index_count: u32,
}

impl std::fmt::Debug for GpuMesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuMesh")
            .field("vertices", &self.vertices)
            .field("indices", &self.indices)
            .field("vertex_address", &self.vertex_address)
            .field("index_address", &self.index_address)
            .field("index_count", &self.index_count)
            .finish()
    }
}

impl GpuMesh {
    pub(super) fn new(
        allocator: &VulkanAllocator,
        command: &VulkanCommand,
        device: &VulkanDevice,
        data: &MeshData,
    ) -> core::result::Result<Self, VulkanMeshError> {
        let index_count = u32::try_from(data.indices().len())
            .map_err(|_| VulkanMeshError::IndexCountTooLarge { count: data.indices().len() })?;
        let upload_vertices =
            data.vertices().iter().copied().map(GpuMeshVertex::from).collect::<Vec<_>>();

        let vertices = VulkanBuffer::from_staged_bytes(
            device.logical(),
            allocator.handle(),
            command,
            device.graphics_queue(),
            c"mesh vertex buffer",
            as_bytes(&upload_vertices),
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
        )?;

        let indices = VulkanBuffer::from_staged_bytes(
            device.logical(),
            allocator.handle(),
            command,
            device.graphics_queue(),
            c"mesh index buffer",
            as_bytes(data.indices()),
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

    pub(super) fn vertex_address(&self) -> vk::DeviceAddress {
        self.vertex_address
    }

    pub(super) fn index_address(&self) -> vk::DeviceAddress {
        self.index_address
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

#[cfg(test)]
mod tests {
    use engine_renderer_api::{MeshVertex, glm};

    use super::GpuMeshVertex;

    #[test]
    fn gpu_mesh_vertex_matches_shader_layout() {
        let vertex = GpuMeshVertex::from(MeshVertex::new(
            glm::Vec4::new(1.0, 2.0, 3.0, 1.0),
            glm::Vec4::new(0.25, 0.5, 0.75, 1.0),
            glm::Vec2::new(0.2, 0.8),
        ));

        assert_eq!(core::mem::size_of::<GpuMeshVertex>(), 48);
        assert_eq!(core::mem::align_of::<GpuMeshVertex>(), 16);
        assert_f32x4_eq(vertex.position, [1.0, 2.0, 3.0, 1.0]);
        assert_f32x4_eq(vertex.color, [0.25, 0.5, 0.75, 1.0]);
        assert_f32x4_eq(vertex.uv, [0.2, 0.8, 0.0, 0.0]);
    }

    fn assert_f32x4_eq(actual: [f32; 4], expected: [f32; 4]) {
        assert!(
            actual
                .into_iter()
                .zip(expected)
                .all(|(actual, expected)| { (actual - expected).abs() <= f32::EPSILON })
        );
    }
}

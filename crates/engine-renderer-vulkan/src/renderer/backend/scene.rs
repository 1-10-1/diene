use ash::vk::{self};
use engine_renderer_api::{RenderCamera, glm};
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
    view_projection_rows: [[f32; 4]; 4],
}

impl SceneData {
    fn from_camera(extent: vk::Extent2D, camera: &RenderCamera) -> Self {
        let aspect = extent.width.max(1) as f32 / extent.height.max(1) as f32;

        let view = glm::look_at_rh(&camera.eye(), &camera.target(), &camera.up());

        let mut projection = glm::perspective_rh_zo(
            aspect,
            camera.vertical_fov_radians(),
            camera.near_plane(),
            camera.far_plane(),
        );

        projection[(1, 1)] = -projection[(1, 1)];

        let view_projection = projection * view;

        Self {
            view_projection_rows: core::array::from_fn(|row| {
                core::array::from_fn(|column| view_projection[(row, column)])
            }),
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
        let buffer = VulkanBuffer::new(
            device.logical(),
            allocator.handle(),
            c"scene buffer",
            core::mem::size_of::<SceneData>() as vk::DeviceSize,
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            MemoryUsage::AutoPreferHost,
            AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE,
        )?;

        let device_address = buffer.device_address(device.logical());

        if device_address == 0 {
            return Err(VulkanSceneError::NullDeviceAddress);
        }

        Ok(Self { buffer, device_address })
    }

    pub(super) fn write_and_get_address(
        &mut self,
        extent: vk::Extent2D,
        camera: &RenderCamera,
    ) -> core::result::Result<vk::DeviceAddress, VulkanSceneError> {
        self.buffer.write_bytes(SceneData::from_camera(extent, camera).as_bytes())?;

        Ok(self.device_address)
    }
}

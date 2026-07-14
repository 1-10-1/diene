use ash::vk;
use engine_renderer_api::RenderCamera;
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
    fn from_camera(extent: vk::Extent2D, camera: RenderCamera) -> Self {
        let aspect = extent.width.max(1) as f32 / extent.height.max(1) as f32;
        let view = look_at_rh(camera.eye(), camera.target(), camera.up());
        let projection = perspective_rh_zo(
            camera.vertical_fov_radians(),
            aspect,
            camera.near_plane(),
            camera.far_plane(),
        );

        Self { view_projection_rows: mul4x4(projection, view) }
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
        extent: vk::Extent2D,
        camera: RenderCamera,
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

        buffer.write_bytes(SceneData::from_camera(extent, camera).as_bytes())?;

        let device_address = buffer.device_address(device.logical());

        if device_address == 0 {
            return Err(VulkanSceneError::NullDeviceAddress);
        }

        Ok(Self { buffer, device_address })
    }

    pub(super) fn update(
        &mut self,
        extent: vk::Extent2D,
        camera: RenderCamera,
    ) -> core::result::Result<(), VulkanSceneError> {
        self.buffer.write_bytes(SceneData::from_camera(extent, camera).as_bytes())?;
        Ok(())
    }

    pub(super) fn device_address(&self) -> vk::DeviceAddress {
        self.device_address
    }
}

type Mat4 = [[f32; 4]; 4];

fn perspective_rh_zo(fovy: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let focal = 1.0 / (fovy * 0.5).tan();
    let depth = far / (near - far);

    [
        [focal / aspect, 0.0, 0.0, 0.0],
        [0.0, -focal, 0.0, 0.0],
        [0.0, 0.0, depth, near * depth],
        [0.0, 0.0, -1.0, 0.0],
    ]
}

fn look_at_rh(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> Mat4 {
    let backward = normalize(sub3(eye, target));
    let right = normalize(cross3(up, backward));
    let camera_up = cross3(backward, right);

    [
        [right[0], right[1], right[2], -dot3(right, eye)],
        [camera_up[0], camera_up[1], camera_up[2], -dot3(camera_up, eye)],
        [backward[0], backward[1], backward[2], -dot3(backward, eye)],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn mul4x4(left: Mat4, right: Mat4) -> Mat4 {
    let mut out = [[0.0; 4]; 4];

    for row in 0..4 {
        for col in 0..4 {
            out[row][col] = (0..4).map(|i| left[row][i] * right[i][col]).sum();
        }
    }

    out
}

fn sub3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn cross3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [
        left[2].mul_add(-right[1], left[1] * right[2]),
        left[0].mul_add(-right[2], left[2] * right[0]),
        left[1].mul_add(-right[0], left[0] * right[1]),
    ]
}

fn dot3(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[2].mul_add(right[2], left[1].mul_add(right[1], left[0] * right[0]))
}

fn normalize(value: [f32; 3]) -> [f32; 3] {
    let reciprocal_length = dot3(value, value).sqrt().recip();
    [
        value[0] * reciprocal_length,
        value[1] * reciprocal_length,
        value[2] * reciprocal_length,
    ]
}

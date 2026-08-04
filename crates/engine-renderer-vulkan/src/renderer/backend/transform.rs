use ash::vk;
use engine_renderer_api::RenderTransform;
use thiserror::Error;

use crate::renderer::backend::{
    allocator::VulkanAllocator,
    buffer::{VulkanBuffer, VulkanBufferError},
    command::VulkanCommand,
    device::VulkanDevice,
};

#[derive(Debug, Error)]
pub(super) enum VulkanTransformError {
    #[error(transparent)]
    Buffer(#[from] VulkanBufferError),

    #[error("transform buffer device address is null")]
    NullDeviceAddress,

    #[error("transform table must contain at least one transform")]
    EmptyTransforms,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug)]
struct GpuTransform {
    model_rows: [[f32; 4]; 4],
}

impl From<RenderTransform> for GpuTransform {
    fn from(transform: RenderTransform) -> Self {
        let model = transform.model_matrix();
        let model_rows =
            core::array::from_fn(|row| core::array::from_fn(|column| model[(row, column)]));

        Self { model_rows }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TransformIndex(u32);

impl TransformIndex {
    pub(super) const fn index(self) -> u32 {
        self.0
    }
}

pub(super) struct TransformTable {
    buffer: VulkanBuffer,
    device_address: vk::DeviceAddress,
    transform_count: usize,
}

impl std::fmt::Debug for TransformTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransformTable")
            .field("buffer", &self.buffer)
            .field("device_address", &self.device_address)
            .field("transform_count", &self.transform_count)
            .finish()
    }
}

impl TransformTable {
    pub(super) fn new(
        allocator: &VulkanAllocator,
        command: &VulkanCommand,
        device: &VulkanDevice,
        transforms: &[RenderTransform],
    ) -> core::result::Result<Self, VulkanTransformError> {
        if transforms.is_empty() {
            return Err(VulkanTransformError::EmptyTransforms);
        }

        let upload_transforms =
            transforms.iter().copied().map(GpuTransform::from).collect::<Vec<_>>();

        let buffer = VulkanBuffer::from_staged_bytes(
            device.logical(),
            allocator.handle(),
            command,
            device.graphics_queue(),
            c"transform buffer",
            as_bytes(&upload_transforms),
            vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
        )?;
        let device_address = buffer.device_address(device.logical());

        if device_address == 0 {
            return Err(VulkanTransformError::NullDeviceAddress);
        }

        Ok(Self { buffer, device_address, transform_count: transforms.len() })
    }

    pub(super) fn device_address(&self) -> vk::DeviceAddress {
        self.device_address
    }

    pub(super) fn transform_index(&self, index: usize) -> Option<TransformIndex> {
        (index < self.transform_count)
            .then(|| u32::try_from(index).ok())
            .flatten()
            .map(TransformIndex)
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
    use engine_renderer_api::{RenderTransform, glm};

    use super::GpuTransform;

    #[test]
    fn gpu_transform_serializes_glm_matrix_as_shader_rows() {
        let transform = GpuTransform::from(RenderTransform::from_translation_scale(
            glm::Vec3::new(1.0, 2.0, 3.0),
            glm::Vec3::new(4.0, 5.0, 6.0),
        ));

        assert_eq!(core::mem::size_of::<GpuTransform>(), 64);
        assert_eq!(core::mem::align_of::<GpuTransform>(), 16);
        assert_f32x4x4_eq(
            transform.model_rows,
            [
                [4.0, 0.0, 0.0, 1.0],
                [0.0, 5.0, 0.0, 2.0],
                [0.0, 0.0, 6.0, 3.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        );
    }

    fn assert_f32x4x4_eq(actual: [[f32; 4]; 4], expected: [[f32; 4]; 4]) {
        assert!(
            actual
                .into_iter()
                .flatten()
                .zip(expected.into_iter().flatten())
                .all(|(actual, expected)| (actual - expected).abs() <= f32::EPSILON)
        );
    }
}

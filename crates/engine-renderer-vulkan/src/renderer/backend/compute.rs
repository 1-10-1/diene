//! A minimal, self-contained compute pipeline: dispatches a compute
//! shader that writes `index * index` into a storage buffer, then
//! reads the result back and verifies it. This exists to prove out
//! compute pipeline creation, dispatch, and compute-to-transfer
//! synchronization end to end (the tutorial's "Compute Shader"
//! chapter), independent of the graphics render path.

use std::sync::Arc;

use ash::vk;
use thiserror::Error;
use vk_mem::{AllocationCreateFlags, MemoryUsage};

use crate::renderer::backend::{
    allocator::VulkanAllocator,
    buffer::{VulkanBuffer, VulkanBufferError},
    call_error::VulkanCallError,
    command::{VulkanCommand, VulkanCommandError},
    device::VulkanDevice,
    pipeline::{
        VulkanComputePipeline, VulkanComputePipelineBuilder, VulkanPipelineError,
        VulkanPipelineLayout,
    },
    shader::VulkanShader,
};

const ELEMENT_COUNT: u32 = 256;
const WORKGROUP_SIZE: u32 = 64;
const PUSH_CONSTANT_SIZE: u32 = 16;

/// Errors returned while running the compute self-check.
#[derive(Debug, Error)]
pub(super) enum VulkanComputeError {
    #[error(transparent)]
    UnexpectedResult(#[from] VulkanCallError),

    #[error(transparent)]
    Buffer(#[from] VulkanBufferError),

    #[error(transparent)]
    Command(#[from] VulkanCommandError),

    #[error(transparent)]
    Pipeline(#[from] VulkanPipelineError),

    #[error("compute self-check produced element {index} = {actual}, expected {expected}")]
    UnexpectedValue { index: u32, actual: u32, expected: u32 },
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct ComputeDemoPushConstants {
    output: vk::DeviceAddress,
    count: u32,
    _padding: u32,
}

impl ComputeDemoPushConstants {
    fn as_bytes(&self) -> &[u8] {
        // SAFETY: `Self` is a repr(C) POD push-constant payload.
        unsafe {
            core::slice::from_raw_parts(
                core::ptr::from_ref(self).cast::<u8>(),
                core::mem::size_of::<Self>(),
            )
        }
    }
}

/// Builds a compute pipeline from `shader`, dispatches it, and
/// verifies its output. Returns an error (rather than panicking) if
/// pipeline creation, dispatch, or the readback comparison fails,
/// since a broken compute path should fail backend construction
/// rather than silently be ignored.
pub(super) fn run_self_check(
    allocator: &VulkanAllocator,
    command: &VulkanCommand,
    device: &VulkanDevice,
    shader: &VulkanShader,
) -> core::result::Result<(), VulkanComputeError> {
    let pipeline_layout = VulkanPipelineLayout::builder()
        .with_push_constants(PUSH_CONSTANT_SIZE, vk::ShaderStageFlags::COMPUTE)
        .build(device.logical().clone())?;

    let pipeline: VulkanComputePipeline = VulkanComputePipelineBuilder::default()
        .with_shader(shader)
        .build(device, "compute-self-check", &pipeline_layout)?;

    let output = create_buffer(
        allocator.handle(),
        device,
        c"compute self-check output buffer",
        vk::BufferUsageFlags::STORAGE_BUFFER
            | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
            | vk::BufferUsageFlags::TRANSFER_SRC,
        MemoryUsage::AutoPreferDevice,
        AllocationCreateFlags::empty(),
    )?;

    let mut readback = create_buffer(
        allocator.handle(),
        device,
        c"compute self-check readback buffer",
        vk::BufferUsageFlags::TRANSFER_DST,
        MemoryUsage::AutoPreferHost,
        AllocationCreateFlags::HOST_ACCESS_RANDOM,
    )?;

    let push_constants = ComputeDemoPushConstants {
        output: output.device_address(device.logical()),
        count: ELEMENT_COUNT,
        _padding: 0,
    };

    let group_count = ELEMENT_COUNT.div_ceil(WORKGROUP_SIZE);

    command.dispatch_compute_and_readback(
        device.compute_queue(),
        pipeline.get(),
        pipeline_layout.get(),
        push_constants.as_bytes(),
        [group_count, 1, 1],
        output.handle(),
        readback.handle(),
        buffer_size(),
    )?;

    let mut raw = vec![0u8; usize::try_from(buffer_size()).unwrap_or(0)];

    readback.read_bytes(&mut raw)?;

    let (chunks, _remainder) = raw.as_chunks::<4>();

    for (index, chunk) in chunks.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let index = index as u32;

        let actual = u32::from_ne_bytes(*chunk);
        let expected = index * index;

        if actual != expected {
            return Err(VulkanComputeError::UnexpectedValue { index, actual, expected });
        }
    }

    Ok(())
}

fn buffer_size() -> vk::DeviceSize {
    vk::DeviceSize::from(ELEMENT_COUNT) * 4
}

fn create_buffer(
    allocator: Arc<vk_mem::Allocator>,
    device: &VulkanDevice,
    name: &'static std::ffi::CStr,
    usage: vk::BufferUsageFlags,
    memory_usage: MemoryUsage,
    flags: AllocationCreateFlags,
) -> core::result::Result<VulkanBuffer, VulkanComputeError> {
    VulkanBuffer::new(device.logical(), allocator, name, buffer_size(), usage, memory_usage, flags)
        .map_err(VulkanComputeError::from)
}

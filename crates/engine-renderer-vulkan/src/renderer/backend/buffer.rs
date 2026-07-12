use std::sync::Arc;

use ash::vk;
use thiserror::Error;
use vk_mem::{Alloc, Allocation, AllocationCreateFlags, AllocationCreateInfo, MemoryUsage};

use crate::renderer::backend::{
    call_error::VulkanCallError, command::VulkanCommandError, device::VulkanLogicalDevice,
};

#[derive(Debug, Error)]
pub(super) enum VulkanBufferError {
    #[error(transparent)]
    UnexpectedResult(#[from] VulkanCallError),

    #[error(transparent)]
    Command(#[from] VulkanCommandError),

    #[error("buffer size {bytes} does not fit VkDeviceSize")]
    BufferTooLarge { bytes: usize },

    #[error("write of {bytes} bytes exceeds buffer capacity {capacity} bytes")]
    WriteTooLarge { bytes: vk::DeviceSize, capacity: vk::DeviceSize },
}

pub(super) struct VulkanBuffer {
    allocator: Arc<vk_mem::Allocator>,
    handle: vk::Buffer,
    allocation: Allocation,
    size: vk::DeviceSize,
}

impl std::fmt::Debug for VulkanBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanBuffer")
            .field("handle", &self.handle)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl Drop for VulkanBuffer {
    fn drop(&mut self) {
        // SAFETY: `self.handle` and `self.allocation` were created together
        // by this allocator and are destroyed exactly once here.
        unsafe {
            self.allocator.destroy_buffer(self.handle, &mut self.allocation);
        }
    }
}

impl VulkanBuffer {
    pub(super) fn new(
        device: &VulkanLogicalDevice,
        allocator: Arc<vk_mem::Allocator>,
        name: &'static std::ffi::CStr,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        memory_usage: MemoryUsage,
        flags: AllocationCreateFlags,
    ) -> core::result::Result<Self, VulkanBufferError> {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let allocation_info =
            AllocationCreateInfo { flags, usage: memory_usage, ..Default::default() };

        // SAFETY: `allocator` and `buffer_info` are valid for the duration of
        // the call. VMA creates and binds the allocation before
        // returning.
        let (handle, allocation) = vk_try!("create buffer", unsafe {
            allocator.create_buffer(&buffer_info, &allocation_info)
        });

        let buffer = Self { allocator, handle, allocation, size };

        #[cfg(debug_assertions)]
        vk_try!("name buffer", device.set_name(name, buffer.handle));

        Ok(buffer)
    }

    pub(super) fn from_staged_bytes(
        device: &VulkanLogicalDevice,
        allocator: Arc<vk_mem::Allocator>,
        command: &crate::renderer::backend::command::VulkanCommand,
        queue: vk::Queue,
        name: &'static std::ffi::CStr,
        bytes: &[u8],
        usage: vk::BufferUsageFlags,
    ) -> core::result::Result<Self, VulkanBufferError> {
        let size = vk::DeviceSize::try_from(bytes.len())
            .map_err(|_| VulkanBufferError::BufferTooLarge { bytes: bytes.len() })?;

        let mut staging = Self::new(
            device,
            allocator.clone(),
            c"staging vulkan buffer",
            size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            MemoryUsage::AutoPreferHost,
            AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE,
        )?;

        staging.write(bytes)?;

        let buffer = Self::new(
            device,
            allocator,
            name,
            size,
            usage | vk::BufferUsageFlags::TRANSFER_DST,
            MemoryUsage::AutoPreferDevice,
            AllocationCreateFlags::empty(),
        )?;

        command.copy_buffer(queue, staging.handle, buffer.handle, size)?;

        Ok(buffer)
    }

    pub(super) fn device_address(&self, device: &VulkanLogicalDevice) -> vk::DeviceAddress {
        let info = vk::BufferDeviceAddressInfo::default().buffer(self.handle);

        // SAFETY: `self.handle` is live and was created with
        // SHADER_DEVICE_ADDRESS usage by callers that need an
        // address.
        unsafe { device.handle().get_buffer_device_address(&info) }
    }

    fn write(&mut self, bytes: &[u8]) -> core::result::Result<(), VulkanBufferError> {
        let size = vk::DeviceSize::try_from(bytes.len())
            .map_err(|_| VulkanBufferError::BufferTooLarge { bytes: bytes.len() })?;

        if size > self.size {
            return Err(VulkanBufferError::WriteTooLarge { bytes: size, capacity: self.size });
        }

        // SAFETY: The allocation was created with host sequential-write
        // access.
        let flush_result = unsafe {
            let mapped =
                vk_try!("map buffer memory", self.allocator.map_memory(&mut self.allocation),);

            // SAFETY: `bytes` and the mapped allocation do not overlap, and
            // `size` has been checked against buffer capacity.
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), mapped, bytes.len());

            let flush_result = self.allocator.flush_allocation(&self.allocation, 0, size);
            self.allocator.unmap_memory(&mut self.allocation);
            flush_result
        };

        vk_try!("flush buffer memory", flush_result);

        Ok(())
    }
}

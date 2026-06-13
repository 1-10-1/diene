mod instance;

use thiserror::Error;

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub enum VulkanBackendError {
    /// Failed to create the vulkan instance
    #[error("instance creation failed: [{0:?}] {0}")]
    InstanceCreationFailed(ash::vk::Result),

    /// Failed to load the vulkan entry
    #[error("entry load failed")]
    EntryLoadFailure,
}

pub(super) struct VulkanBackend {
    // Held for RAII and drop order; instance-backed objects must not outlive the instance.
    #[allow(dead_code)]
    instance: instance::VulkanInstance,

    // Held for RAII and loader lifetime; Vulkan objects are created from this entry.
    #[allow(dead_code)]
    entry: ash::Entry,
}

impl std::fmt::Debug for VulkanBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanBackend").finish()
    }
}

impl VulkanBackend {
    pub(super) fn new() -> Result<Self, VulkanBackendError> {
        // SAFETY: Must outlive every other object spawned from it.
        let mut entry = unsafe { ash::Entry::load() }.map_err(|_| VulkanBackendError::EntryLoadFailure)?;

        let instance = Self::create_instance(&mut entry)?;

        Ok(Self { instance, entry })
    }
}

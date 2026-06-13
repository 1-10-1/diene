use ash::vk::{self, ApplicationInfo, InstanceCreateInfo};
use common::logging::macros::*;

use super::{VulkanBackend, VulkanBackendError};

pub(super) struct VulkanInstance {
    raw: ash::Instance,
}

impl Drop for VulkanInstance {
    fn drop(&mut self) {
        unsafe {
            self.raw.destroy_instance(None);
        }
    }
}

impl VulkanBackend {
    /// Creates the Vulkan instance.
    pub(super) fn create_instance(entry: &mut ash::Entry) -> Result<VulkanInstance, VulkanBackendError> {
        let application_info = &ApplicationInfo::default()
            .application_name(c"Diene Vulkan Backend")
            .api_version(vk::API_VERSION_1_3);

        let create_info = InstanceCreateInfo::default().application_info(application_info);

        let instance = VulkanInstance {
            raw: unsafe { entry.create_instance(&create_info, None).map_err(VulkanBackendError::InstanceCreationFailed)? },
        };

        debug!("instance initialized");

        Ok(instance)
    }
}

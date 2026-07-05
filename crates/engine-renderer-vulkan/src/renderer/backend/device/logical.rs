use std::ffi::CStr;

#[cfg(debug_assertions)]
use ash::ext::debug_utils;
use ash::vk;
use common::logging::macros::*;

pub(in crate::renderer::backend) struct VulkanLogicalDevice {
    #[cfg(debug_assertions)]
    debug_utils_loader: debug_utils::Device,

    handle: ash::Device,
}

impl VulkanLogicalDevice {
    pub(in crate::renderer::backend) fn new(instance: &ash::Instance, handle: ash::Device) -> Self {
        #[cfg(debug_assertions)]
        let debug_utils_loader = debug_utils::Device::new(instance, &handle);

        Self {
            #[cfg(debug_assertions)]
            debug_utils_loader,
            handle,
        }
    }

    pub(in crate::renderer::backend) fn get_handle(&self) -> &ash::Device {
        &self.handle
    }

    #[cfg(debug_assertions)]
    pub(in crate::renderer::backend) fn set_name<T: vk::Handle>(
        &self,
        name: &CStr,
        handle: T,
    ) -> core::result::Result<(), vk::Result> {
        let name_info =
            vk::DebugUtilsObjectNameInfoEXT::default().object_name(name).object_handle(handle);

        // SAFETY: `self.handle` is a live device, `debug_utils_loader` was created for it, and
        // `name_info` points to `name`, which lives through this call.
        unsafe { self.debug_utils_loader.set_debug_utils_object_name(&name_info)? };

        Ok(())
    }
}

impl Drop for VulkanLogicalDevice {
    fn drop(&mut self) {
        // SAFETY: `self.handle` is a valid logical device created by `create_device`,
        // owned exclusively by this RAII wrapper, and destroyed exactly once here.
        // Future device-owned resources must be destroyed before this wrapper drops.
        unsafe {
            let _ = self.handle.device_wait_idle();
            self.handle.destroy_device(None);
        }
        trace!("device destroyed");
    }
}

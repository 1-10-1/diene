#[cfg(debug_assertions)]
use std::ffi::CString;
use std::sync::Arc;

use ash::vk::{self, Handle};

use crate::renderer::backend::{call_error::VulkanCallError, device::VulkanLogicalDevice};

pub(super) struct VulkanFrameSync {
    device: Arc<VulkanLogicalDevice>,
    image_available: vk::Semaphore,
    render_finished: Vec<vk::Semaphore>,
    in_flight: vk::Fence,
}

impl Drop for VulkanFrameSync {
    fn drop(&mut self) {
        // SAFETY: All synchronization objects were created from `self.device`
        // and are destroyed once after the backend has waited for the
        // device to become idle.
        unsafe {
            if !self.image_available.is_null() {
                self.device.handle().destroy_semaphore(self.image_available, None);
            }

            for semaphore in self.render_finished.drain(..) {
                self.device.handle().destroy_semaphore(semaphore, None);
            }

            if !self.in_flight.is_null() {
                self.device.handle().destroy_fence(self.in_flight, None);
            }
        }
    }
}

impl VulkanFrameSync {
    pub(super) fn new(
        device: Arc<VulkanLogicalDevice>,
        swapchain_image_count: usize,
    ) -> core::result::Result<Self, VulkanCallError> {
        let mut sync = Self {
            device,
            image_available: vk::Semaphore::null(),
            render_finished: Vec::with_capacity(swapchain_image_count),
            in_flight: vk::Fence::null(),
        };

        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

        // SAFETY: `sync.device` is a live logical device and no custom
        // allocator is used.
        sync.image_available = vk_try!("create image-available semaphore", unsafe {
            sync.device.handle().create_semaphore(&semaphore_info, None)
        });

        #[cfg(debug_assertions)]
        vk_try!(
            "name image-available semaphore",
            sync.device.set_name(c"image available semaphore", sync.image_available),
        );

        for index in 0..swapchain_image_count {
            // SAFETY: `sync.device` is a live logical device and no custom
            // allocator is used.
            let semaphore = vk_try!("create render-finished semaphore", unsafe {
                sync.device.handle().create_semaphore(&semaphore_info, None)
            });

            #[cfg(debug_assertions)]
            if let Ok(name) = CString::new(format!("render finished semaphore {index}")) {
                vk_try!(
                    "name render-finished semaphore",
                    sync.device.set_name(name.as_c_str(), semaphore),
                );
            }

            sync.render_finished.push(semaphore);
        }

        // SAFETY: `sync.device` is a live logical device and no custom
        // allocator is used.
        sync.in_flight = vk_try!("create in-flight frame fence", unsafe {
            sync.device.handle().create_fence(&fence_info, None)
        });

        #[cfg(debug_assertions)]
        vk_try!(
            "name in-flight frame fence",
            sync.device.set_name(c"in-flight frame fence", sync.in_flight),
        );

        Ok(sync)
    }

    pub(super) fn image_available(&self) -> vk::Semaphore {
        self.image_available
    }

    pub(super) fn render_finished(&self, image_index: u32) -> Option<vk::Semaphore> {
        usize::try_from(image_index)
            .ok()
            .and_then(|index| self.render_finished.get(index))
            .copied()
    }

    pub(super) fn in_flight(&self) -> vk::Fence {
        self.in_flight
    }
}

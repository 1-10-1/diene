#[cfg(debug_assertions)]
use std::ffi::CString;
use std::sync::Arc;

use ash::vk::{self, Extent2D, Handle};

use crate::renderer::backend::{
    VulkanBackendError,
    allocator::VulkanAllocator,
    call_error::VulkanCallError,
    depth,
    device::{self, VulkanLogicalDevice},
    image, instance, surface, swapchain,
};

/// Per-frame-in-flight synchronization objects, one set per slot in
/// [`super::FRAMES_IN_FLIGHT`]. Unlike [`FrameResources`], these are
/// not tied to the swapchain extent and are not recreated when the
/// window resizes.
pub(super) struct VulkanFrameSync {
    device: Arc<VulkanLogicalDevice>,
    image_available: [vk::Semaphore; super::FRAMES_IN_FLIGHT],
    in_flight: [vk::Fence; super::FRAMES_IN_FLIGHT],
}

impl Drop for VulkanFrameSync {
    fn drop(&mut self) {
        // SAFETY: All objects were created from `self.device` and are
        // destroyed once after the backend has waited for the device to
        // become idle.
        unsafe {
            for semaphore in self.image_available {
                if !semaphore.is_null() {
                    self.device.handle().destroy_semaphore(semaphore, None);
                }
            }

            for fence in self.in_flight {
                if !fence.is_null() {
                    self.device.handle().destroy_fence(fence, None);
                }
            }
        }
    }
}

impl VulkanFrameSync {
    pub(super) fn new(
        device: Arc<VulkanLogicalDevice>,
    ) -> core::result::Result<Self, VulkanCallError> {
        let mut sync = Self {
            device,
            image_available: [vk::Semaphore::null(); super::FRAMES_IN_FLIGHT],
            in_flight: [vk::Fence::null(); super::FRAMES_IN_FLIGHT],
        };

        let semaphore_info = vk::SemaphoreCreateInfo::default();

        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

        for index in 0..super::FRAMES_IN_FLIGHT {
            // SAFETY: `sync.device` is a live logical device and no custom
            // allocator is used.
            let image_available = vk_try!("create image-available semaphore", unsafe {
                sync.device.handle().create_semaphore(&semaphore_info, None)
            });

            #[cfg(debug_assertions)]
            if let Ok(name) = CString::new(format!("image available semaphore {index}")) {
                vk_try!(
                    "name image-available semaphore",
                    sync.device.set_name(name.as_c_str(), image_available),
                );
            }

            sync.image_available[index] = image_available;

            // SAFETY: `sync.device` is a live logical device and no custom
            // allocator is used.
            let in_flight = vk_try!("create in-flight frame fence", unsafe {
                sync.device.handle().create_fence(&fence_info, None)
            });

            #[cfg(debug_assertions)]
            if let Ok(name) = CString::new(format!("in-flight frame fence {index}")) {
                vk_try!(
                    "name in-flight frame fence",
                    sync.device.set_name(name.as_c_str(), in_flight),
                );
            }

            sync.in_flight[index] = in_flight;
        }

        Ok(sync)
    }

    pub(super) fn image_available(&self, frame_index: usize) -> vk::Semaphore {
        self.image_available[frame_index % super::FRAMES_IN_FLIGHT]
    }

    pub(super) fn in_flight(&self, frame_index: usize) -> vk::Fence {
        self.in_flight[frame_index % super::FRAMES_IN_FLIGHT]
    }
}

/// The render targets and per-swapchain-image synchronization needed
/// to render and present a frame. Recreated as a unit whenever the
/// window resizes.
///
/// `depth` and `color` are written by every frame's rendering
/// commands (not just read back like the swapchain image), so — like
/// [`VulkanFrameSync`] — they need one copy per slot in
/// [`super::FRAMES_IN_FLIGHT`]: with more than one frame genuinely in
/// flight on the GPU, a single shared depth/color image would let two
/// frames race on the same underlying memory. The per-frame layout
/// transitions in `record_render_commands` use `srcAccessMask: NONE`,
/// which is only sound because each frame-in-flight owns its own
/// image; if these were ever shared again, that barrier would need a
/// real `COLOR_ATTACHMENT_WRITE`/`DEPTH_STENCIL_ATTACHMENT_WRITE`
/// source access to avoid reintroducing the write-after-write hazard
/// this duplication exists to prevent.
pub(super) struct FrameResources {
    device: Arc<VulkanLogicalDevice>,
    swapchain: swapchain::VulkanSwapchain,
    depth: [depth::DepthAttachment; super::FRAMES_IN_FLIGHT],
    color: [Option<image::VulkanImage>; super::FRAMES_IN_FLIGHT],
    render_finished: Vec<vk::Semaphore>,
}

impl Drop for FrameResources {
    fn drop(&mut self) {
        for semaphore in self.render_finished.drain(..) {
            // SAFETY: `semaphore` was created through `self.device` and is
            // destroyed exactly once here.
            unsafe {
                self.device.handle().destroy_semaphore(semaphore, None);
            }
        }
    }
}

impl FrameResources {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        instance: &instance::VulkanInstance,
        device: &device::VulkanDevice,
        allocator: &VulkanAllocator,
        surface: &surface::VulkanSurface,
        surface_config: &surface::SurfaceConfig,
        sample_count: vk::SampleCountFlags,
        old_swapchain: vk::SwapchainKHR,
    ) -> core::result::Result<Self, VulkanBackendError> {
        let swapchain = swapchain::VulkanSwapchain::new_replacing(
            instance,
            device.logical().clone(),
            surface,
            surface_config,
            old_swapchain,
        )
        .map_err(|_| VulkanBackendError::SwapchainOperation)?;

        let depth = (0..super::FRAMES_IN_FLIGHT)
            .map(|_| {
                depth::DepthAttachment::new(allocator, device, surface_config.extent, sample_count)
            })
            .collect::<core::result::Result<Vec<_>, _>>()
            .map_err(|_| VulkanBackendError::DepthOperation)?
            .try_into()
            .map_err(|_| VulkanBackendError::DepthOperation)?;

        let color = (0..super::FRAMES_IN_FLIGHT)
            .map(|_| {
                new_msaa_color_attachment(
                    allocator,
                    device,
                    surface_config.extent,
                    surface_config.surface_format.format,
                    sample_count,
                )
            })
            .collect::<core::result::Result<Vec<_>, _>>()
            .map_err(|_| VulkanBackendError::MsaaColorOperation)?
            .try_into()
            .map_err(|_| VulkanBackendError::MsaaColorOperation)?;

        let render_finished =
            new_render_finished_semaphores(device.logical(), swapchain.image_count())?;

        Ok(Self { device: device.logical().clone(), swapchain, depth, color, render_finished })
    }

    pub(super) fn swapchain(&self) -> &swapchain::VulkanSwapchain {
        &self.swapchain
    }

    pub(super) fn depth_mut(&mut self, frame_index: usize) -> &mut depth::DepthAttachment {
        &mut self.depth[frame_index % super::FRAMES_IN_FLIGHT]
    }

    pub(super) fn depth(&self, frame_index: usize) -> &depth::DepthAttachment {
        &self.depth[frame_index % super::FRAMES_IN_FLIGHT]
    }

    pub(super) fn color(&self, frame_index: usize) -> Option<&image::VulkanImage> {
        self.color[frame_index % super::FRAMES_IN_FLIGHT].as_ref()
    }

    pub(super) fn render_finished(&self, image_index: u32) -> Option<vk::Semaphore> {
        usize::try_from(image_index)
            .ok()
            .and_then(|index| self.render_finished.get(index))
            .copied()
    }
}

fn new_render_finished_semaphores(
    device: &VulkanLogicalDevice,
    count: usize,
) -> core::result::Result<Vec<vk::Semaphore>, VulkanBackendError> {
    let semaphore_info = vk::SemaphoreCreateInfo::default();

    let mut semaphores = Vec::with_capacity(count);

    for index in 0..count {
        // SAFETY: `device` is a live logical device and no custom allocator
        // is used.
        let semaphore = vk_try!("create render-finished semaphore", unsafe {
            device.handle().create_semaphore(&semaphore_info, None)
        });

        #[cfg(debug_assertions)]
        if let Ok(name) = CString::new(format!("render finished semaphore {index}")) {
            vk_try!("name render-finished semaphore", device.set_name(name.as_c_str(), semaphore),);
        }

        semaphores.push(semaphore);
    }

    Ok(semaphores)
}

fn new_msaa_color_attachment(
    allocator: &VulkanAllocator,
    device: &device::VulkanDevice,
    extent: Extent2D,
    format: vk::Format,
    sample_count: vk::SampleCountFlags,
) -> core::result::Result<Option<image::VulkanImage>, image::VulkanImageError> {
    if sample_count == vk::SampleCountFlags::TYPE_1 {
        return Ok(None);
    }

    image::VulkanImage::new(
        allocator.handle(),
        device,
        c"MSAA color image",
        extent,
        format,
        sample_count,
        vk::ImageUsageFlags::COLOR_ATTACHMENT,
        vk::ImageAspectFlags::COLOR,
        1,
    )
    .map(Some)
}

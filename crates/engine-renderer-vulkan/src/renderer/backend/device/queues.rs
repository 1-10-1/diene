use ash::vk::{self, PhysicalDevice};

use super::VulkanDeviceError;
use crate::renderer::backend::{instance, surface};

#[derive(Clone)]
pub(in crate::renderer::backend) struct QueueFamilyIndices {
    pub(in crate::renderer::backend) graphics: u32,
    // NOTE: This field is currently dead. The renderer requires graphics == present for now.
    pub(in crate::renderer::backend) present: u32,
    pub(in crate::renderer::backend) compute: u32,
    pub(in crate::renderer::backend) transfer: u32,
}

pub(super) fn find_queue_family_indices(
    inst: &instance::VulkanInstance,
    device: PhysicalDevice,
    surf: &surface::VulkanSurface,
) -> core::result::Result<Option<QueueFamilyIndices>, VulkanDeviceError> {
    let mut graphics: Option<u32> = None;
    let mut graphics_present: Option<u32> = None;
    let mut present: Option<u32> = None;
    let mut compute: Option<u32> = None;
    let mut compute_is_non_graphics = false;
    let mut transfer_only: Option<u32> = None;
    let mut transfer_non_graphics: Option<u32> = None;
    let mut transfer_any: Option<u32> = None;

    // SAFETY: `device` came from `inst`, so querying its queue families against
    // the same instance is valid.
    let queue_families =
        unsafe { inst.handle().get_physical_device_queue_family_properties(device) };

    for (index, queue_family) in queue_families.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
        let index = index as u32;

        let flags = queue_family.queue_flags;
        let has_graphics = flags.contains(vk::QueueFlags::GRAPHICS);
        let has_compute = flags.contains(vk::QueueFlags::COMPUTE);
        let has_transfer = flags.contains(vk::QueueFlags::TRANSFER);

        if has_graphics {
            graphics.get_or_insert(index);
        }

        if has_compute {
            let non_graphics = !has_graphics;

            if compute.is_none() || (non_graphics && !compute_is_non_graphics) {
                compute = Some(index);
                compute_is_non_graphics = non_graphics;
            }
        }

        if has_transfer {
            transfer_any.get_or_insert(index);

            if !has_graphics {
                transfer_non_graphics.get_or_insert(index);
            }

            if !has_graphics && !has_compute {
                transfer_only.get_or_insert(index);
            }
        }

        // SAFETY: `device` came from `inst`, `surf` was created for the same
        // instance, and `index` comes from this physical device's queue families.
        let supports_present = vk_try!("query queue-family present support", unsafe {
            surf.loader().get_physical_device_surface_support(device, index, surf.handle())
        });

        if supports_present {
            present.get_or_insert(index);

            if has_graphics {
                graphics_present.get_or_insert(index);
            }
        }
    }

    let graphics = graphics_present.or(graphics);
    let present = graphics_present.or(present);

    let transfer =
        transfer_only.or(transfer_non_graphics).or(transfer_any).or(compute).or(graphics);

    // NOTE: We're currently forcing graphics == present here.
    Ok(match (graphics, present, compute, transfer) {
        (Some(graphics), Some(present), Some(compute), Some(transfer)) if graphics == present => {
            Some(QueueFamilyIndices { graphics, present, compute, transfer })
        }
        _ => None,
    })
}

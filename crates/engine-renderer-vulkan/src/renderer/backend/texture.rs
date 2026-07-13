use std::sync::Arc;

use ash::vk::{self, Handle};
use engine_renderer_api::{TextureData, TextureExtent};
use thiserror::Error;

use crate::renderer::backend::{
    allocator::VulkanAllocator,
    call_error::VulkanCallError,
    command::VulkanCommand,
    device::{VulkanDevice, VulkanLogicalDevice},
    image::{VulkanImage, VulkanImageError},
};

pub(super) const BINDLESS_TEXTURE_CAPACITY: u32 = 1024;

#[derive(Debug, Error)]
pub(super) enum VulkanTextureError {
    #[error(transparent)]
    UnexpectedResult(#[from] VulkanCallError),

    #[error(transparent)]
    Image(#[from] VulkanImageError),

    #[error("bindless texture heap capacity {capacity} exceeded")]
    CapacityExceeded { capacity: u32 },
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TextureHandle(u32);

impl TextureHandle {
    pub(super) const fn index(self) -> u32 {
        self.0
    }
}

pub(super) struct VulkanTexture {
    image: VulkanImage,
}

impl std::fmt::Debug for VulkanTexture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanTexture").field("image", &self.image).finish()
    }
}

impl VulkanTexture {
    fn from_data(
        allocator: &VulkanAllocator,
        command: &VulkanCommand,
        device: &VulkanDevice,
        data: &TextureData,
    ) -> core::result::Result<Self, VulkanTextureError> {
        let image = VulkanImage::from_texture_data(
            allocator.handle(),
            command,
            device,
            c"bindless texture image",
            data,
        )?;

        Ok(Self { image })
    }

    fn image_view(&self) -> vk::ImageView {
        self.image.view()
    }

    #[allow(dead_code)]
    fn extent(&self) -> TextureExtent {
        self.image.extent()
    }
}

struct VulkanSampler {
    device: Arc<VulkanLogicalDevice>,
    handle: vk::Sampler,
}

impl std::fmt::Debug for VulkanSampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanSampler")
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}

impl Drop for VulkanSampler {
    fn drop(&mut self) {
        // SAFETY: `self.handle` was created through `self.device` and is
        // destroyed exactly once.
        unsafe {
            self.device.handle().destroy_sampler(self.handle, None);
        }
    }
}

impl VulkanSampler {
    fn new(device: &VulkanDevice) -> core::result::Result<Self, VulkanTextureError> {
        let create_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::NEAREST)
            .min_filter(vk::Filter::NEAREST)
            .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT)
            .mip_lod_bias(0.0)
            .anisotropy_enable(true)
            .max_anisotropy(device.properties().limits.max_sampler_anisotropy)
            .compare_enable(false)
            .compare_op(vk::CompareOp::ALWAYS)
            .min_lod(0.0)
            .max_lod(0.0)
            .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false);

        // SAFETY: `create_info` references no borrowed data.
        let handle = vk_try!("create texture sampler", unsafe {
            device.logical().handle().create_sampler(&create_info, None)
        });

        #[cfg(debug_assertions)]
        vk_try!("name texture sampler", device.logical().set_name(c"texture sampler", handle));

        Ok(Self { device: device.logical().clone(), handle })
    }

    fn handle(&self) -> vk::Sampler {
        self.handle
    }
}

pub(super) struct BindlessTextureHeap {
    device: Arc<VulkanLogicalDevice>,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_set: vk::DescriptorSet,
    sampler: VulkanSampler,
    textures: Vec<VulkanTexture>,
    default_handle: TextureHandle,
}

impl std::fmt::Debug for BindlessTextureHeap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BindlessTextureHeap")
            .field("descriptor_pool", &self.descriptor_pool)
            .field("descriptor_set_layout", &self.descriptor_set_layout)
            .field("descriptor_set", &self.descriptor_set)
            .field("sampler", &self.sampler)
            .field("texture_count", &self.textures.len())
            .field("default_handle", &self.default_handle)
            .finish_non_exhaustive()
    }
}

impl Drop for BindlessTextureHeap {
    fn drop(&mut self) {
        // SAFETY: These descriptor objects were created through `self.device`
        // and are destroyed exactly once. The backend waits for device idle
        // before resource fields are dropped.
        unsafe {
            if !self.descriptor_pool.is_null() {
                self.device.handle().destroy_descriptor_pool(self.descriptor_pool, None);
            }

            if !self.descriptor_set_layout.is_null() {
                self.device
                    .handle()
                    .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            }
        }
    }
}

impl BindlessTextureHeap {
    pub(super) fn new(
        allocator: &VulkanAllocator,
        command: &VulkanCommand,
        device: &VulkanDevice,
    ) -> core::result::Result<Self, VulkanTextureError> {
        let descriptor_set_layout = create_descriptor_set_layout(device.logical())?;
        let descriptor_pool = create_descriptor_pool(device.logical())?;
        let descriptor_set =
            allocate_descriptor_set(device.logical(), descriptor_pool, descriptor_set_layout)?;
        let sampler = VulkanSampler::new(device)?;

        write_sampler_descriptor(device.logical(), descriptor_set, sampler.handle());

        let mut heap = Self {
            device: device.logical().clone(),
            descriptor_pool,
            descriptor_set_layout,
            descriptor_set,
            sampler,
            textures: Vec::new(),
            default_handle: TextureHandle(0),
        };

        heap.default_handle = heap.insert(allocator, command, device, &TextureData::default())?;

        Ok(heap)
    }

    pub(super) fn insert(
        &mut self,
        allocator: &VulkanAllocator,
        command: &VulkanCommand,
        device: &VulkanDevice,
        data: &TextureData,
    ) -> core::result::Result<TextureHandle, VulkanTextureError> {
        let index = u32::try_from(self.textures.len()).unwrap_or(u32::MAX);

        if index >= BINDLESS_TEXTURE_CAPACITY {
            return Err(VulkanTextureError::CapacityExceeded {
                capacity: BINDLESS_TEXTURE_CAPACITY,
            });
        }

        let texture = VulkanTexture::from_data(allocator, command, device, data)?;
        write_texture_descriptor(&self.device, self.descriptor_set, TextureHandle(index), &texture);
        self.textures.push(texture);

        Ok(TextureHandle(index))
    }

    pub(super) fn descriptor_set_layout(&self) -> vk::DescriptorSetLayout {
        self.descriptor_set_layout
    }

    pub(super) fn descriptor_set(&self) -> vk::DescriptorSet {
        self.descriptor_set
    }

    pub(super) fn default_handle(&self) -> TextureHandle {
        self.default_handle
    }
}

fn create_descriptor_set_layout(
    device: &VulkanLogicalDevice,
) -> core::result::Result<vk::DescriptorSetLayout, VulkanTextureError> {
    let bindings = [
        vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
            .descriptor_count(BINDLESS_TEXTURE_CAPACITY)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT),
        vk::DescriptorSetLayoutBinding::default()
            .binding(1)
            .descriptor_type(vk::DescriptorType::SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT),
    ];
    let binding_flags =
        [vk::DescriptorBindingFlags::PARTIALLY_BOUND, vk::DescriptorBindingFlags::empty()];
    let mut binding_flags_info =
        vk::DescriptorSetLayoutBindingFlagsCreateInfo::default().binding_flags(&binding_flags);
    let create_info = vk::DescriptorSetLayoutCreateInfo::default()
        .bindings(&bindings)
        .push_next(&mut binding_flags_info);

    // SAFETY: `create_info` only references local slices that live
    // through the call.
    let layout = vk_try!("create bindless texture descriptor set layout", unsafe {
        device.handle().create_descriptor_set_layout(&create_info, None)
    });

    #[cfg(debug_assertions)]
    vk_try!(
        "name bindless texture descriptor set layout",
        device.set_name(c"bindless texture descriptor set layout", layout),
    );

    Ok(layout)
}

fn create_descriptor_pool(
    device: &VulkanLogicalDevice,
) -> core::result::Result<vk::DescriptorPool, VulkanTextureError> {
    let pool_sizes = [
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::SAMPLED_IMAGE,
            descriptor_count: BINDLESS_TEXTURE_CAPACITY,
        },
        vk::DescriptorPoolSize { ty: vk::DescriptorType::SAMPLER, descriptor_count: 1 },
    ];
    let create_info = vk::DescriptorPoolCreateInfo::default().max_sets(1).pool_sizes(&pool_sizes);

    // SAFETY: `create_info` only references local slices that live
    // through the call.
    let pool = vk_try!("create bindless texture descriptor pool", unsafe {
        device.handle().create_descriptor_pool(&create_info, None)
    });

    #[cfg(debug_assertions)]
    vk_try!(
        "name bindless texture descriptor pool",
        device.set_name(c"bindless texture descriptor pool", pool)
    );

    Ok(pool)
}

fn allocate_descriptor_set(
    device: &VulkanLogicalDevice,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
) -> core::result::Result<vk::DescriptorSet, VulkanTextureError> {
    let layouts = [descriptor_set_layout];
    let allocate_info = vk::DescriptorSetAllocateInfo::default()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&layouts);

    // SAFETY: Pool and layout are live and compatible.
    let mut sets = vk_try!("allocate bindless texture descriptor set", unsafe {
        device.handle().allocate_descriptor_sets(&allocate_info)
    });
    let set = sets.pop().ok_or_else(|| {
        VulkanCallError::new("allocate bindless texture descriptor set", vk::Result::ERROR_UNKNOWN)
    })?;

    #[cfg(debug_assertions)]
    vk_try!(
        "name bindless texture descriptor set",
        device.set_name(c"bindless texture descriptor set", set)
    );

    Ok(set)
}

fn write_sampler_descriptor(
    device: &VulkanLogicalDevice,
    descriptor_set: vk::DescriptorSet,
    sampler: vk::Sampler,
) {
    let sampler_info = [vk::DescriptorImageInfo::default().sampler(sampler)];
    let writes = [vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set)
        .dst_binding(1)
        .descriptor_type(vk::DescriptorType::SAMPLER)
        .image_info(&sampler_info)];

    // SAFETY: `descriptor_set` is live and binding 1 is a single sampler.
    unsafe {
        device.handle().update_descriptor_sets(&writes, &[]);
    }
}

fn write_texture_descriptor(
    device: &VulkanLogicalDevice,
    descriptor_set: vk::DescriptorSet,
    handle: TextureHandle,
    texture: &VulkanTexture,
) {
    let image_info = [vk::DescriptorImageInfo::default()
        .image_view(texture.image_view())
        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)];
    let writes = [vk::WriteDescriptorSet::default()
        .dst_set(descriptor_set)
        .dst_binding(0)
        .dst_array_element(handle.index())
        .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
        .image_info(&image_info)];

    // SAFETY: `descriptor_set` is live and binding 0 is a sampled-image
    // array.
    unsafe {
        device.handle().update_descriptor_sets(&writes, &[]);
    }
}

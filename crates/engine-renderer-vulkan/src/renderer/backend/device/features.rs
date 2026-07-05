use ash::vk;

#[derive(Default)]
pub(super) struct FeaturesInfo {
    pub(super) availability: Vec<(&'static str, bool)>,
    pub(super) enabled_f10: vk::PhysicalDeviceFeatures,
    pub(super) enabled_f11: vk::PhysicalDeviceVulkan11Features<'static>,
    pub(super) enabled_f12: vk::PhysicalDeviceVulkan12Features<'static>,
    pub(super) enabled_f13: vk::PhysicalDeviceVulkan13Features<'static>,
}

macro_rules! require_features {
    (
        $availability:ident;
        $(
            $supported:ident => $enabled:ident {
                $(
                    $field:ident => $label:literal
                ),* $(,)?
            }
        )*
    ) => {
        $(
            $(
                let available = $supported.$field != vk::FALSE;
                $enabled.$field = if available { vk::TRUE } else { vk::FALSE };
                $availability.push(($label, available));
            )*
        )*
    };
}

pub(super) fn get_features_info(
    supported_10: &vk::PhysicalDeviceFeatures,
    supported_11: &vk::PhysicalDeviceVulkan11Features<'_>,
    supported_12: &vk::PhysicalDeviceVulkan12Features<'_>,
    supported_13: &vk::PhysicalDeviceVulkan13Features<'_>,
) -> FeaturesInfo {
    let mut availability = Vec::new();

    let mut enabled_10 = vk::PhysicalDeviceFeatures::default();
    let mut enabled_11 = vk::PhysicalDeviceVulkan11Features::default();
    let mut enabled_12 = vk::PhysicalDeviceVulkan12Features::default();
    let mut enabled_13 = vk::PhysicalDeviceVulkan13Features::default();

    require_features!(availability;
        supported_10 => enabled_10 {
            geometry_shader => "Geometry shader availability",
            sampler_anisotropy => "Anisotropy availability",
            sample_rate_shading => "Sample rate shading availability",
            multi_draw_indirect => "Multi-draw indirect availability",
            fill_mode_non_solid => "Non-solid fill mode availability",
            vertex_pipeline_stores_and_atomics => "Vertex pipeline stores and atomics availability",
            fragment_stores_and_atomics => "Fragment stores and atomics availability",
            shader_storage_image_multisample => "Storage image multisample availability",
            shader_int64 => "64-bit integer shader support availability",
        }

        supported_11 => enabled_11 {
            shader_draw_parameters => "Shader draw parameters availability",
        }

        supported_12 => enabled_12 {
            storage_buffer8_bit_access => "8-bit storage buffer access availability",
            descriptor_indexing => "Descriptor indexing availability",
            shader_sampled_image_array_non_uniform_indexing => "Non-uniform sampled image array indexing availability",
            descriptor_binding_partially_bound => "Partially bound descriptor binding availability",
            runtime_descriptor_array => "Runtime descriptor array availability",
            timeline_semaphore => "Timeline semaphore availability",
            buffer_device_address => "Buffer device address availability",
            vulkan_memory_model => "Vulkan memory model availability",
            vulkan_memory_model_device_scope => "Vulkan memory model device scope availability",
        }

        supported_13 => enabled_13 {
            synchronization2 => "Synchronization2 availability",
            dynamic_rendering => "Dynamic rendering availability",
        }
    );

    FeaturesInfo {
        availability,
        enabled_f10: enabled_10,
        enabled_f11: enabled_11,
        enabled_f12: enabled_12,
        enabled_f13: enabled_13,
    }
}

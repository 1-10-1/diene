#[macro_use]
mod call_error;

mod allocator;
mod buffer;
mod command;
mod compute;
mod depth;
mod device;
mod draw;
mod frame;
mod image;
mod instance;
mod material;
mod mesh;
mod pipeline;
mod scene;
mod shader;
mod surface;
mod swapchain;
mod texture;
mod transform;

use std::{rc::Rc, time::Instant};

use ash::vk::{self, Extent2D};
use common::logging::macros::*;
use engine_renderer_api::{RenderCamera, RenderExtent, RenderScene, RenderWindow};
use engine_shader::{ShaderCompiler, ShaderCompilerOptions};
use error_stack::{Report, ResultExt};
use thiserror::Error;

/// Number of frames the CPU may record and submit concurrently before
/// it must wait on the GPU. Each frame in flight needs its own
/// command buffer and synchronization objects (see
/// [`frame::VulkanFrameSync`]).
pub(super) const FRAMES_IN_FLIGHT: usize = 3;

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub(super) enum VulkanBackendError {
    /// Failed to load the vulkan entry
    #[error("failed to load the vulkan entry")]
    EntryLoadFailure,

    /// Failed to get the display handle.
    #[error("failed to get display handle")]
    DisplayHandle,

    /// Failed to get the window handle.
    #[error("failed to get window handle")]
    WindowHandle,

    /// Vulkan instance operation failed.
    #[error("instance operation failed")]
    InstanceOperation,

    /// Vulkan surface operation failed.
    #[error("surface operation failed")]
    SurfaceOperation,

    /// Vulkan device operation failed.
    #[error("logical device operation failed")]
    DeviceOperation,

    /// Vulkan swapchain operation failed.
    #[error("swapchain operation failed")]
    SwapchainOperation,

    /// Vulkan allocator operation failed.
    #[error("allocator operation failed")]
    AllocatorOperation,

    /// Vulkan command operation failed.
    #[error("command operation failed")]
    CommandOperation,

    /// Vulkan frame operation failed.
    #[error("frame operation failed")]
    FrameOperation,

    /// Vulkan pipeline operation failed.
    #[error("pipeline operation failed")]
    PipelineOperation,

    /// Vulkan depth attachment operation failed.
    #[error("depth attachment operation failed")]
    DepthOperation,

    /// Vulkan MSAA color attachment operation failed.
    #[error("MSAA color attachment operation failed")]
    MsaaColorOperation,

    /// The MSAA color attachment image was created without an image
    /// view.
    #[error("MSAA color attachment has no image view")]
    MsaaColorViewMissing,

    /// Vulkan draw-list operation failed.
    #[error("draw-list operation failed")]
    DrawOperation,

    /// Material index for a generated draw item was not available.
    #[error("material index {index} was not available")]
    MaterialIndexUnavailable {
        /// Material index requested by the draw item.
        index: usize,
    },

    /// Mesh index for a generated draw item was not available.
    #[error("mesh index {index} was not available")]
    MeshIndexUnavailable {
        /// Mesh index requested by the draw item.
        index: usize,
    },

    /// Transform index for a generated draw item was not available.
    #[error("transform index {index} was not available")]
    TransformIndexUnavailable {
        /// Transform index requested by the draw item.
        index: usize,
    },

    /// Vulkan mesh operation failed.
    #[error("mesh operation failed")]
    MeshOperation,

    /// Vulkan scene-buffer operation failed.
    #[error("scene-buffer operation failed")]
    SceneBufferOperation,

    /// Vulkan material-buffer operation failed.
    #[error("material-buffer operation failed")]
    MaterialBufferOperation,

    /// Vulkan transform-buffer operation failed.
    #[error("transform-buffer operation failed")]
    TransformBufferOperation,

    /// Vulkan texture operation failed.
    #[error("texture operation failed")]
    TextureOperation,

    /// Vulkan surface refresh failed.
    #[error("failed to refresh vulkan surface details")]
    RefreshSurface,

    /// Vulkan shader compiler operation failed.
    #[error("shader compiler operation failed")]
    ShaderCompilation,

    /// The compute pipeline self-check failed to build, dispatch, or
    /// verify its output.
    #[error("compute pipeline self-check failed")]
    ComputeSelfCheck,

    /// Vulkan API call returned an error value.
    #[error(transparent)]
    UnexpectedResult(#[from] call_error::VulkanCallError),

    /// The swapchain returned an image index without a matching image
    /// and view.
    #[error("swapchain returned invalid image index {image_index}")]
    InvalidSwapchainImageIndex {
        /// Image index returned by Vulkan.
        image_index: u32,
    },

    /// Surface format changed after the triangle pipeline was
    /// created.
    #[error("surface format changed from {old:?} to {new:?}")]
    SurfaceFormatChanged {
        /// Format used by the current graphics pipeline.
        old: vk::Format,

        /// Format requested by the refreshed surface.
        new: vk::Format,
    },
}

#[allow(dead_code)]
pub(super) struct VulkanBackend {
    // NOTE: Fields are dropped in declaration order. Keep Vulkan children above their parents.
    frame: frame::FrameResources,
    frame_sync: frame::VulkanFrameSync,
    graphics_pipeline: pipeline::VulkanGraphicsPipeline,
    pipeline_layout: pipeline::VulkanPipelineLayout,
    draw_list: Option<draw::GpuDrawList>,
    transform_table: Option<transform::TransformTable>,
    material_table: Option<material::MaterialTable>,
    texture_heap: texture::BindlessTextureHeap,
    scene_buffers: Vec<scene::SceneBuffer>,
    meshes: Vec<mesh::GpuMesh>,
    command: command::VulkanCommand,
    allocator: allocator::VulkanAllocator,
    surface_config: surface::SurfaceConfig,
    sample_count: vk::SampleCountFlags,
    current_frame: usize,
    rendering_paused: bool,
    vsync: bool,
    device: device::VulkanDevice,
    surface: surface::VulkanSurface,
    instance: instance::VulkanInstance,
    entry: ash::Entry,
}

impl Drop for VulkanBackend {
    fn drop(&mut self) {
        // SAFETY: The logical device is live, and waiting here keeps
        // backend-owned resources from being destroyed while their
        // final submitted frame may still be executing.
        if let Err(result) = unsafe { self.device.logical().handle().device_wait_idle() } {
            warn!("failed to wait for Vulkan device idle during backend drop: {result:?}");
        }
    }
}

impl VulkanBackend {
    pub(super) fn new(
        render_window: &dyn RenderWindow,
        vsync: bool,
        scene: &RenderScene,
    ) -> error_stack::Result<Self, VulkanBackendError> {
        let display_handle = render_window
            .display_handle()
            .map_err(|error| {
                Report::new(VulkanBackendError::DisplayHandle).attach_printable(error.to_string())
            })?
            .as_raw();

        let window_handle = render_window
            .window_handle()
            .map_err(|error| {
                Report::new(VulkanBackendError::WindowHandle).attach_printable(error.to_string())
            })?
            .as_raw();

        // SAFETY: Loading the Vulkan entry only performs dynamic symbol
        // lookup; the owned entry is stored in the backend and
        // outlives all objects created from it.
        let entry = unsafe { ash::Entry::load() }.map_err(|err| {
            Report::new(VulkanBackendError::EntryLoadFailure).attach_printable(err.to_string())
        })?;

        let instance = instance::VulkanInstance::new(&entry, display_handle)
            .change_context(VulkanBackendError::InstanceOperation)?;

        let surface = surface::VulkanSurface::new(&entry, &instance, display_handle, window_handle)
            .change_context(VulkanBackendError::SurfaceOperation)?;

        let device = device::VulkanDevice::new(&instance, &surface)
            .change_context(VulkanBackendError::DeviceOperation)?;

        // let sample_count = device.get_max_usable_sample_count();
        let sample_count = device.get_max_usable_sample_count();

        debug!("selected MSAA sample count {sample_count:?}");

        let RenderExtent { width, height } = render_window.size();

        let surface_config = surface
            .make_config(device.physical(), Extent2D { width, height }, vsync)
            .change_context(VulkanBackendError::RefreshSurface)?;

        let allocator =
            allocator::VulkanAllocator::new(&instance, device.logical(), device.physical())
                .change_context(VulkanBackendError::AllocatorOperation)?;

        let command =
            command::VulkanCommand::new(device.logical().clone(), device.queue_families())
                .change_context(VulkanBackendError::CommandOperation)?;

        let mut meshes = Vec::with_capacity(scene.meshes().len());

        for data in scene.meshes() {
            meshes.push(
                mesh::GpuMesh::new(&allocator, &command, &device, data)
                    .change_context(VulkanBackendError::MeshOperation)?,
            );
        }

        let scene_buffers = (0..FRAMES_IN_FLIGHT)
            .map(|_| scene::SceneBuffer::new(&allocator, &device))
            .collect::<core::result::Result<Vec<_>, _>>()
            .change_context(VulkanBackendError::SceneBufferOperation)?;

        let mut texture_heap = texture::BindlessTextureHeap::new(&allocator, &command, &device)
            .change_context(VulkanBackendError::TextureOperation)?;

        let (material_table, transform_table, draw_list) = if scene.objects().is_empty() {
            (None, None, None)
        } else {
            let material_table = material::MaterialTable::new(
                &allocator,
                &command,
                &device,
                &mut texture_heap,
                scene.materials(),
            )
            .change_context(VulkanBackendError::MaterialBufferOperation)?;

            let transforms =
                scene.objects().iter().map(|object| object.transform()).collect::<Vec<_>>();
            let transform_table =
                transform::TransformTable::new(&allocator, &command, &device, &transforms)
                    .change_context(VulkanBackendError::TransformBufferOperation)?;

            let draw_inputs = scene
                .objects()
                .iter()
                .enumerate()
                .map(|(index, object)| {
                    let mesh = meshes
                        .get(object.mesh_index())
                        .ok_or(VulkanBackendError::MeshIndexUnavailable { index })?;
                    let material = material_table
                        .material_index(object.material_index())
                        .ok_or(VulkanBackendError::MaterialIndexUnavailable { index })?;
                    let transform = transform_table
                        .transform_index(index)
                        .ok_or(VulkanBackendError::TransformIndexUnavailable { index })?;

                    Ok(draw::DrawInput::new(mesh, material, transform))
                })
                .collect::<core::result::Result<Vec<_>, VulkanBackendError>>()?;

            let draw_list = draw::GpuDrawList::new(&allocator, &command, &device, &draw_inputs)
                .change_context(VulkanBackendError::DrawOperation)?;

            (Some(material_table), Some(transform_table), Some(draw_list))
        };

        let frame = frame::FrameResources::new(
            &instance,
            &device,
            &allocator,
            &surface,
            &surface_config,
            sample_count,
            vk::SwapchainKHR::null(),
        )?;

        let shader_compiler = Rc::new(
            ShaderCompiler::with_options(
                ShaderCompilerOptions::default()
                    .with_search_path("shaders")
                    .with_spirv_profile("spirv_1_5"),
            )
            .change_context(VulkanBackendError::ShaderCompilation)?,
        );

        let shaders =
            shader::VulkanShaderManager::new(device.logical().clone(), shader_compiler.clone());

        let compiled_main_shader = {
            let timer = Instant::now();

            let s = shader_compiler
                .compile(
                    "main",
                    [
                        engine_shader::ShaderEntrypoint::new(
                            "fragMain",
                            engine_shader::ShaderStage::Fragment,
                        ),
                        engine_shader::ShaderEntrypoint::new(
                            "vertMain",
                            engine_shader::ShaderStage::Vertex,
                        ),
                    ],
                )
                .change_context(VulkanBackendError::ShaderCompilation)?;

            debug!(
                "took {:.2}ms to compile the main shader",
                timer.elapsed().as_secs_f64() * 1000.0
            );

            s
        };

        let vk_main_shader = shaders
            .create_shader(&compiled_main_shader)
            .change_context(VulkanBackendError::ShaderCompilation)?;

        let pipeline_layout = pipeline::VulkanPipelineLayout::builder()
            .with_descriptor_set_layouts([texture_heap.descriptor_set_layout()])
            .with_push_constants(
                draw::DRAW_PUSH_CONSTANT_SIZE,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            )
            .build(device.logical().clone())
            .change_context(VulkanBackendError::PipelineOperation)?;

        let graphics_pipeline = pipeline::VulkanGraphicsPipeline::builder()
            .with_shaders([&vk_main_shader])
            .with_blending_enabled(false)
            .with_depth_test(true, vk::CompareOp::LESS)
            .with_depth_write(true)
            .with_color_attachment_format(surface_config.surface_format.format)
            .with_depth_attachment_format(depth::DEPTH_FORMAT)
            .with_sample_count(sample_count)
            .with_sample_shading(sample_count != vk::SampleCountFlags::TYPE_1, 0.2)
            .build(&device, "gpu-driven-multidraw", &pipeline_layout)
            .change_context(VulkanBackendError::PipelineOperation)?;

        // Verifies compute pipeline creation, dispatch, and
        // compute-to-transfer synchronization end to end. This is a
        // one-time development-time check, not a rendering feature, so
        // it only runs in debug builds.
        #[cfg(debug_assertions)]
        {
            let compiled_compute_demo_shader = shader_compiler
                .compile(
                    "compute_squares",
                    [engine_shader::ShaderEntrypoint::new(
                        "main",
                        engine_shader::ShaderStage::Compute,
                    )],
                )
                .change_context(VulkanBackendError::ShaderCompilation)?;

            let vk_compute_demo_shader = shaders
                .create_shader(&compiled_compute_demo_shader)
                .change_context(VulkanBackendError::ShaderCompilation)?;

            compute::run_self_check(&allocator, &command, &device, &vk_compute_demo_shader)
                .change_context(VulkanBackendError::ComputeSelfCheck)?;
        }

        let frame_sync = frame::VulkanFrameSync::new(device.logical().clone())
            .change_context(VulkanBackendError::FrameOperation)?;

        Ok(Self {
            frame,
            frame_sync,
            graphics_pipeline,
            pipeline_layout,
            draw_list,
            transform_table,
            material_table,
            texture_heap,
            scene_buffers,
            meshes,
            command,
            allocator,
            surface_config,
            sample_count,
            current_frame: 0,
            rendering_paused: false,
            vsync,
            device,
            surface,
            instance,
            entry,
        })
    }

    pub(super) fn prepare_frame(&mut self) -> core::result::Result<(), VulkanBackendError> {
        Ok(())
    }

    pub(super) fn render(
        &mut self,
        camera: &RenderCamera,
    ) -> core::result::Result<(), VulkanBackendError> {
        if self.rendering_paused {
            return Ok(());
        }

        let frame_index = self.current_frame;

        let logical_device = self.device.logical().clone();
        let logical = logical_device.handle();
        let command_buffer = self.command.render_command_buffer(frame_index);
        let in_flight = self.frame_sync.in_flight(frame_index);
        let in_flight_fences = [in_flight];

        // SAFETY: `in_flight` is a live fence owned by this backend.
        vk_try!("wait for in-flight frame fence", unsafe {
            logical.wait_for_fences(&in_flight_fences, true, u64::MAX)
        });

        // SAFETY: The swapchain and image-available semaphore are live.
        let (image_index, acquire_suboptimal) = match unsafe {
            self.frame.swapchain().loader().acquire_next_image(
                self.frame.swapchain().get(),
                u64::MAX,
                self.frame_sync.image_available(frame_index),
                vk::Fence::null(),
            )
        } {
            Ok(acquired) => acquired,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.recreate_swapchain(self.surface_config.extent)?;
                return Ok(());
            }
            Err(result) => {
                return Err(call_error::VulkanCallError::new(
                    "acquire next swapchain image",
                    result,
                )
                .into());
            }
        };

        let render_finished = self
            .frame
            .render_finished(image_index)
            .ok_or(VulkanBackendError::InvalidSwapchainImageIndex { image_index })?;

        // SAFETY: The fence is signaled because we waited above, and the
        // command buffer was allocated from a pool created with
        // RESET_COMMAND_BUFFER.
        vk_try!("reset graphics command buffer", unsafe {
            logical.reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
        });

        let scene_buffer_address = self.scene_buffers[frame_index]
            .write_and_get_address(self.surface_config.extent, camera)
            .map_err(|_| VulkanBackendError::SceneBufferOperation)?;

        self.record_render_commands(
            command_buffer,
            image_index,
            frame_index,
            scene_buffer_address,
        )?;

        // SAFETY: Reset immediately before submission so failures before this
        // point leave the fence signaled for the next frame.
        vk_try!("reset in-flight frame fence", unsafe {
            logical.reset_fences(&in_flight_fences)
        });

        let wait_semaphore_infos = [vk::SemaphoreSubmitInfo::default()
            .semaphore(self.frame_sync.image_available(frame_index))
            .stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)];

        let command_buffer_infos =
            [vk::CommandBufferSubmitInfo::default().command_buffer(command_buffer)];

        let signal_semaphore_infos = [vk::SemaphoreSubmitInfo::default()
            .semaphore(render_finished)
            .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)];

        let submit_infos = [vk::SubmitInfo2::default()
            .wait_semaphore_infos(&wait_semaphore_infos)
            .command_buffer_infos(&command_buffer_infos)
            .signal_semaphore_infos(&signal_semaphore_infos)];

        // SAFETY: Queue, command buffer, synchronization objects, and
        // submit-info slices are live through the call. The in-flight
        // fence tracks this submission.
        vk_try!("submit graphics command buffer", unsafe {
            logical.queue_submit2(self.device.graphics_queue(), &submit_infos, in_flight)
        });

        let wait_semaphores = [render_finished];

        let swapchains = [self.frame.swapchain().get()];

        let image_indices = [image_index];

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        // SAFETY: The swapchain belongs to the device queue family selected
        // for graphics and presentation, and the render-finished
        // semaphore is signaled by the submitted frame.
        let present_suboptimal = match unsafe {
            self.frame
                .swapchain()
                .loader()
                .queue_present(self.device.graphics_queue(), &present_info)
        } {
            Ok(suboptimal) => suboptimal,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => true,
            Err(result) => {
                return Err(
                    call_error::VulkanCallError::new("present swapchain image", result).into()
                );
            }
        };

        if acquire_suboptimal || present_suboptimal {
            self.recreate_swapchain(self.surface_config.extent)?;
        }

        self.current_frame = (self.current_frame + 1) % FRAMES_IN_FLIGHT;

        Ok(())
    }

    pub(super) fn resize(
        &mut self,
        extent: RenderExtent,
    ) -> core::result::Result<(), VulkanBackendError> {
        self.rendering_paused = extent.is_empty();

        if self.rendering_paused {
            return Ok(());
        }

        let extent = Extent2D { width: extent.width, height: extent.height };

        if extent.width == self.surface_config.extent.width
            && extent.height == self.surface_config.extent.height
        {
            return Ok(());
        }

        self.recreate_swapchain(extent)
    }

    fn recreate_swapchain(
        &mut self,
        extent: Extent2D,
    ) -> core::result::Result<(), VulkanBackendError> {
        // SAFETY: The logical device is live; waiting before replacement
        // keeps old swapchain images from being destroyed while
        // commands or presentation still reference them.
        vk_try!("wait for device idle before recreating swapchain", unsafe {
            self.device.logical().handle().device_wait_idle()
        });

        let surface_config = self
            .surface
            .make_config(self.device.physical(), extent, self.vsync)
            .map_err(|_| VulkanBackendError::RefreshSurface)?;

        if surface_config.surface_format.format != self.surface_config.surface_format.format {
            return Err(VulkanBackendError::SurfaceFormatChanged {
                old: self.surface_config.surface_format.format,
                new: surface_config.surface_format.format,
            });
        }

        let frame = frame::FrameResources::new(
            &self.instance,
            &self.device,
            &self.allocator,
            &self.surface,
            &surface_config,
            self.sample_count,
            self.frame.swapchain().get(),
        )?;

        self.frame = frame;
        self.surface_config = surface_config;

        Ok(())
    }

    fn record_render_commands(
        &mut self,
        command_buffer: vk::CommandBuffer,
        image_index: u32,
        frame_index: usize,
        scene_buffer_address: vk::DeviceAddress,
    ) -> core::result::Result<(), VulkanBackendError> {
        let image = self
            .frame
            .swapchain()
            .image(image_index)
            .ok_or(VulkanBackendError::InvalidSwapchainImageIndex { image_index })?;

        let image_view = self
            .frame
            .swapchain()
            .image_view(image_index)
            .ok_or(VulkanBackendError::InvalidSwapchainImageIndex { image_index })?;

        let logical_device = self.device.logical().clone();

        let logical = logical_device.handle();

        let extent = self.surface_config.extent;

        // SAFETY: `command_buffer` is a reset primary command buffer owned by
        // this backend.
        vk_try!("begin graphics command buffer", unsafe {
            logical.begin_command_buffer(command_buffer, &vk::CommandBufferBeginInfo::default())
        });

        self.frame.depth_mut(frame_index).transition_for_render(command_buffer);

        self.transition_color_image(
            command_buffer,
            image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags2::NONE,
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
        );

        let clear_value = vk::ClearValue {
            color: vk::ClearColorValue { float32: [5.0 / 255.0, 5.0 / 255.0, 5.0 / 255.0, 1.0] },
        };

        // With MSAA, rendering targets the multisampled color image and
        // resolves into the swapchain image in the same rendering
        // instance; the swapchain image never needs a separate resolve
        // pass. Without MSAA, rendering targets the swapchain image
        // directly, matching the previous single-sample behavior.
        let color_attachment = if let Some(msaa_image) = self.frame.color(frame_index) {
            let msaa_view = msaa_image.view().ok_or(VulkanBackendError::MsaaColorViewMissing)?;

            self.transition_color_image(
                command_buffer,
                msaa_image.handle(),
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                vk::AccessFlags2::NONE,
                vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
                vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
            );

            vk::RenderingAttachmentInfo::default()
                .image_view(msaa_view)
                .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .resolve_mode(vk::ResolveModeFlags::AVERAGE)
                .resolve_image_view(image_view)
                .resolve_image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::DONT_CARE)
                .clear_value(clear_value)
        } else {
            vk::RenderingAttachmentInfo::default()
                .image_view(image_view)
                .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .clear_value(clear_value)
        };

        let color_attachments = [color_attachment];

        let depth_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(self.frame.depth(frame_index).view())
            .image_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .clear_value(vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 },
            });

        let render_area = vk::Rect2D { offset: vk::Offset2D::default(), extent };

        let rendering_info = vk::RenderingInfo::default()
            .render_area(render_area)
            .layer_count(1)
            .color_attachments(&color_attachments)
            .depth_attachment(&depth_attachment);

        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: extent.width as f32,
            height: extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };

        let viewports = [viewport];

        let scissors = [render_area];

        // SAFETY: The command buffer is recording, the dynamic-rendering
        // attachment references a live swapchain image view, and the
        // graphics pipeline was created for this color format.
        unsafe {
            logical.cmd_begin_rendering(command_buffer, &rendering_info);

            logical.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.graphics_pipeline.get(),
            );

            logical.cmd_set_viewport(command_buffer, 0, &viewports);

            logical.cmd_set_scissor(command_buffer, 0, &scissors);

            if let (Some(draw_list), Some(material_table), Some(transform_table)) =
                (&self.draw_list, &self.material_table, &self.transform_table)
            {
                let descriptor_sets = [self.texture_heap.descriptor_set()];

                logical.cmd_bind_descriptor_sets(
                    command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline_layout.get(),
                    0,
                    &descriptor_sets,
                    &[],
                );

                let push_constants = draw_list.push_constants(
                    material_table.device_address(),
                    transform_table.device_address(),
                    scene_buffer_address,
                );

                logical.cmd_push_constants(
                    command_buffer,
                    self.pipeline_layout.get(),
                    vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                    0,
                    push_constants.as_bytes(),
                );

                logical.cmd_draw_indirect(
                    command_buffer,
                    draw_list.indirect_buffer(),
                    0,
                    draw_list.draw_count(),
                    draw_list.indirect_stride(),
                );
            }

            logical.cmd_end_rendering(command_buffer);
        }

        self.transition_color_image(
            command_buffer,
            image,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
            vk::PipelineStageFlags2::NONE,
            vk::AccessFlags2::NONE,
        );

        // SAFETY: Recording was begun above and all commands have been
        // emitted.
        vk_try!("end graphics command buffer", unsafe {
            logical.end_command_buffer(command_buffer)
        });

        Ok(())
    }

    /// Transitions a color-aspect image (the swapchain image or the
    /// MSAA color attachment) between layouts.
    #[allow(clippy::too_many_arguments)]
    fn transition_color_image(
        &self,
        command_buffer: vk::CommandBuffer,
        image: vk::Image,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        src_stage: vk::PipelineStageFlags2,
        src_access: vk::AccessFlags2,
        dst_stage: vk::PipelineStageFlags2,
        dst_access: vk::AccessFlags2,
    ) {
        let barrier = vk::ImageMemoryBarrier2::default()
            .src_stage_mask(src_stage)
            .src_access_mask(src_access)
            .dst_stage_mask(dst_stage)
            .dst_access_mask(dst_access)
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        let barriers = [barrier];

        let dependency_info = vk::DependencyInfo::default().image_memory_barriers(&barriers);

        // SAFETY: `command_buffer` is recording, and the barrier references
        // a live color image acquired or owned by this backend.
        unsafe {
            self.device
                .logical()
                .handle()
                .cmd_pipeline_barrier2(command_buffer, &dependency_info);
        }
    }
}

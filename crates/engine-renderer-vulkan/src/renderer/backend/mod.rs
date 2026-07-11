#[macro_use]
mod call_error;

mod allocator;
mod command;
mod device;
mod frame;
mod instance;
mod pipeline;
mod shader;
mod surface;
mod swapchain;

use std::rc::Rc;

use ash::vk::{self, Extent2D};
use common::logging::macros::warn;
use engine_renderer_api::{RenderExtent, RenderWindow};
use engine_shader::{ShaderCompiler, ShaderCompilerOptions};
use error_stack::{Report, ResultExt};
use thiserror::Error;

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

    /// Vulkan surface refresh failed.
    #[error("failed to refresh vulkan surface details")]
    RefreshSurface,

    /// Vulkan shader compiler operation failed.
    #[error("shader compiler operation failed")]
    ShaderCompilation,

    /// Vulkan API call returned an error value.
    #[error(transparent)]
    UnexpectedResult(#[from] call_error::VulkanCallError),

    /// The swapchain returned an image index without a matching image and view.
    #[error("swapchain returned invalid image index {image_index}")]
    InvalidSwapchainImageIndex {
        /// Image index returned by Vulkan.
        image_index: u32,
    },

    /// Surface format changed after the triangle pipeline was created.
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
    // Rust drops fields in declaration order. Keep Vulkan children above their parents.
    frame_sync: frame::VulkanFrameSync,
    graphics_pipeline: pipeline::VulkanGraphicsPipeline,
    pipeline_layout: pipeline::VulkanPipelineLayout,
    command: command::VulkanCommand,
    allocator: allocator::VulkanAllocator,
    swapchain: swapchain::VulkanSwapchain,
    surface_config: surface::SurfaceConfig,
    rendering_paused: bool,
    vsync: bool,
    device: device::VulkanDevice,
    surface: surface::VulkanSurface,
    instance: instance::VulkanInstance,
    entry: ash::Entry,
}

impl Drop for VulkanBackend {
    fn drop(&mut self) {
        // SAFETY: The logical device is live, and waiting here keeps backend-owned resources from
        // being destroyed while their final submitted frame may still be executing.
        if let Err(result) = unsafe { self.device.logical().handle().device_wait_idle() } {
            warn!("failed to wait for Vulkan device idle during backend drop: {result:?}");
        }
    }
}

impl VulkanBackend {
    pub(super) fn new(
        rw: &dyn RenderWindow,
        vsync: bool,
    ) -> error_stack::Result<Self, VulkanBackendError> {
        let display_handle = rw
            .display_handle()
            .map_err(|error| {
                Report::new(VulkanBackendError::DisplayHandle).attach_printable(error.to_string())
            })?
            .as_raw();
        let window_handle = rw
            .window_handle()
            .map_err(|error| {
                Report::new(VulkanBackendError::WindowHandle).attach_printable(error.to_string())
            })?
            .as_raw();

        // SAFETY: Loading the Vulkan entry only performs dynamic symbol lookup;
        // the owned entry is stored in the backend and outlives all objects
        // created from it.
        let entry = unsafe { ash::Entry::load() }.map_err(|err| {
            Report::new(VulkanBackendError::EntryLoadFailure).attach_printable(err.to_string())
        })?;

        let instance = instance::VulkanInstance::new(&entry, display_handle)
            .change_context(VulkanBackendError::InstanceOperation)?;

        let surface = surface::VulkanSurface::new(&entry, &instance, display_handle, window_handle)
            .change_context(VulkanBackendError::SurfaceOperation)?;

        let device = device::VulkanDevice::new(&instance, &surface)
            .change_context(VulkanBackendError::DeviceOperation)?;

        let RenderExtent { width, height } = rw.size();

        let surface_config = surface
            .make_config(device.physical(), Extent2D { width, height }, vsync)
            .change_context(VulkanBackendError::RefreshSurface)?;

        // WARN: The C++ equivalent recreates fences and semaphores in the
        // constructor for some reason.
        // Figure out the reason, and if valid, implement that.
        let swapchain = swapchain::VulkanSwapchain::new(
            &instance,
            device.logical().clone(),
            &surface,
            &surface_config,
        )
        .change_context(VulkanBackendError::SwapchainOperation)?;

        let allocator =
            allocator::VulkanAllocator::new(&instance, device.logical(), device.physical())
                .change_context(VulkanBackendError::AllocatorOperation)?;

        let command =
            command::VulkanCommand::new(device.logical().clone(), device.queue_families())
                .change_context(VulkanBackendError::CommandOperation)?;

        let compiler = Rc::new(
            ShaderCompiler::with_options(
                ShaderCompilerOptions::default()
                    .with_search_path("shaders")
                    .with_spirv_profile("spirv_1_5")
                    .with_assembly_output_dir("shaders"),
            )
            .change_context(VulkanBackendError::ShaderCompilation)?,
        );

        let shaders = shader::VulkanShaderManager::new(device.logical().clone(), compiler.clone());

        let compiled_main_shader = compiler
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

        let vk_main_shader = shaders
            .create_shader(&compiled_main_shader)
            .change_context(VulkanBackendError::ShaderCompilation)?;

        let pipeline_layout = pipeline::VulkanPipelineLayout::builder()
            .build(device.logical().clone())
            .change_context(VulkanBackendError::PipelineOperation)?;

        let graphics_pipeline = pipeline::VulkanGraphicsPipeline::builder()
            .with_shaders([&vk_main_shader])
            .with_blending_enabled(false)
            .with_depth_write(false)
            .with_color_attachment_format(surface_config.surface_format.format)
            .with_depth_attachment_format(vk::Format::UNDEFINED)
            .build(&device, "triangle", &pipeline_layout)
            .change_context(VulkanBackendError::PipelineOperation)?;

        let frame_sync =
            frame::VulkanFrameSync::new(device.logical().clone(), swapchain.image_count())
                .change_context(VulkanBackendError::FrameOperation)?;

        Ok(Self {
            frame_sync,
            graphics_pipeline,
            pipeline_layout,
            command,
            allocator,
            swapchain,
            surface_config,
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

    pub(super) fn render(&mut self) -> core::result::Result<(), VulkanBackendError> {
        if self.rendering_paused {
            return Ok(());
        }

        let logical = self.device.logical().handle();
        let command_buffer = self.command.graphics_command_buffer();
        let in_flight = self.frame_sync.in_flight();
        let in_flight_fences = [in_flight];

        // SAFETY: `in_flight` is a live fence owned by this backend.
        vk_try!("wait for in-flight frame fence", unsafe {
            logical.wait_for_fences(&in_flight_fences, true, u64::MAX)
        });

        // SAFETY: The swapchain and image-available semaphore are live.
        let (image_index, acquire_suboptimal) = match unsafe {
            self.swapchain.loader().acquire_next_image(
                self.swapchain.get(),
                u64::MAX,
                self.frame_sync.image_available(),
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
            .frame_sync
            .render_finished(image_index)
            .ok_or(VulkanBackendError::InvalidSwapchainImageIndex { image_index })?;

        // SAFETY: The fence is signaled because we waited above, and the command buffer was
        // allocated from a pool created with RESET_COMMAND_BUFFER.
        vk_try!("reset graphics command buffer", unsafe {
            logical.reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
        });

        self.record_triangle_commands(command_buffer, image_index)?;

        // SAFETY: Reset immediately before submission so failures before this point leave the fence
        // signaled for the next frame.
        vk_try!("reset in-flight frame fence", unsafe { logical.reset_fences(&in_flight_fences) });

        let wait_semaphore_infos = [vk::SemaphoreSubmitInfo::default()
            .semaphore(self.frame_sync.image_available())
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

        // SAFETY: Queue, command buffer, synchronization objects, and submit-info slices are live
        // through the call. The in-flight fence tracks this submission.
        vk_try!("submit graphics command buffer", unsafe {
            logical.queue_submit2(self.device.graphics_queue(), &submit_infos, in_flight)
        });

        let wait_semaphores = [render_finished];
        let swapchains = [self.swapchain.get()];
        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        // SAFETY: The swapchain belongs to the device queue family selected for graphics and
        // presentation, and the render-finished semaphore is signaled by the submitted frame.
        let present_suboptimal = match unsafe {
            self.swapchain.loader().queue_present(self.device.graphics_queue(), &present_info)
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
        // SAFETY: The logical device is live; waiting before replacement keeps old swapchain images
        // from being destroyed while commands or presentation still reference them.
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

        let swapchain = swapchain::VulkanSwapchain::new_replacing(
            &self.instance,
            self.device.logical().clone(),
            &self.surface,
            &surface_config,
            self.swapchain.get(),
        )
        .map_err(|_| VulkanBackendError::SwapchainOperation)?;
        let frame_sync =
            frame::VulkanFrameSync::new(self.device.logical().clone(), swapchain.image_count())?;

        self.frame_sync = frame_sync;
        self.swapchain = swapchain;
        self.surface_config = surface_config;

        Ok(())
    }

    #[allow(clippy::as_conversions, clippy::cast_precision_loss)]
    fn record_triangle_commands(
        &self,
        command_buffer: vk::CommandBuffer,
        image_index: u32,
    ) -> core::result::Result<(), VulkanBackendError> {
        let image = self
            .swapchain
            .image(image_index)
            .ok_or(VulkanBackendError::InvalidSwapchainImageIndex { image_index })?;
        let image_view = self
            .swapchain
            .image_view(image_index)
            .ok_or(VulkanBackendError::InvalidSwapchainImageIndex { image_index })?;
        let logical = self.device.logical().handle();
        let extent = self.surface_config.extent;

        // SAFETY: `command_buffer` is a reset primary command buffer owned by this backend.
        vk_try!("begin graphics command buffer", unsafe {
            logical.begin_command_buffer(command_buffer, &vk::CommandBufferBeginInfo::default())
        });

        self.transition_swapchain_image(
            command_buffer,
            image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags2::NONE,
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
        );

        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(image_view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue { float32: [0.02, 0.02, 0.03, 1.0] },
            });
        let color_attachments = [color_attachment];
        let render_area = vk::Rect2D { offset: vk::Offset2D::default(), extent };
        let rendering_info = vk::RenderingInfo::default()
            .render_area(render_area)
            .layer_count(1)
            .color_attachments(&color_attachments);
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

        // SAFETY: The command buffer is recording, the dynamic-rendering attachment references a
        // live swapchain image view, and the graphics pipeline was created for this color format.
        unsafe {
            logical.cmd_begin_rendering(command_buffer, &rendering_info);
            logical.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.graphics_pipeline.get(),
            );
            logical.cmd_set_viewport(command_buffer, 0, &viewports);
            logical.cmd_set_scissor(command_buffer, 0, &scissors);
            logical.cmd_draw(command_buffer, 3, 1, 0, 0);
            logical.cmd_end_rendering(command_buffer);
        }

        self.transition_swapchain_image(
            command_buffer,
            image,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
            vk::PipelineStageFlags2::NONE,
            vk::AccessFlags2::NONE,
        );

        // SAFETY: Recording was begun above and all commands have been emitted.
        vk_try!("end graphics command buffer", unsafe {
            logical.end_command_buffer(command_buffer)
        });

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn transition_swapchain_image(
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

        // SAFETY: `command_buffer` is recording, and the barrier references the live swapchain
        // image acquired for this frame.
        unsafe {
            self.device.logical().handle().cmd_pipeline_barrier2(command_buffer, &dependency_info);
        }
    }
}

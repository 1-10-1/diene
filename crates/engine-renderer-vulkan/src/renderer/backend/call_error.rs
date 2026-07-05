use std::panic::Location;

use thiserror::Error;

#[derive(Error)]
pub(in crate::renderer::backend) struct VulkanCallError {
    operation: &'static str,
    result: ash::vk::Result,
    location: &'static Location<'static>,

    #[cfg(debug_assertions)]
    backtrace: std::backtrace::Backtrace,
}

impl std::fmt::Debug for VulkanCallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self, f)
    }
}

impl std::fmt::Display for VulkanCallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "Vulkan operation `{}` failed with result {} ({:#?}) at {}",
            self.operation, self.result, self.result, self.location
        ))
    }
}

impl VulkanCallError {
    #[track_caller]
    #[cold]
    pub(in crate::renderer::backend) fn new(
        operation: &'static str,
        result: ash::vk::Result,
    ) -> Self {
        Self {
            operation,
            result,
            location: Location::caller(),

            #[cfg(debug_assertions)]
            backtrace: std::backtrace::Backtrace::capture(),
        }
    }
}

macro_rules! vk_try {
    ($operation:expr, $expr:expr $(,)?) => {
        match $expr {
            Ok(value) => value,
            Err(err) => {
                return Err($crate::renderer::backend::call_error::VulkanCallError::new(
                    $operation, err,
                )
                .into())
            }
        }
    };
    ($expr:expr) => {
        vk_try!(stringify!($expr), $expr)
    };
}

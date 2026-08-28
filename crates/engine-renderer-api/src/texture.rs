use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::glm;

/// Width and height of a 2D texture in pixels.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextureExtent {
    /// Texture width in pixels.
    pub width: u32,

    /// Texture height in pixels.
    pub height: u32,
}

impl TextureExtent {
    /// Creates a texture extent in pixels.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Returns `true` when either dimension is zero.
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    fn rgba8_byte_len(self) -> Option<usize> {
        usize::try_from(self.width)
            .ok()?
            .checked_mul(usize::try_from(self.height).ok()?)?
            .checked_mul(ImageData::RGBA8_BYTES_PER_PIXEL)
    }
}

/// Errors returned while creating CPU texture data.
#[derive(Error, Debug)]
pub enum TextureDataError {
    /// Texture extent must have non-zero width and height.
    #[error("texture extent must be non-zero: {0:?}")]
    EmptyExtent(TextureExtent),

    /// RGBA8 byte length does not match the supplied extent.
    #[error("invalid rgba8 texture byte length: expected {expected}, got {actual}")]
    InvalidRgba8ByteLength {
        /// Expected number of bytes for the extent.
        expected: usize,

        /// Actual byte count supplied by the caller.
        actual: usize,
    },

    /// RGBA8 byte length overflowed `usize` for the supplied extent.
    #[error("rgba8 byte length overflowed for extent {0:?}")]
    ByteLengthOverflow(TextureExtent),

    /// Failed to open an image file.
    #[error("failed to open image file {}", path.display())]
    ImageOpen {
        /// Image path that failed to open.
        path: PathBuf,

        /// Underlying I/O error.
        #[source]
        source: Box<std::io::Error>,
    },

    /// Failed to decode an image file.
    #[error("failed to decode image file {}", path.display())]
    ImageDecode {
        /// Image path that failed to decode.
        path: PathBuf,

        /// Underlying image decoding error.
        #[source]
        source: Box<image::ImageError>,
    },
}

/// CPU-side RGBA8 image pixels.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageData {
    extent: TextureExtent,
    pixels: Vec<u8>,
}

impl Default for ImageData {
    fn default() -> Self {
        Self::checkerboard()
    }
}

impl ImageData {
    /// Bytes per pixel for the image data format.
    pub const RGBA8_BYTES_PER_PIXEL: usize = 4;

    /// Creates a one-pixel RGBA8 image with the supplied color.
    pub fn solid_rgba8(color: glm::U8Vec4) -> Self {
        Self { extent: TextureExtent::new(1, 1), pixels: color.as_slice().to_vec() }
    }

    /// Creates the default magenta/black checkerboard image.
    pub fn checkerboard() -> Self {
        let extent = TextureExtent::new(16, 16);
        let pixels = (0..extent.height)
            .flat_map(|y| {
                (0..extent.width).flat_map(move |x| {
                    if (x / 8 + y / 8) % 2 == 0 {
                        [255, 0, 255, 255]
                    } else {
                        [0, 0, 0, 255]
                    }
                })
            })
            .collect();

        Self { extent, pixels }
    }

    /// Creates RGBA8 image data from raw bytes.
    pub fn from_rgba8(
        extent: TextureExtent,
        pixels: impl Into<Vec<u8>>,
    ) -> Result<Self, TextureDataError> {
        if extent.is_empty() {
            return Err(TextureDataError::EmptyExtent(extent));
        }

        let pixels = pixels.into();

        let expected =
            extent.rgba8_byte_len().ok_or(TextureDataError::ByteLengthOverflow(extent))?;
        let actual = pixels.len();

        if actual != expected {
            return Err(TextureDataError::InvalidRgba8ByteLength { expected, actual });
        }

        Ok(Self { extent, pixels })
    }

    /// Decodes an image file into RGBA8 image data using the `image`
    /// crate.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, TextureDataError> {
        let path = path.as_ref();

        let reader = image::ImageReader::open(path).map_err(|source| {
            TextureDataError::ImageOpen { path: path.to_owned(), source: Box::new(source) }
        })?;

        let image = reader.decode().map_err(|source| TextureDataError::ImageDecode {
            path: path.to_owned(),
            source: Box::new(source),
        })?;
        let rgba = image.into_rgba8();

        Self::from_rgba8(TextureExtent::new(rgba.width(), rgba.height()), rgba.into_raw())
    }

    /// Returns the image extent.
    pub const fn extent(&self) -> TextureExtent {
        self.extent
    }

    /// Returns the image width in pixels.
    pub const fn width(&self) -> u32 {
        self.extent.width
    }

    /// Returns the image height in pixels.
    pub const fn height(&self) -> u32 {
        self.extent.height
    }

    /// Returns the raw RGBA8 image bytes.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Returns the number of raw RGBA8 bytes.
    pub fn byte_len(&self) -> usize {
        self.pixels.len()
    }

    /// Consumes the image data and returns its raw RGBA8 bytes.
    pub fn into_pixels(self) -> Vec<u8> {
        self.pixels
    }
}

/// CPU-side texture payload plus upload metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextureData {
    label: Option<String>,
    image: ImageData,
}

impl Default for TextureData {
    fn default() -> Self {
        Self { label: Some("default-checkerboard".to_owned()), image: ImageData::default() }
    }
}

impl TextureData {
    /// Bytes per pixel for the texture data format.
    pub const RGBA8_BYTES_PER_PIXEL: usize = ImageData::RGBA8_BYTES_PER_PIXEL;

    /// Creates a labeled one-pixel RGBA8 texture with the supplied
    /// color.
    pub fn solid_rgba8(label: impl Into<String>, color: glm::U8Vec4) -> Self {
        Self { label: Some(label.into()), image: ImageData::solid_rgba8(color) }
    }

    /// Creates unlabeled RGBA8 texture data from raw bytes.
    pub fn from_rgba8(
        extent: TextureExtent,
        pixels: impl Into<Vec<u8>>,
    ) -> Result<Self, TextureDataError> {
        Ok(Self { label: None, image: ImageData::from_rgba8(extent, pixels)? })
    }

    /// Creates labeled RGBA8 texture data from raw bytes.
    pub fn from_rgba8_with_label(
        label: impl Into<String>,
        extent: TextureExtent,
        pixels: impl Into<Vec<u8>>,
    ) -> Result<Self, TextureDataError> {
        Ok(Self { label: Some(label.into()), image: ImageData::from_rgba8(extent, pixels)? })
    }

    /// Decodes an image file into labeled RGBA8 texture data.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, TextureDataError> {
        let path = path.as_ref();

        Ok(Self { label: Some(path.display().to_string()), image: ImageData::from_file(path)? })
    }

    /// Creates labeled texture data from decoded image data.
    pub fn from_image(label: impl Into<String>, image: ImageData) -> Self {
        Self { label: Some(label.into()), image }
    }

    /// Returns the optional texture label or source path.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Returns the decoded image payload.
    pub const fn image(&self) -> &ImageData {
        &self.image
    }

    /// Returns the texture extent.
    pub const fn extent(&self) -> TextureExtent {
        self.image.extent()
    }

    /// Returns the texture width in pixels.
    pub const fn width(&self) -> u32 {
        self.image.width()
    }

    /// Returns the texture height in pixels.
    pub const fn height(&self) -> u32 {
        self.image.height()
    }

    /// Returns the raw RGBA8 texture bytes.
    pub fn pixels(&self) -> &[u8] {
        self.image.pixels()
    }

    /// Returns the number of raw RGBA8 bytes.
    pub fn byte_len(&self) -> usize {
        self.image.byte_len()
    }

    /// Consumes the texture data and returns its decoded image
    /// payload.
    pub fn into_image(self) -> ImageData {
        self.image
    }

    /// Consumes the texture data and returns its raw RGBA8 bytes.
    pub fn into_pixels(self) -> Vec<u8> {
        self.image.into_pixels()
    }
}

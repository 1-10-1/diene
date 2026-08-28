use std::{error::Error, fmt};

use crate::{TextureData, glm};

/// CPU-side mesh vertex consumed by renderer backends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshVertex {
    /// Homogeneous clip-space or object-space position.
    pub position: glm::Vec4,

    /// Vertex color multiplier.
    pub color: glm::Vec4,

    /// Two-dimensional texture coordinates.
    pub uv: glm::Vec2,
}

impl MeshVertex {
    /// Creates a vertex from position, color, and UV coordinates.
    pub const fn new(position: glm::Vec4, color: glm::Vec4, uv: glm::Vec2) -> Self {
        Self { position, color, uv }
    }
}

/// Errors returned while creating CPU mesh data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeshDataError {
    /// Mesh has no vertices.
    EmptyVertices,

    /// Mesh has no indices.
    EmptyIndices,
}

impl fmt::Display for MeshDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyVertices => {
                formatter.write_str("mesh data must contain at least one vertex")
            }
            Self::EmptyIndices => formatter.write_str("mesh data must contain at least one index"),
        }
    }
}

impl Error for MeshDataError {}

/// CPU-side indexed mesh data ready for GPU upload.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshData {
    vertices: Vec<MeshVertex>,
    indices: Vec<u32>,
}

impl MeshData {
    /// Creates validated indexed mesh data.
    pub fn new(
        vertices: impl Into<Vec<MeshVertex>>,
        indices: impl Into<Vec<u32>>,
    ) -> Result<Self, MeshDataError> {
        let vertices = vertices.into();
        let indices = indices.into();

        if vertices.is_empty() {
            return Err(MeshDataError::EmptyVertices);
        }

        if indices.is_empty() {
            return Err(MeshDataError::EmptyIndices);
        }

        Ok(Self { vertices, indices })
    }

    /// Creates a textured quad centered at `center` with the supplied
    /// size.
    #[must_use]
    pub fn quad(center: glm::Vec3, size: [f32; 2], color: glm::Vec4) -> Self {
        let [width, height] = size;
        let half_width = width * 0.5;
        let half_height = height * 0.5;
        let (x, y, z) = (center.x, center.y, center.z);

        let vertices = vec![
            MeshVertex::new(
                glm::Vec4::new(x - half_width, y - half_height, z, 1.0),
                color,
                glm::Vec2::new(0.0, 1.0),
            ),
            MeshVertex::new(
                glm::Vec4::new(x + half_width, y - half_height, z, 1.0),
                color,
                glm::Vec2::new(1.0, 1.0),
            ),
            MeshVertex::new(
                glm::Vec4::new(x + half_width, y + half_height, z, 1.0),
                color,
                glm::Vec2::new(1.0, 0.0),
            ),
            MeshVertex::new(
                glm::Vec4::new(x - half_width, y + half_height, z, 1.0),
                color,
                glm::Vec2::new(0.0, 0.0),
            ),
        ];

        let indices = vec![0, 1, 2, 2, 3, 0];

        Self { vertices, indices }
    }

    /// Returns mesh vertices.
    pub fn vertices(&self) -> &[MeshVertex] {
        &self.vertices
    }

    /// Returns mesh indices.
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }
}

/// CPU-side material data used to build renderer material tables.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialData {
    label: Option<String>,
    albedo_texture: Option<TextureData>,
    tint: glm::Vec4,
}

impl Default for MaterialData {
    fn default() -> Self {
        Self { label: None, albedo_texture: None, tint: glm::Vec4::repeat(1.0) }
    }
}

impl MaterialData {
    /// Creates an unnamed material with the supplied color tint.
    #[must_use]
    pub const fn tinted(tint: glm::Vec4) -> Self {
        Self { label: None, albedo_texture: None, tint }
    }

    /// Sets a debug/source label for this material.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets the albedo texture payload for this material.
    #[must_use]
    pub fn with_albedo_texture(mut self, texture: TextureData) -> Self {
        self.albedo_texture = Some(texture);
        self
    }

    /// Returns the optional material label.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Returns the optional albedo texture payload.
    pub const fn albedo_texture(&self) -> Option<&TextureData> {
        self.albedo_texture.as_ref()
    }

    /// Returns the material tint multiplier.
    pub const fn tint(&self) -> glm::Vec4 {
        self.tint
    }
}

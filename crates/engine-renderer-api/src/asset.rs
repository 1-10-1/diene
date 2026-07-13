use std::{error::Error, fmt};

use crate::TextureData;

/// CPU-side mesh vertex layout consumed by GPU-driven renderer
/// backends.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshVertex {
    /// Homogeneous clip-space or object-space position.
    pub position: [f32; 4],

    /// Vertex color multiplier.
    pub color: [f32; 4],

    /// Texture coordinates in `xy`; remaining components are
    /// reserved.
    pub uv: [f32; 4],
}

impl MeshVertex {
    /// Creates a vertex from position, color, and UV coordinates.
    #[must_use]
    pub const fn new(position: [f32; 4], color: [f32; 4], uv: [f32; 2]) -> Self {
        Self { position, color, uv: [uv[0], uv[1], 0.0, 0.0] }
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
    pub fn quad(center: [f32; 3], size: [f32; 2], color: [f32; 4]) -> Self {
        let half_width = size[0] * 0.5;
        let half_height = size[1] * 0.5;
        let [x, y, z] = center;
        let vertices = vec![
            MeshVertex::new([x - half_width, y - half_height, z, 1.0], color, [0.0, 1.0]),
            MeshVertex::new([x + half_width, y - half_height, z, 1.0], color, [1.0, 1.0]),
            MeshVertex::new([x + half_width, y + half_height, z, 1.0], color, [1.0, 0.0]),
            MeshVertex::new([x - half_width, y + half_height, z, 1.0], color, [0.0, 0.0]),
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
    tint: [f32; 4],
}

impl Default for MaterialData {
    fn default() -> Self {
        Self { label: None, albedo_texture: None, tint: [1.0, 1.0, 1.0, 1.0] }
    }
}

impl MaterialData {
    /// Creates an unnamed material with the supplied color tint.
    #[must_use]
    pub const fn tinted(tint: [f32; 4]) -> Self {
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
    pub const fn tint(&self) -> [f32; 4] {
        self.tint
    }
}

#[cfg(test)]
mod tests {
    use super::{MaterialData, MeshData, MeshDataError};

    #[test]
    fn quad_mesh_has_expected_shape() {
        let mesh = MeshData::quad([0.0, 0.0, 0.0], [2.0, 4.0], [1.0; 4]);

        assert_eq!(mesh.vertices().len(), 4);
        assert_eq!(mesh.indices(), &[0, 1, 2, 2, 3, 0]);
        assert_f32x4_eq(mesh.vertices()[0].position, [-1.0, -2.0, 0.0, 1.0]);
        assert_f32x4_eq(mesh.vertices()[2].uv, [1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn mesh_data_rejects_empty_inputs() {
        assert_eq!(MeshData::new([], [0]), Err(MeshDataError::EmptyVertices));
        assert_eq!(
            MeshData::new([MeshData::quad([0.0; 3], [1.0; 2], [1.0; 4]).vertices()[0]], []),
            Err(MeshDataError::EmptyIndices)
        );
    }

    #[test]
    fn material_data_defaults_to_white_default_texture_slot() {
        let material = MaterialData::default();

        assert_f32x4_eq(material.tint(), [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(material.label(), None);
        assert!(material.albedo_texture().is_none());
    }

    fn assert_f32x4_eq(actual: [f32; 4], expected: [f32; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() <= f32::EPSILON);
        }
    }
}

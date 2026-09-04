use std::{error::Error, fmt};

use crate::{MaterialData, MeshData, glm};

/// Camera parameters used to build the renderer view-projection
/// matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderCamera {
    eye: glm::Vec3,
    target: glm::Vec3,
    up: glm::Vec3,
    vertical_fov_radians: f32,
    near_plane: f32,
    far_plane: f32,
}

impl Default for RenderCamera {
    fn default() -> Self {
        Self {
            eye: glm::Vec3::new(0.0, 0.25, 3.2),
            target: glm::Vec3::zeros(),
            up: glm::Vec3::y(),
            vertical_fov_radians: 45.0_f32.to_radians(),
            near_plane: 0.1,
            far_plane: 100.0,
        }
    }
}

impl RenderCamera {
    /// Creates camera data from eye, target, up, field-of-view, and
    /// clip planes.
    pub const fn new(
        eye: glm::Vec3,
        target: glm::Vec3,
        up: glm::Vec3,
        vertical_fov_radians: f32,
        near_plane: f32,
        far_plane: f32,
    ) -> Self {
        Self { eye, target, up, vertical_fov_radians, near_plane, far_plane }
    }

    /// Returns the camera position in world space.
    pub const fn eye(self) -> glm::Vec3 {
        self.eye
    }

    /// Returns the camera look target in world space.
    pub const fn target(self) -> glm::Vec3 {
        self.target
    }

    /// Returns the camera up direction.
    pub const fn up(self) -> glm::Vec3 {
        self.up
    }

    /// Returns the vertical field of view in radians.
    pub const fn vertical_fov_radians(self) -> f32 {
        self.vertical_fov_radians
    }

    /// Returns the near clipping plane distance.
    pub const fn near_plane(self) -> f32 {
        self.near_plane
    }

    /// Returns the far clipping plane distance.
    pub const fn far_plane(self) -> f32 {
        self.far_plane
    }
}

/// Backend-neutral model transform consumed by renderer backends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderTransform {
    model_matrix: glm::Mat4,
}

impl Default for RenderTransform {
    fn default() -> Self {
        Self::identity()
    }
}

impl RenderTransform {
    /// Creates an identity model transform.
    pub fn identity() -> Self {
        Self { model_matrix: glm::identity() }
    }

    /// Creates a model transform from translation and scale.
    pub fn from_translation_scale(translation: glm::Vec3, scale: glm::Vec3) -> Self {
        let [[tx, ty, tz]] = translation.data.0;
        let [[sx, sy, sz]] = scale.data.0;

        Self {
            model_matrix: glm::Mat4::new(
                sx, 0.0, 0.0, tx, 0.0, sy, 0.0, ty, 0.0, 0.0, sz, tz, 0.0, 0.0, 0.0, 1.0,
            ),
        }
    }

    /// Creates a new model transform.
    pub fn new(model_matrix: glm::Mat4) -> Self {
        Self { model_matrix }
    }

    /// Returns the model matrix.
    pub fn model_matrix(self) -> glm::Mat4 {
        self.model_matrix
    }
}

/// One submitted render object referencing scene mesh and material
/// arrays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderObject {
    mesh_index: usize,
    material_index: usize,
    transform: RenderTransform,
}

impl RenderObject {
    /// Creates a render object with scene-local mesh and material
    /// indices.
    pub const fn new(mesh_index: usize, material_index: usize, transform: RenderTransform) -> Self {
        Self { mesh_index, material_index, transform }
    }

    /// Returns the referenced mesh index.
    pub const fn mesh_index(self) -> usize {
        self.mesh_index
    }

    /// Returns the referenced material index.
    pub const fn material_index(self) -> usize {
        self.material_index
    }

    /// Returns the object model transform.
    pub const fn transform(self) -> RenderTransform {
        self.transform
    }
}

/// Errors returned while creating a backend-neutral render scene.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderSceneError {
    /// A render object references a mesh index outside the scene mesh
    /// array.
    MeshIndexUnavailable {
        /// Object index in the scene object array.
        object_index: usize,
        /// Referenced mesh index.
        mesh_index: usize,
        /// Number of meshes in the scene.
        mesh_count: usize,
    },

    /// A render object references a material index outside the scene
    /// material array.
    MaterialIndexUnavailable {
        /// Object index in the scene object array.
        object_index: usize,
        /// Referenced material index.
        material_index: usize,
        /// Number of materials in the scene.
        material_count: usize,
    },
}

impl fmt::Display for RenderSceneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MeshIndexUnavailable { object_index, mesh_index, mesh_count } => write!(
                formatter,
                "render object {object_index} references mesh {mesh_index}, but scene has \
                 {mesh_count} meshes"
            ),
            Self::MaterialIndexUnavailable { object_index, material_index, material_count } => {
                write!(
                    formatter,
                    "render object {object_index} references material {material_index}, but scene \
                     has {material_count} materials"
                )
            }
        }
    }
}

impl Error for RenderSceneError {}

/// Backend-neutral scene payload used to initialize renderer
/// resources.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderScene {
    meshes: Vec<MeshData>,
    materials: Vec<MaterialData>,
    objects: Vec<RenderObject>,
}

impl RenderScene {
    /// Creates an empty scene with the default camera.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Creates a validated scene from meshes, materials, and render
    /// objects.
    pub fn new(
        meshes: impl Into<Vec<MeshData>>,
        materials: impl Into<Vec<MaterialData>>,
        objects: impl Into<Vec<RenderObject>>,
    ) -> Result<Self, RenderSceneError> {
        let meshes = meshes.into();
        let materials = materials.into();
        let objects = objects.into();

        validate_objects(&objects, meshes.len(), materials.len())?;

        Ok(Self { meshes, materials, objects })
    }

    /// Returns scene mesh payloads.
    pub fn meshes(&self) -> &[MeshData] {
        &self.meshes
    }

    /// Returns scene material payloads.
    pub fn materials(&self) -> &[MaterialData] {
        &self.materials
    }

    /// Returns submitted render objects.
    pub fn objects(&self) -> &[RenderObject] {
        &self.objects
    }
}

fn validate_objects(
    objects: &[RenderObject],
    mesh_count: usize,
    material_count: usize,
) -> Result<(), RenderSceneError> {
    for (object_index, object) in objects.iter().enumerate() {
        if object.mesh_index >= mesh_count {
            return Err(RenderSceneError::MeshIndexUnavailable {
                object_index,
                mesh_index: object.mesh_index,
                mesh_count,
            });
        }

        if object.material_index >= material_count {
            return Err(RenderSceneError::MaterialIndexUnavailable {
                object_index,
                material_index: object.material_index,
                material_count,
            });
        }
    }

    Ok(())
}

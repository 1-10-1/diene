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
    camera: RenderCamera,
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

        Ok(Self { camera: RenderCamera::default(), meshes, materials, objects })
    }

    /// Sets the scene camera.
    #[must_use]
    pub const fn with_camera(mut self, camera: RenderCamera) -> Self {
        self.camera = camera;
        self
    }

    /// Returns the scene camera.
    pub const fn camera(&self) -> RenderCamera {
        self.camera
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

#[cfg(test)]
mod tests {
    use super::{RenderCamera, RenderObject, RenderScene, RenderSceneError, RenderTransform};
    use crate::{MaterialData, MeshData, glm};

    #[test]
    fn scene_accepts_valid_object_indices() {
        let scene = RenderScene::new(
            [MeshData::quad(glm::Vec3::zeros(), [1.0; 2], glm::Vec4::repeat(1.0))],
            [MaterialData::default()],
            [RenderObject::new(0, 0, RenderTransform::identity())],
        );

        assert_eq!(
            scene.as_ref().map(|scene| (
                scene.meshes().len(),
                scene.materials().len(),
                scene.objects().len(),
            )),
            Ok((1, 1, 1))
        );
    }

    #[test]
    fn scene_rejects_invalid_mesh_index() {
        assert_eq!(
            RenderScene::new(
                [],
                [MaterialData::default()],
                [RenderObject::new(0, 0, RenderTransform::identity())],
            ),
            Err(RenderSceneError::MeshIndexUnavailable {
                object_index: 0,
                mesh_index: 0,
                mesh_count: 0,
            })
        );
    }

    #[test]
    fn scene_rejects_invalid_material_index() {
        assert_eq!(
            RenderScene::new(
                [MeshData::quad(glm::Vec3::zeros(), [1.0; 2], glm::Vec4::repeat(1.0))],
                [],
                [RenderObject::new(0, 0, RenderTransform::identity())],
            ),
            Err(RenderSceneError::MaterialIndexUnavailable {
                object_index: 0,
                material_index: 0,
                material_count: 0,
            })
        );
    }

    #[test]
    fn render_transform_exposes_glm_model_matrix() {
        assert_eq!(
            RenderTransform::from_translation_scale(
                glm::Vec3::new(1.0, 2.0, 3.0),
                glm::Vec3::new(4.0, 5.0, 6.0),
            )
            .model_matrix(),
            glm::Mat4::new(
                4.0, 0.0, 0.0, 1.0, 0.0, 5.0, 0.0, 2.0, 0.0, 0.0, 6.0, 3.0, 0.0, 0.0, 0.0, 1.0,
            ),
        );
    }

    #[test]
    fn render_camera_exposes_glm_vectors() {
        let eye = glm::Vec3::new(1.0, 2.0, 3.0);
        let target = glm::Vec3::new(0.0, 1.0, 0.0);
        let up = glm::Vec3::y();
        let camera = RenderCamera::new(eye, target, up, 1.0, 0.1, 100.0);

        assert_eq!(camera.eye(), eye);
        assert_eq!(camera.target(), target);
        assert_eq!(camera.up(), up);
    }
}

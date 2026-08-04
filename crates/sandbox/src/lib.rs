#![allow(missing_docs)]
#![forbid(unsafe_code)]

use common::logging::macros::*;
use engine_renderer_api::{
    MaterialData, MeshData, MeshVertex, RenderCamera, RenderObject, RenderScene, RenderTransform,
    TextureData, glm,
};
use engine_runtime::Application;

pub fn run() -> anyhow::Result<()> {
    let _logger_guard = common::logging::init()?;

    let app_name = "diene sandbox";

    let app = Application::builder().with_name(app_name).with_scene(demo_scene()?).build()?;

    app.run()?;

    info!("[{}] sandbox application exited", app_name);

    Ok(())
}

fn demo_scene() -> anyhow::Result<RenderScene> {
    let model = glm::identity::<f32, 4>();
    let model = glm::rotate_x(&model, 45.0_f32.to_radians());
    let model = glm::rotate_y(&model, 45.0_f32.to_radians());

    Ok(RenderScene::new(
        [MeshData::new(
            vec![
                // Front (+Z)
                MeshVertex::new(
                    glm::Vec4::new(-0.5, -0.5, 0.5, 1.0),
                    glm::Vec4::new(1.0, 0.0, 0.0, 1.0),
                    glm::Vec2::new(0.0, 1.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(0.5, -0.5, 0.5, 1.0),
                    glm::Vec4::new(1.0, 0.0, 0.0, 1.0),
                    glm::Vec2::new(1.0, 1.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(0.5, 0.5, 0.5, 1.0),
                    glm::Vec4::new(1.0, 0.0, 0.0, 1.0),
                    glm::Vec2::new(1.0, 0.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(-0.5, 0.5, 0.5, 1.0),
                    glm::Vec4::new(1.0, 0.0, 0.0, 1.0),
                    glm::Vec2::new(0.0, 0.0),
                ),
                // Back (-Z)
                MeshVertex::new(
                    glm::Vec4::new(0.5, -0.5, -0.5, 1.0),
                    glm::Vec4::new(0.0, 1.0, 0.0, 1.0),
                    glm::Vec2::new(0.0, 1.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(-0.5, -0.5, -0.5, 1.0),
                    glm::Vec4::new(0.0, 1.0, 0.0, 1.0),
                    glm::Vec2::new(1.0, 1.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(-0.5, 0.5, -0.5, 1.0),
                    glm::Vec4::new(0.0, 1.0, 0.0, 1.0),
                    glm::Vec2::new(1.0, 0.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(0.5, 0.5, -0.5, 1.0),
                    glm::Vec4::new(0.0, 1.0, 0.0, 1.0),
                    glm::Vec2::new(0.0, 0.0),
                ),
                // Left (-X)
                MeshVertex::new(
                    glm::Vec4::new(-0.5, -0.5, -0.5, 1.0),
                    glm::Vec4::new(0.0, 0.0, 1.0, 1.0),
                    glm::Vec2::new(0.0, 1.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(-0.5, -0.5, 0.5, 1.0),
                    glm::Vec4::new(0.0, 0.0, 1.0, 1.0),
                    glm::Vec2::new(1.0, 1.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(-0.5, 0.5, 0.5, 1.0),
                    glm::Vec4::new(0.0, 0.0, 1.0, 1.0),
                    glm::Vec2::new(1.0, 0.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(-0.5, 0.5, -0.5, 1.0),
                    glm::Vec4::new(0.0, 0.0, 1.0, 1.0),
                    glm::Vec2::new(0.0, 0.0),
                ),
                // Right (+X)
                MeshVertex::new(
                    glm::Vec4::new(0.5, -0.5, 0.5, 1.0),
                    glm::Vec4::new(1.0, 1.0, 0.0, 1.0),
                    glm::Vec2::new(0.0, 1.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(0.5, -0.5, -0.5, 1.0),
                    glm::Vec4::new(1.0, 1.0, 0.0, 1.0),
                    glm::Vec2::new(1.0, 1.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(0.5, 0.5, -0.5, 1.0),
                    glm::Vec4::new(1.0, 1.0, 0.0, 1.0),
                    glm::Vec2::new(1.0, 0.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(0.5, 0.5, 0.5, 1.0),
                    glm::Vec4::new(1.0, 1.0, 0.0, 1.0),
                    glm::Vec2::new(0.0, 0.0),
                ),
                // Top (+Y)
                MeshVertex::new(
                    glm::Vec4::new(-0.5, 0.5, 0.5, 1.0),
                    glm::Vec4::new(1.0, 0.0, 1.0, 1.0),
                    glm::Vec2::new(0.0, 1.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(0.5, 0.5, 0.5, 1.0),
                    glm::Vec4::new(1.0, 0.0, 1.0, 1.0),
                    glm::Vec2::new(1.0, 1.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(0.5, 0.5, -0.5, 1.0),
                    glm::Vec4::new(1.0, 0.0, 1.0, 1.0),
                    glm::Vec2::new(1.0, 0.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(-0.5, 0.5, -0.5, 1.0),
                    glm::Vec4::new(1.0, 0.0, 1.0, 1.0),
                    glm::Vec2::new(0.0, 0.0),
                ),
                // Bottom (-Y)
                MeshVertex::new(
                    glm::Vec4::new(-0.5, -0.5, -0.5, 1.0),
                    glm::Vec4::new(0.0, 1.0, 1.0, 1.0),
                    glm::Vec2::new(0.0, 1.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(0.5, -0.5, -0.5, 1.0),
                    glm::Vec4::new(0.0, 1.0, 1.0, 1.0),
                    glm::Vec2::new(1.0, 1.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(0.5, -0.5, 0.5, 1.0),
                    glm::Vec4::new(0.0, 1.0, 1.0, 1.0),
                    glm::Vec2::new(1.0, 0.0),
                ),
                MeshVertex::new(
                    glm::Vec4::new(-0.5, -0.5, 0.5, 1.0),
                    glm::Vec4::new(0.0, 1.0, 1.0, 1.0),
                    glm::Vec2::new(0.0, 0.0),
                ),
            ],
            vec![
                0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4, 8, 9, 10, 10, 11, 8, 12, 13, 14, 14, 15, 12,
                16, 17, 18, 18, 19, 16, 20, 21, 22, 22, 23, 20,
            ],
        )?],
        [MaterialData::tinted(glm::Vec4::repeat(1.0))
            .with_albedo_texture(TextureData::from_file("assets/textures/texture.jpg")?)
            .with_label("blue quad")],
        [RenderObject::new(0, 0, RenderTransform::new(model))],
    )
    .map(|scene| {
        scene.with_camera(RenderCamera::new(
            glm::Vec3::new(0.0, 0.0, 3.2),
            glm::Vec3::zeros(),
            glm::Vec3::y(),
            45.0_f32.to_radians(),
            0.1,
            100.0,
        ))
    })?)
}

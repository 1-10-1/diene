#![allow(missing_docs)]
#![forbid(unsafe_code)]

use common::logging::macros::*;
use engine_renderer_api::{MaterialData, MeshData, RenderObject, RenderScene, RenderTransform};
use engine_runtime::Application;

pub fn run() -> anyhow::Result<()> {
    let _logger_guard = common::logging::init()?;

    let app_name = "diene sandbox";

    let app = Application::builder().with_name(app_name).with_scene(demo_scene()?).build()?;

    app.run()?;

    info!("[{}] sandbox application exited", app_name);

    Ok(())
}

fn demo_scene() -> Result<RenderScene, engine_renderer_api::RenderSceneError> {
    RenderScene::new(
        [MeshData::quad([0.0, 0.0, 0.0], [1.0, 1.0], [1.0; 4])],
        [
            MaterialData::tinted([1.0, 0.55, 0.55, 1.0]).with_label("warm checker"),
            MaterialData::tinted([0.55, 1.0, 0.65, 1.0]).with_label("green checker"),
            MaterialData::tinted([0.55, 0.7, 1.0, 1.0]).with_label("blue checker"),
        ],
        [
            RenderObject::new(
                0,
                0,
                RenderTransform::from_translation_scale([-0.9, -0.1, 0.0], [0.75, 0.75, 1.0]),
            ),
            RenderObject::new(
                0,
                1,
                RenderTransform::from_translation_scale([0.0, 0.2, -0.35], [0.85, 0.85, 1.0]),
            ),
            RenderObject::new(
                0,
                2,
                RenderTransform::from_translation_scale([0.9, -0.1, -0.7], [0.75, 0.75, 1.0]),
            ),
        ],
    )
}

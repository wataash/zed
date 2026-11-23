//! Optional real rendering for deterministic UI tests. Normal tests do not
//! initialize a GPU or load system fonts.
use std::sync::Arc;

use anyhow::Result;
use gpui::{TestAppContext, VisualTestContext};

/// Call before creating settings, entities, or windows in the test.
pub fn initialize(cx: &mut TestAppContext) -> Result<()> {
    if std::env::var_os("ZED_TEST_CAPTURE_DIR").is_none() {
        return Ok(());
    }
    *cx = TestAppContext::build_with_rendering(
        cx.dispatcher.clone(),
        Arc::new(gpui_wgpu::CosmicTextSystem::new("DejaVu Sans")),
        Arc::new(assets::Assets),
        || Ok(Some(Box::new(gpui_wgpu::WgpuHeadlessRenderer::new()?))),
    );
    cx.update(|cx| assets::Assets.load_fonts(cx))
}

/// Saves the actual rendered test window, including glyphs and icons.
pub fn capture(cx: &mut VisualTestContext, name: &str) -> Result<()> {
    let Some(directory) = std::env::var_os("ZED_TEST_CAPTURE_DIR") else {
        return Ok(());
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory)?;
    cx.run_until_parked();
    cx.update(|window, _| window.render_to_image())?
        .save(directory.join(format!("{name}.png")))?;
    Ok(())
}

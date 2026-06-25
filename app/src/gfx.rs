pub mod camera;
pub mod globe;
pub mod sky;

/// Depth format shared by every pipeline that draws into eframe's egui pass.
/// Must match the bits requested via `NativeOptions`/`WebOptions::depth_buffer`
/// (32 -> Depth32Float), since eframe attaches a depth texture of this format.
pub const DEPTH_FORMAT: eframe::wgpu::TextureFormat = eframe::wgpu::TextureFormat::Depth32Float;

/// Standard depth-stencil state for the 3D scene pipelines. `write` is true for
/// opaque geometry (globes/planets) and false for the additive star field, which
/// tests against the scene but must not occlude.
pub fn depth_state(write: bool) -> eframe::wgpu::DepthStencilState {
    eframe::wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(write),
        depth_compare: Some(eframe::wgpu::CompareFunction::Less),
        stencil: eframe::wgpu::StencilState::default(),
        bias: eframe::wgpu::DepthBiasState::default(),
    }
}

/// Depth-stencil state for the background sky (stars, billboards, lines). It
/// never tests or writes depth (`Always`, no write): the sky is drawn first and
/// the opaque globes/planets paint over it, so it cannot occlude them. Testing
/// it against the scene caused flicker, because the huge surface-to-system near/
/// far range leaves almost no depth precision out at the star shell.
pub fn depth_state_background() -> eframe::wgpu::DepthStencilState {
    eframe::wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(false),
        depth_compare: Some(eframe::wgpu::CompareFunction::Always),
        stencil: eframe::wgpu::StencilState::default(),
        bias: eframe::wgpu::DepthBiasState::default(),
    }
}

pub use camera::{LookAroundCamera, OrbitCamera, UnifiedCamera};
pub use globe::GlobeRenderer;
pub use sky::SkyRenderer;

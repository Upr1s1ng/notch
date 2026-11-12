use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: (r as f32) / 255.0,
        g: (g as f32) / 255.0,
        b: (b as f32) / 255.0,
        a,
    }
}

pub const BG_TOP: D2D1_COLOR_F = rgba(10, 11, 15, 0.94);
pub const BG_BOTTOM: D2D1_COLOR_F = rgba(16, 18, 26, 0.92);
pub const BG_MINIMAL: D2D1_COLOR_F = rgba(6, 7, 10, 0.97);
pub const BORDER_SPECULAR: D2D1_COLOR_F = rgba(255, 255, 255, 0.16);
pub const SURFACE: D2D1_COLOR_F = rgba(255, 255, 255, 0.06);
pub const SURFACE_HOVER: D2D1_COLOR_F = rgba(255, 255, 255, 0.12);
pub const TAB_ACTIVE: D2D1_COLOR_F = rgba(116, 185, 255, 0.22);

pub const TEXT_PRIMARY: D2D1_COLOR_F = rgba(244, 246, 251, 0.98);
pub const TEXT_SECONDARY: D2D1_COLOR_F = rgba(176, 184, 200, 0.88);
pub const TEXT_MUTED: D2D1_COLOR_F = rgba(122, 132, 150, 0.78);

pub const SKY: D2D1_COLOR_F = rgba(96, 165, 250, 1.0);
pub const EMERALD: D2D1_COLOR_F = rgba(52, 211, 153, 1.0);
pub const AMBER: D2D1_COLOR_F = rgba(251, 191, 36, 1.0);
pub const ROSE: D2D1_COLOR_F = rgba(251, 113, 133, 1.0);

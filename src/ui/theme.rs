use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: (r as f32) / 255.0,
        g: (g as f32) / 255.0,
        b: (b as f32) / 255.0,
        a,
    }
}

pub const BG_TOP: D2D1_COLOR_F = rgba(0, 0, 0, 0.98);
pub const BG_BOTTOM: D2D1_COLOR_F = rgba(7, 8, 13, 0.96);
pub const BG_MINIMAL: D2D1_COLOR_F = rgba(0, 0, 0, 0.99);
pub const BORDER_SPECULAR: D2D1_COLOR_F = rgba(255, 255, 255, 0.11);
pub const BORDER_SUBTLE: D2D1_COLOR_F = rgba(255, 255, 255, 0.06);
pub const TRACK_BG: D2D1_COLOR_F = rgba(14, 16, 23, 0.95);
pub const CARD_BG: D2D1_COLOR_F = rgba(22, 25, 34, 0.92);
pub const SURFACE: D2D1_COLOR_F = rgba(255, 255, 255, 0.055);
pub const SURFACE_HOVER: D2D1_COLOR_F = rgba(255, 255, 255, 0.11);
pub const TAB_ACTIVE: D2D1_COLOR_F = rgba(255, 255, 255, 0.14);
pub const PILL_WHITE: D2D1_COLOR_F = rgba(255, 255, 255, 0.96);
pub const ICON_ON_WHITE: D2D1_COLOR_F = rgba(8, 10, 14, 1.0);

pub const TEXT_PRIMARY: D2D1_COLOR_F = rgba(247, 248, 252, 0.98);
pub const TEXT_SECONDARY: D2D1_COLOR_F = rgba(168, 176, 194, 0.88);
pub const TEXT_MUTED: D2D1_COLOR_F = rgba(112, 121, 140, 0.78);

pub const SKY: D2D1_COLOR_F = rgba(96, 165, 250, 1.0);
pub const EMERALD: D2D1_COLOR_F = rgba(52, 211, 153, 1.0);
pub const AMBER: D2D1_COLOR_F = rgba(251, 191, 36, 1.0);
pub const ROSE: D2D1_COLOR_F = rgba(251, 113, 133, 1.0);
pub const DISCORD_BLURPLE: D2D1_COLOR_F = rgba(88, 101, 242, 1.0);

use windows::core::HSTRING;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_BEZIER_SEGMENT, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED, D2D1_FIGURE_END_OPEN, D2D1_GRADIENT_STOP,
    D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_F, D2D_SIZE_U,
};
use windows_numerics::Vector2;

fn pt(x: f32, y: f32) -> Vector2 {
    Vector2 { X: x, Y: y }
}
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1LinearGradientBrush,
    ID2D1PathGeometry, ID2D1SolidColorBrush, D2D1_ARC_SEGMENT, D2D1_ARC_SIZE_LARGE,
    D2D1_ARC_SIZE_SMALL, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, D2D1_EXTEND_MODE_CLAMP,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_GAMMA_2_2, D2D1_HWND_RENDER_TARGET_PROPERTIES,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_ROUNDED_RECT, D2D1_SWEEP_DIRECTION_CLOCKWISE,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Imaging::{CLSID_WICImagingFactory, IWICImagingFactory};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};

pub struct D2dRenderer {
    pub factory: ID2D1Factory,
    pub dwrite: IDWriteFactory,
    pub _wic: Option<IWICImagingFactory>,
    pub target: ID2D1HwndRenderTarget,
    pub font_regular: IDWriteTextFormat,
    pub font_bold: IDWriteTextFormat,
    pub font_small: IDWriteTextFormat,
    pub font_mono: IDWriteTextFormat,
}

impl D2dRenderer {
    pub fn new(hwnd: HWND, width_px: u32, height_px: u32) -> windows::core::Result<Self> {
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let wic: Option<IWICImagingFactory> =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok();

            let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                ..Default::default()
            };
            let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd,
                pixelSize: D2D_SIZE_U {
                    width: width_px.max(1),
                    height: height_px.max(1),
                },
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };
            let target = factory.CreateHwndRenderTarget(&rt_props, &hwnd_props)?;

            let font_regular = Self::make_font(&dwrite, "Segoe UI", 13.0, false)?;
            let font_bold = Self::make_font(&dwrite, "Segoe UI", 13.5, true)?;
            let font_small = Self::make_font(&dwrite, "Segoe UI", 11.0, false)?;
            let font_mono = Self::make_font(&dwrite, "Consolas", 12.5, true)?;

            Ok(Self {
                factory,
                dwrite,
                _wic: wic,
                target,
                font_regular,
                font_bold,
                font_small,
                font_mono,
            })
        }
    }

    unsafe fn make_font(
        dwrite: &IDWriteFactory,
        family: &str,
        size: f32,
        bold: bool,
    ) -> windows::core::Result<IDWriteTextFormat> {
        let weight = if bold {
            DWRITE_FONT_WEIGHT_BOLD
        } else {
            DWRITE_FONT_WEIGHT_NORMAL
        };
        let fmt = dwrite.CreateTextFormat(
            &HSTRING::from(family),
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            &HSTRING::from("en-US"),
        )?;
        let _ = fmt.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
        Ok(fmt)
    }

    pub fn resize(&self, width_px: u32, height_px: u32) {
        unsafe {
            let _ = self.target.Resize(&D2D_SIZE_U {
                width: width_px.max(1),
                height: height_px.max(1),
            });
        }
    }

    pub fn begin_draw(&self) {
        unsafe {
            self.target.BeginDraw();
            self.target.Clear(Some(&D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            }));
        }
    }

    pub fn end_draw(&self) -> windows::core::Result<()> {
        unsafe { self.target.EndDraw(None, None) }
    }

    pub fn brush(&self, color: D2D1_COLOR_F) -> windows::core::Result<ID2D1SolidColorBrush> {
        unsafe { self.target.CreateSolidColorBrush(&color, None) }
    }

    pub fn linear_gradient(
        &self,
        y0: f32,
        y1: f32,
        top: D2D1_COLOR_F,
        bottom: D2D1_COLOR_F,
    ) -> windows::core::Result<ID2D1LinearGradientBrush> {
        unsafe {
            let stops = [
                D2D1_GRADIENT_STOP {
                    position: 0.0,
                    color: top,
                },
                D2D1_GRADIENT_STOP {
                    position: 1.0,
                    color: bottom,
                },
            ];
            let coll = self.target.CreateGradientStopCollection(
                &stops,
                D2D1_GAMMA_2_2,
                D2D1_EXTEND_MODE_CLAMP,
            )?;
            let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                startPoint: pt(0.0, y0),
                endPoint: pt(0.0, y1),
            };
            self.target.CreateLinearGradientBrush(&props, None, &coll)
        }
    }

    pub fn fill_round_rect(&self, x: f32, y: f32, w: f32, h: f32, r: f32, color: D2D1_COLOR_F) {
        if let Ok(b) = self.brush(color) {
            let rr = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: x,
                    top: y,
                    right: x + w,
                    bottom: y + h,
                },
                radiusX: r,
                radiusY: r,
            };
            unsafe {
                self.target.FillRoundedRectangle(&rr, &b);
            }
        }
    }

    pub fn draw_round_rect(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        color: D2D1_COLOR_F,
        stroke: f32,
    ) {
        if let Ok(b) = self.brush(color) {
            let rr = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: x,
                    top: y,
                    right: x + w,
                    bottom: y + h,
                },
                radiusX: r,
                radiusY: r,
            };
            unsafe {
                self.target.DrawRoundedRectangle(&rr, &b, stroke, None);
            }
        }
    }

    /// Draws a top-edge attached notch with inverted-ear top corners and smooth rounded bottom corners.
    pub fn draw_top_edge_notch(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        bottom_r: f32,
        ear_r: f32,
        top_col: D2D1_COLOR_F,
        bot_col: D2D1_COLOR_F,
        border_col: D2D1_COLOR_F,
    ) {
        let Ok(path): windows::core::Result<ID2D1PathGeometry> =
            (unsafe { self.factory.CreatePathGeometry() })
        else {
            return;
        };
        let Ok(sink) = (unsafe { path.Open() }) else {
            return;
        };
        let ear = ear_r.min(h * 0.45).max(4.0);
        let br = bottom_r.min(w * 0.45).min(h * 0.85).max(4.0);
        unsafe {
            sink.BeginFigure(pt(x - ear, y), D2D1_FIGURE_BEGIN_FILLED);
            // Left inverted ear curving down into left wall
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: pt(x - ear * 0.2, y),
                point2: pt(x, y + ear * 0.25),
                point3: pt(x, y + ear),
            });
            // Left wall down to bottom-left corner
            sink.AddLine(pt(x, y + h - br));
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: pt(x, y + h),
                point2: pt(x + br * 0.35, y + h),
                point3: pt(x + br, y + h),
            });
            // Bottom edge
            sink.AddLine(pt(x + w - br, y + h));
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: pt(x + w - br * 0.35, y + h),
                point2: pt(x + w, y + h),
                point3: pt(x + w, y + h - br),
            });
            // Right wall up to right inverted ear
            sink.AddLine(pt(x + w, y + ear));
            sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                point1: pt(x + w, y + ear * 0.25),
                point2: pt(x + w + ear * 0.2, y),
                point3: pt(x + w + ear, y),
            });
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            let _ = sink.Close();

            if let Ok(grad) = self.linear_gradient(y, y + h, top_col, bot_col) {
                self.target.FillGeometry(&path, &grad, None);
            }
            if let Ok(b) = self.brush(border_col) {
                self.target.DrawGeometry(&path, &b, 1.0, None);
            }
        }
    }

    pub fn fill_circle(&self, cx: f32, cy: f32, r: f32, color: D2D1_COLOR_F) {
        if let Ok(b) = self.brush(color) {
            let ell = windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE {
                point: pt(cx, cy),
                radiusX: r,
                radiusY: r,
            };
            unsafe {
                self.target.FillEllipse(&ell, &b);
            }
        }
    }

    pub fn draw_circle(&self, cx: f32, cy: f32, r: f32, color: D2D1_COLOR_F, stroke: f32) {
        if let Ok(b) = self.brush(color) {
            let ell = windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE {
                point: pt(cx, cy),
                radiusX: r,
                radiusY: r,
            };
            unsafe {
                self.target.DrawEllipse(&ell, &b, stroke, None);
            }
        }
    }

    pub fn draw_line(&self, x1: f32, y1: f32, x2: f32, y2: f32, color: D2D1_COLOR_F, stroke: f32) {
        if let Ok(b) = self.brush(color) {
            unsafe {
                self.target
                    .DrawLine(pt(x1, y1), pt(x2, y2), &b, stroke, None);
            }
        }
    }

    pub fn draw_arc_ring(
        &self,
        cx: f32,
        cy: f32,
        r: f32,
        start_deg: f32,
        sweep_deg: f32,
        color: D2D1_COLOR_F,
        stroke: f32,
    ) {
        let sweep = sweep_deg.clamp(0.0, 360.0);
        if sweep < 0.5 || r <= 0.5 {
            return;
        }
        if sweep >= 359.5 {
            self.draw_circle(cx, cy, r, color, stroke);
            return;
        }
        let Ok(path): windows::core::Result<ID2D1PathGeometry> =
            (unsafe { self.factory.CreatePathGeometry() })
        else {
            return;
        };
        let Ok(sink) = (unsafe { path.Open() }) else {
            return;
        };
        let a0 = start_deg.to_radians();
        let a1 = (start_deg + sweep).to_radians();
        let p0 = pt(cx + r * a0.cos(), cy + r * a0.sin());
        let p1 = pt(cx + r * a1.cos(), cy + r * a1.sin());
        let arc_size = if sweep > 180.0 {
            D2D1_ARC_SIZE_LARGE
        } else {
            D2D1_ARC_SIZE_SMALL
        };
        unsafe {
            sink.BeginFigure(p0, D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: p1,
                size: D2D_SIZE_F {
                    width: r,
                    height: r,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: arc_size,
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            let _ = sink.Close();
            if let Ok(b) = self.brush(color) {
                self.target.DrawGeometry(&path, &b, stroke, None);
            }
        }
    }

    pub fn fill_triangle(
        &self,
        p1: (f32, f32),
        p2: (f32, f32),
        p3: (f32, f32),
        color: D2D1_COLOR_F,
    ) {
        let Ok(path): windows::core::Result<ID2D1PathGeometry> =
            (unsafe { self.factory.CreatePathGeometry() })
        else {
            return;
        };
        let Ok(sink) = (unsafe { path.Open() }) else {
            return;
        };
        unsafe {
            sink.BeginFigure(pt(p1.0, p1.1), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLine(pt(p2.0, p2.1));
            sink.AddLine(pt(p3.0, p3.1));
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            let _ = sink.Close();
            if let Ok(b) = self.brush(color) {
                self.target.FillGeometry(&path, &b, None);
            }
        }
    }

    pub fn draw_polyline(&self, pts: &[(f32, f32)], color: D2D1_COLOR_F, stroke: f32) {
        if pts.len() < 2 {
            return;
        }
        let Ok(path): windows::core::Result<ID2D1PathGeometry> =
            (unsafe { self.factory.CreatePathGeometry() })
        else {
            return;
        };
        let Ok(sink) = (unsafe { path.Open() }) else {
            return;
        };
        unsafe {
            sink.BeginFigure(pt(pts[0].0, pts[0].1), D2D1_FIGURE_BEGIN_HOLLOW);
            for &(x, y) in &pts[1..] {
                sink.AddLine(pt(x, y));
            }
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            let _ = sink.Close();
            if let Ok(b) = self.brush(color) {
                self.target.DrawGeometry(&path, &b, stroke, None);
            }
        }
    }

    /// align: 0 = left, 1 = center, 2 = right
    pub fn draw_text(
        &self,
        text: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: D2D1_COLOR_F,
        fmt: &IDWriteTextFormat,
        align: u8,
    ) {
        if text.is_empty() {
            return;
        }
        let Ok(b) = self.brush(color) else {
            return;
        };
        let wide: Vec<u16> = text.encode_utf16().collect();
        let rect = D2D_RECT_F {
            left: x,
            top: y,
            right: x + w.max(1.0),
            bottom: y + h.max(1.0),
        };
        unsafe {
            let _ = fmt.SetTextAlignment(match align {
                1 => DWRITE_TEXT_ALIGNMENT_CENTER,
                2 => DWRITE_TEXT_ALIGNMENT_TRAILING,
                _ => DWRITE_TEXT_ALIGNMENT_LEADING,
            });
            self.target.DrawText(
                &wide,
                fmt,
                &rect,
                &b,
                windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_CLIP,
                windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
}

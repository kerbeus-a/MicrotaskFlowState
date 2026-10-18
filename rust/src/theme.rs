//! Visual design: palette, typography and egui style.

use eframe::egui::{self, Color32, FontFamily, FontId, Rounding, Stroke, TextStyle};

pub const BG: Color32 = Color32::from_rgb(13, 15, 20);
pub const SURFACE: Color32 = Color32::from_rgb(22, 25, 33);
pub const SURFACE_HI: Color32 = Color32::from_rgb(30, 34, 44);
pub const SURFACE_HOVER: Color32 = Color32::from_rgb(36, 41, 53);
pub const BORDER: Color32 = Color32::from_rgb(41, 46, 59);
pub const TEXT: Color32 = Color32::from_rgb(232, 234, 240);
pub const MUTED: Color32 = Color32::from_rgb(136, 144, 163);
pub const FAINT: Color32 = Color32::from_rgb(88, 95, 112);
pub const ACCENT: Color32 = Color32::from_rgb(124, 92, 255);
pub const ACCENT_2: Color32 = Color32::from_rgb(34, 211, 238);
pub const DANGER: Color32 = Color32::from_rgb(244, 63, 94);
pub const SUCCESS: Color32 = Color32::from_rgb(52, 211, 153);


pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        mix(a.a(), b.a()),
    )
}

pub fn with_alpha(c: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (alpha.clamp(0.0, 1.0) * 255.0) as u8)
}

pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style.text_styles = [
        (TextStyle::Heading, FontId::new(19.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
    ]
    .into();

    let s = &mut style.spacing;
    s.item_spacing = egui::vec2(8.0, 8.0);
    s.button_padding = egui::vec2(12.0, 6.0);
    s.interact_size = egui::vec2(32.0, 32.0);
    s.slider_width = 180.0;
    s.combo_width = 200.0;
    s.scroll = egui::style::ScrollStyle::thin();

    let v = &mut style.visuals;
    *v = egui::Visuals::dark();
    v.override_text_color = Some(TEXT);
    v.panel_fill = BG;
    v.window_fill = SURFACE;
    v.extreme_bg_color = SURFACE_HI;
    v.faint_bg_color = SURFACE;
    v.code_bg_color = SURFACE_HI;
    v.window_rounding = Rounding::same(16.0);
    v.menu_rounding = Rounding::same(10.0);
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 8.0),
        blur: 24.0,
        spread: 0.0,
        color: Color32::from_black_alpha(120),
    };
    v.popup_shadow = v.window_shadow;
    v.selection.bg_fill = with_alpha(ACCENT, 0.45);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT_2;
    v.text_cursor.stroke = Stroke::new(2.0, ACCENT);

    let r = Rounding::same(10.0);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = SURFACE;
    w.noninteractive.weak_bg_fill = SURFACE;
    w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    w.noninteractive.rounding = r;

    w.inactive.bg_fill = SURFACE_HI;
    w.inactive.weak_bg_fill = SURFACE_HI;
    w.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.inactive.rounding = r;

    w.hovered.bg_fill = SURFACE_HOVER;
    w.hovered.weak_bg_fill = SURFACE_HOVER;
    w.hovered.bg_stroke = Stroke::new(1.0, with_alpha(ACCENT, 0.6));
    w.hovered.fg_stroke = Stroke::new(1.5, TEXT);
    w.hovered.rounding = r;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = with_alpha(ACCENT, 0.35);
    w.active.weak_bg_fill = with_alpha(ACCENT, 0.35);
    w.active.bg_stroke = Stroke::new(1.0, ACCENT);
    w.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    w.active.rounding = r;
    w.active.expansion = 0.0;

    w.open = w.active;

    ctx.set_style(style);
}

/// A rounded "card" frame used for grouped content.
pub fn card() -> egui::Frame {
    egui::Frame::none()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(Rounding::same(16.0))
        .inner_margin(egui::Margin::same(14.0))
}

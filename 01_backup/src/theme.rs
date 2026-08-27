// ============================================================================
// theme.rs - Giao dien toi mau hien dai, bo goc, ho tro ca CHE DO TOI va SANG
// ============================================================================
// Moi mau duoc cung cap qua 1 ham nhan tham so `dark: bool` thay vi hang so
// co dinh, de toan bo giao dien co the doi mau ngay lap tuc khi nguoi dung
// bam nut chuyen Sang/Toi (xem nut trong header, app.rs).

use egui::{Color32, Rounding, Stroke, Visuals};

pub fn accent(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0x4F, 0xC3, 0xF7)
    } else {
        Color32::from_rgb(0x02, 0x88, 0xD1)
    }
}

pub fn accent_strong(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0x03, 0xA9, 0xF4)
    } else {
        Color32::from_rgb(0x02, 0x77, 0xBD)
    }
}

pub fn success(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0x4C, 0xAF, 0x50)
    } else {
        Color32::from_rgb(0x2E, 0x7D, 0x32)
    }
}

pub fn warning(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0xFF, 0xB7, 0x4D)
    } else {
        Color32::from_rgb(0xE6, 0x51, 0x00)
    }
}

pub fn danger(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0xEF, 0x53, 0x50)
    } else {
        Color32::from_rgb(0xC6, 0x28, 0x28)
    }
}

pub fn muted(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0x9E, 0x9E, 0xA7)
    } else {
        Color32::from_rgb(0x5F, 0x63, 0x6B)
    }
}

pub fn bg_panel(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0x1E, 0x21, 0x28)
    } else {
        Color32::from_rgb(0xEE, 0xF0, 0xF2)
    }
}

pub fn bg_card(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0x26, 0x2A, 0x33)
    } else {
        Color32::from_rgb(0xFF, 0xFF, 0xFF)
    }
}

pub fn bg_window(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0x17, 0x19, 0x1F)
    } else {
        Color32::from_rgb(0xF8, 0xF9, 0xFA)
    }
}

pub fn text(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0xE8, 0xE9, 0xEC)
    } else {
        Color32::from_rgb(0x1A, 0x1D, 0x23)
    }
}

fn hover_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0x33, 0x38, 0x44)
    } else {
        Color32::from_rgb(0xE2, 0xE6, 0xEA)
    }
}

fn extreme_bg(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0x12, 0x14, 0x19)
    } else {
        Color32::from_rgb(0xFF, 0xFF, 0xFF)
    }
}

/// Ap dung bang mau (toi hoac sang) cho toan bo giao dien. Goi lai moi frame
/// (xem app.rs update()) de doi mau ngay khi nguoi dung bam nut chuyen theme.
pub fn apply(ctx: &egui::Context, dark: bool) {
    let mut visuals = if dark { Visuals::dark() } else { Visuals::light() };

    visuals.override_text_color = Some(text(dark));
    visuals.window_fill = bg_window(dark);
    visuals.panel_fill = bg_window(dark);
    visuals.extreme_bg_color = extreme_bg(dark);
    visuals.faint_bg_color = bg_card(dark);

    visuals.widgets.noninteractive.bg_fill = bg_panel(dark);
    visuals.widgets.noninteractive.rounding = Rounding::same(8.0);

    visuals.widgets.inactive.bg_fill = bg_card(dark);
    visuals.widgets.inactive.rounding = Rounding::same(6.0);

    visuals.widgets.hovered.bg_fill = hover_bg(dark);
    visuals.widgets.hovered.rounding = Rounding::same(6.0);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, accent(dark));

    visuals.widgets.active.bg_fill = accent_strong(dark);
    visuals.widgets.active.rounding = Rounding::same(6.0);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);

    visuals.selection.bg_fill = accent_strong(dark).linear_multiply(0.55);
    visuals.selection.stroke = Stroke::new(1.0, accent(dark));

    visuals.window_rounding = Rounding::same(10.0);
    visuals.menu_rounding = Rounding::same(8.0);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(12.0);
    ctx.set_style(style);
}

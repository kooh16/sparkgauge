//! Visual identity: palette, bundled fonts, egui style.
//!
//! Screens never write a colour literal; every colour has a single role.

use std::sync::Arc;

use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke, Visuals,
};

// Surfaces, darkest first.
pub const BG0: Color32 = Color32::from_rgb(0x07, 0x09, 0x0D);
pub const BG1: Color32 = Color32::from_rgb(0x0C, 0x10, 0x16);
pub const BG2: Color32 = Color32::from_rgb(0x12, 0x18, 0x21);
pub const BG3: Color32 = Color32::from_rgb(0x19, 0x21, 0x2C);
pub const LINE: Color32 = Color32::from_rgb(0x22, 0x2C, 0x39);
pub const LINE_SOFT: Color32 = Color32::from_rgb(0x17, 0x1F, 0x29);

pub const TEXT: Color32 = Color32::from_rgb(0xE6, 0xED, 0xF3);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x8E, 0x9C, 0xAE);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x55, 0x63, 0x75);

/// The GPU and its share of the unified memory.
pub const GPU: Color32 = Color32::from_rgb(0x9A, 0xDB, 0x4E);
/// The CPU and the memory used outside the GPU.
pub const CPU: Color32 = Color32::from_rgb(0x3C, 0xC8, 0xFF);
/// Efficiency cores, a quieter shade of the CPU colour.
pub const CPU_E: Color32 = Color32::from_rgb(0x2A, 0x8C, 0xB8);
/// Reclaimable page cache.
pub const CACHE: Color32 = Color32::from_rgb(0x4C, 0x5A, 0x6C);
/// What comes in: download, disk reads.
pub const IN: Color32 = Color32::from_rgb(0xA8, 0x9C, 0xFF);
/// What goes out: upload, disk writes.
pub const OUT: Color32 = Color32::from_rgb(0xFF, 0xB4, 0x3F);
pub const WARN: Color32 = Color32::from_rgb(0xFF, 0x86, 0x4A);
pub const DANGER: Color32 = Color32::from_rgb(0xFF, 0x5A, 0x6A);

pub const MEDIUM: &str = "barlow-medium";
pub const SEMIBOLD: &str = "barlow-semibold";
pub const COND: &str = "barlow-cond";
pub const COND_BOLD: &str = "barlow-cond-bold";
pub const MONO_MEDIUM: &str = "mono-medium";

pub fn body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}
/// Section titles and labels: condensed, upper case.
pub fn cond(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(COND.into()))
}
pub fn cond_bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(COND_BOLD.into()))
}
pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}
pub fn mono_medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MONO_MEDIUM.into()))
}

/// `c` at opacity `a` (0..1).
pub fn alpha(c: Color32, a: f32) -> Color32 {
    let a = (a.clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Linear blend of two opaque colours.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Colour of a temperature reading: normal, warm, hot.
pub fn temp_color(c: f32) -> Color32 {
    if c >= 90.0 {
        DANGER
    } else if c >= 80.0 {
        WARN
    } else {
        TEXT
    }
}

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    install_style(ctx);
}

fn install_fonts(ctx: &egui::Context) {
    macro_rules! font {
        ($file:literal) => {
            Arc::new(FontData::from_static(include_bytes!(concat!(
                "../assets/fonts/",
                $file
            ))))
        };
    }
    let mut defs = FontDefinitions::default();
    let faces = [
        ("barlow", font!("Barlow-Regular.ttf")),
        (MEDIUM, font!("Barlow-Medium.ttf")),
        (SEMIBOLD, font!("Barlow-SemiBold.ttf")),
        (COND, font!("BarlowCondensed-SemiBold.ttf")),
        (COND_BOLD, font!("BarlowCondensed-Bold.ttf")),
        ("mono", font!("JetBrainsMono-Regular.ttf")),
        (MONO_MEDIUM, font!("JetBrainsMono-Medium.ttf")),
    ];
    // egui's own fonts stay behind ours for the glyphs Barlow lacks.
    let fallback: Vec<String> = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (name, data) in faces {
        defs.font_data.insert(name.to_owned(), data);
    }
    let family = |first: &str| {
        let mut v = vec![first.to_owned()];
        v.extend(fallback.iter().cloned());
        v
    };
    defs.families
        .insert(FontFamily::Proportional, family("barlow"));
    defs.families.insert(FontFamily::Monospace, family("mono"));
    for name in [MEDIUM, SEMIBOLD, COND, COND_BOLD, MONO_MEDIUM] {
        defs.families
            .insert(FontFamily::Name(name.into()), family(name));
    }
    ctx.set_fonts(defs);
}

fn install_style(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        let mut v = Visuals::dark();
        v.panel_fill = BG1;
        v.window_fill = BG2;
        v.extreme_bg_color = BG0;
        v.faint_bg_color = BG2;
        v.override_text_color = Some(TEXT);
        v.selection.bg_fill = alpha(CPU, 0.28);
        v.selection.stroke = Stroke::new(1.0_f32, CPU);
        v.window_stroke = Stroke::new(1.0_f32, LINE);
        v.window_corner_radius = CornerRadius::same(10);
        v.popup_shadow = egui::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(140),
        };
        v.menu_corner_radius = CornerRadius::same(8);

        let r = CornerRadius::same(6);
        v.widgets.noninteractive.bg_fill = BG2;
        v.widgets.noninteractive.weak_bg_fill = BG2;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE_SOFT);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_DIM);
        v.widgets.noninteractive.corner_radius = r;
        v.widgets.inactive.bg_fill = BG3;
        v.widgets.inactive.weak_bg_fill = BG3;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, LINE);
        v.widgets.inactive.corner_radius = r;
        v.widgets.hovered.bg_fill = mix(BG3, TEXT, 0.06);
        v.widgets.hovered.weak_bg_fill = mix(BG3, TEXT, 0.06);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, mix(LINE, TEXT, 0.25));
        v.widgets.hovered.corner_radius = r;
        v.widgets.hovered.expansion = 0.0;
        v.widgets.active.corner_radius = r;
        v.widgets.active.expansion = 0.0;
        style.visuals = v;

        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.window_margin = Margin::same(12);
        style.spacing.scroll = egui::style::ScrollStyle::floating();
        style.spacing.scroll.bar_width = 6.0;
        style.spacing.scroll.floating_allocated_width = 0.0;
        style.interaction.selectable_labels = false;
        style.interaction.tooltip_delay = 0.25;

        use egui::TextStyle::*;
        style.text_styles = [
            (Small, body(12.0)),
            (Body, body(14.0)),
            (Monospace, mono(13.0)),
            (Button, medium(14.0)),
            (Heading, cond_bold(20.0)),
        ]
        .into();
    });
}

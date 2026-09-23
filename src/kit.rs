//! Hand-painted pieces shared by every panel: cards, titles, chips, stat
//! rows and time-series charts. Nothing here animates or flashes.

use egui::epaint::Mesh;
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};

use crate::theme::{self, alpha};

/// One line of text, letter-spaced, cut at `max_width`.
pub fn job(text: &str, font: FontId, color: Color32, spacing: f32, max_width: f32) -> LayoutJob {
    let mut j = LayoutJob::default();
    j.append(
        text,
        0.0,
        TextFormat {
            font_id: font,
            color,
            extra_letter_spacing: spacing,
            ..Default::default()
        },
    );
    j.wrap = TextWrapping::truncate_at_width(max_width);
    j
}

/// Paints text anchored at its left-centre; returns its rectangle.
pub fn text(
    p: &Painter,
    left_center: Pos2,
    s: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
) -> Rect {
    let g = p.layout_job(job(s, font, color, 0.0, max_width.max(8.0)));
    let r = Rect::from_min_size(
        pos2(left_center.x, left_center.y - g.size().y / 2.0),
        g.size(),
    );
    p.galley(r.min, g, color);
    r
}

/// Paints text anchored at its right-centre; returns its rectangle.
pub fn text_right(p: &Painter, right_center: Pos2, s: &str, font: FontId, color: Color32) -> Rect {
    p.text(right_center, Align2::RIGHT_CENTER, s, font, color)
}

/// A panel: raised surface, soft border, fixed padding.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(theme::BG2)
        .stroke(Stroke::new(1.0_f32, theme::LINE_SOFT))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Section title: coloured dot, spaced capitals, optional note on the right.
pub fn section(ui: &mut Ui, title: &str, accent: Color32, note: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(
        Rect::from_center_size(pos2(rect.left() + 4.0, rect.center().y), vec2(8.0, 8.0)),
        2,
        accent,
    );
    let g = p.layout_job(job(
        title,
        theme::cond_bold(14.0),
        theme::TEXT_DIM,
        2.2,
        rect.width(),
    ));
    p.galley(
        pos2(rect.left() + 16.0, rect.center().y - g.size().y / 2.0),
        g,
        theme::TEXT_DIM,
    );
    if !note.is_empty() {
        text_right(
            p,
            rect.right_center(),
            note,
            theme::cond(13.0),
            theme::TEXT_FAINT,
        );
    }
}

/// Chip: upper-case text on a tinted background. Returns its rectangle.
pub fn chip(p: &Painter, left_center: Pos2, s: &str, color: Color32) -> Rect {
    let g = p.layout_job(job(s, theme::cond_bold(12.5), color, 0.9, f32::INFINITY));
    let size = vec2(g.size().x + 14.0, 20.0);
    let r = Rect::from_min_size(pos2(left_center.x, left_center.y - size.y / 2.0), size);
    p.rect(
        r,
        4,
        alpha(color, 0.10),
        Stroke::new(1.0_f32, alpha(color, 0.32)),
        StrokeKind::Inside,
    );
    p.galley(
        pos2(r.left() + 7.0, r.center().y - g.size().y / 2.0),
        g,
        color,
    );
    r
}

pub fn chip_width(p: &Painter, s: &str) -> f32 {
    p.layout_job(job(
        s,
        theme::cond_bold(12.5),
        Color32::WHITE,
        0.9,
        f32::INFINITY,
    ))
    .size()
    .x + 14.0
}

/// One reading of a stat row.
pub struct Stat<'a> {
    pub label: &'a str,
    pub value: String,
    pub color: Color32,
}

impl<'a> Stat<'a> {
    pub fn new(label: &'a str, value: impl Into<String>, color: Color32) -> Self {
        Stat {
            label,
            value: value.into(),
            color,
        }
    }
}

/// A row of readings in equal columns, painted into `rect`: small label
/// above, value below.
pub fn stats_in(p: &Painter, rect: Rect, items: &[Stat]) {
    let w = rect.width() / items.len().max(1) as f32;
    for (i, s) in items.iter().enumerate() {
        let x = rect.left() + w * i as f32;
        let g = p.layout_job(job(
            s.label,
            theme::cond(11.5),
            theme::TEXT_FAINT,
            1.2,
            w - 6.0,
        ));
        let label_h = g.size().y;
        p.galley(
            pos2(x, rect.center().y - label_h - 1.0),
            g,
            theme::TEXT_FAINT,
        );
        text(
            p,
            pos2(x, rect.center().y + 9.0),
            &s.value,
            theme::mono_medium(14.0),
            s.color,
            w - 6.0,
        );
    }
}

/// Two-state or n-state switch; returns true when the value changed.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    width: f32,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 26.0), Sense::hover());
    let p = ui.painter().clone();
    p.rect(
        rect,
        6,
        theme::BG0,
        Stroke::new(1.0_f32, theme::LINE),
        StrokeKind::Inside,
    );
    let w = rect.width() / options.len() as f32;
    let mut changed = false;
    for (i, (v, label)) in options.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(rect.left() + w * i as f32, rect.top()),
            vec2(w, rect.height()),
        )
        .shrink(2.0);
        let resp = ui.interact(r, ui.id().with(("seg", i, *label)), Sense::click());
        let on = *value == *v;
        if on {
            p.rect(
                r,
                4,
                alpha(theme::TEXT, 0.08),
                Stroke::new(1.0_f32, alpha(theme::TEXT, 0.22)),
                StrokeKind::Inside,
            );
        } else if resp.hovered() {
            p.rect_filled(r, 4, theme::BG3);
        }
        let color = if on { theme::TEXT } else { theme::TEXT_DIM };
        p.text(
            r.center(),
            Align2::CENTER_CENTER,
            *label,
            theme::cond_bold(13.0),
            color,
        );
        if resp.clicked() && !on {
            *value = *v;
            changed = true;
        }
    }
    changed
}

/// One curve of a chart. Values are oldest first, newest last.
pub struct Series<'a> {
    pub label: &'a str,
    pub color: Color32,
    /// Upper edge of the curve.
    pub top: Vec<f32>,
    /// Lower edge when curves are stacked; the axis otherwise.
    pub base: Option<Vec<f32>>,
    /// What the hover read-out shows for this curve.
    pub shown: Vec<f32>,
    pub line: bool,
}

impl<'a> Series<'a> {
    pub fn line(label: &'a str, color: Color32, values: Vec<f32>) -> Self {
        Series {
            label,
            color,
            shown: values.clone(),
            top: values,
            base: None,
            line: true,
        }
    }
}

/// Time-series chart. The newest sample sits on the right edge and `slots`
/// samples fill the width, so a young history grows from the right like the
/// GNOME monitor. `y_max` is the value at the top edge.
pub fn chart(
    ui: &mut Ui,
    height: f32,
    slots: usize,
    y_max: f32,
    series: &[Series],
    fmt: &dyn Fn(f32) -> String,
) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let p = ui.painter_at(rect.expand(1.0));
    p.rect(
        rect,
        6,
        theme::BG0,
        Stroke::new(1.0_f32, theme::LINE_SOFT),
        StrokeKind::Inside,
    );
    let plot = rect.shrink2(vec2(1.0, 4.0));
    for k in 1..4 {
        let y = plot.bottom() - plot.height() * k as f32 / 4.0;
        p.hline(plot.x_range(), y, Stroke::new(1.0_f32, theme::LINE_SOFT));
    }
    text(
        &p,
        pos2(plot.left() + 8.0, plot.top() + 8.0),
        &fmt(y_max),
        theme::mono(11.0),
        theme::TEXT_FAINT,
        120.0,
    );
    let step = plot.width() / (slots.max(2) - 1) as f32;
    let x_of = |i: usize, n: usize| plot.right() - (n - 1 - i) as f32 * step;
    let y_of = |v: f32| plot.bottom() - plot.height() * (v / y_max.max(1e-9)).clamp(0.0, 1.0);

    for s in series {
        let n = s.top.len();
        if n < 2 {
            continue;
        }
        let mut mesh = Mesh::default();
        for i in 0..n - 1 {
            let (x0, x1) = (x_of(i, n), x_of(i + 1, n));
            let (t0, t1) = (y_of(s.top[i]), y_of(s.top[i + 1]));
            let (b0, b1, fill_top, fill_bottom) = match &s.base {
                Some(b) => (
                    y_of(b[i]),
                    y_of(b[i + 1]),
                    alpha(s.color, 0.34),
                    alpha(s.color, 0.34),
                ),
                None => (
                    plot.bottom(),
                    plot.bottom(),
                    alpha(s.color, 0.28),
                    alpha(s.color, 0.02),
                ),
            };
            let v = mesh.vertices.len() as u32;
            mesh.colored_vertex(pos2(x0, t0), fill_top);
            mesh.colored_vertex(pos2(x1, t1), fill_top);
            mesh.colored_vertex(pos2(x1, b1), fill_bottom);
            mesh.colored_vertex(pos2(x0, b0), fill_bottom);
            mesh.add_triangle(v, v + 1, v + 2);
            mesh.add_triangle(v, v + 2, v + 3);
        }
        p.add(mesh);
        if s.line {
            let pts: Vec<Pos2> = (0..n).map(|i| pos2(x_of(i, n), y_of(s.top[i]))).collect();
            p.add(egui::Shape::line(pts, Stroke::new(1.5_f32, s.color)));
        }
    }

    // Hover read-out: vertical guide and the values under the pointer.
    let Some(ptr) = resp.hover_pos() else { return };
    let n = series.iter().map(|s| s.top.len()).max().unwrap_or(0);
    if n == 0 {
        return;
    }
    let back = ((plot.right() - ptr.x) / step).round().max(0.0) as usize;
    if back >= n {
        return;
    }
    let i = n - 1 - back;
    let x = x_of(i, n);
    p.vline(
        x,
        plot.y_range(),
        Stroke::new(1.0_f32, alpha(theme::TEXT, 0.35)),
    );
    for s in series.iter().filter(|s| s.line && s.top.len() == n) {
        p.circle(
            pos2(x, y_of(s.top[i])),
            3.0,
            s.color,
            Stroke::new(1.5_f32, theme::BG0),
        );
    }
    let secs = back as f32 * crate::sampler::PERIOD.as_secs_f32();
    let lines: Vec<(String, String, Color32)> = std::iter::once((
        "".to_owned(),
        if secs < 0.5 {
            "now".to_owned()
        } else {
            format!("{secs:.0} s ago")
        },
        theme::TEXT_FAINT,
    ))
    .chain(
        series
            .iter()
            .filter(|s| s.shown.len() == n)
            .map(|s| (s.label.to_owned(), fmt(s.shown[i]), s.color)),
    )
    .collect();
    let lw = 150.0;
    let lh = 18.0;
    let size = vec2(lw, lines.len() as f32 * lh + 10.0);
    let mut at = pos2(x + 10.0, plot.top() + 4.0);
    if at.x + size.x > rect.right() {
        at.x = x - 10.0 - size.x;
    }
    let tip = Rect::from_min_size(at, size);
    p.rect(
        tip,
        6,
        alpha(theme::BG1, 0.96),
        Stroke::new(1.0_f32, theme::LINE),
        StrokeKind::Inside,
    );
    for (k, (label, value, color)) in lines.iter().enumerate() {
        let y = tip.top() + 5.0 + lh * (k as f32 + 0.5);
        if !label.is_empty() {
            p.rect_filled(
                Rect::from_center_size(pos2(tip.left() + 12.0, y), vec2(7.0, 7.0)),
                2,
                *color,
            );
            text(
                &p,
                pos2(tip.left() + 22.0, y),
                label,
                theme::body(13.0),
                theme::TEXT_DIM,
                80.0,
            );
            text_right(
                &p,
                pos2(tip.right() - 8.0, y),
                value,
                theme::mono(12.5),
                theme::TEXT,
            );
        } else {
            text(
                &p,
                pos2(tip.left() + 8.0, y),
                value,
                theme::cond(12.5),
                *color,
                lw,
            );
        }
    }
}

/// A "nice" upper bound for an auto-scaled chart: 1, 2 or 5 times a power
/// of ten, never below `floor`.
pub fn nice_max(v: f32, floor: f32) -> f32 {
    let v = v.max(floor);
    let mag = 10f32.powf(v.log10().floor());
    for m in [1.0, 2.0, 5.0, 10.0] {
        if v <= m * mag {
            return m * mag;
        }
    }
    10.0 * mag
}

/// Upper bound for a byte-rate chart, "nice" in binary units so the label
/// reads 2 MiB/s rather than 1.9 MiB/s.
pub fn nice_bytes_max(v: f32) -> f32 {
    let mut unit = 1.0_f32;
    while v / unit >= 1000.0 && unit < 1024f32.powi(4) {
        unit *= 1024.0;
    }
    nice_max(v / unit, 1.0) * unit
}

/// Bytes in binary units: "0 B", "512 KiB", "17.3 GiB".
pub fn bytes(b: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = b.max(0.0);
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{v:.0} B")
    } else if v >= 100.0 {
        format!("{v:.0} {}", UNITS[u])
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

pub fn rate(bps: f64) -> String {
    format!("{}/s", bytes(bps))
}

pub fn gib(b: u64) -> f32 {
    b as f32 / (1u64 << 30) as f32
}

pub fn pct(x: f32) -> String {
    format!("{:.0}%", x * 100.0)
}

/// Uptime: "12m", "5h 04m", "3d 07h".
pub fn uptime(s: u64) -> String {
    let (d, h, m) = (s / 86400, (s / 3600) % 24, (s / 60) % 60);
    if d > 0 {
        format!("{d}d {h:02}h")
    } else if h > 0 {
        format!("{h}h {m:02}m")
    } else {
        format!("{m}m")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_units() {
        assert_eq!(bytes(0.0), "0 B");
        assert_eq!(bytes(991.0), "991 B");
        assert_eq!(bytes(1536.0), "1.5 KiB");
        assert_eq!(bytes(17.3 * (1u64 << 30) as f64), "17.3 GiB");
        assert_eq!(bytes(300.0 * (1u64 << 20) as f64), "300 MiB");
    }

    #[test]
    fn nice_bounds() {
        assert_eq!(nice_max(0.0, 1024.0), 2000.0);
        assert_eq!(nice_max(3.2e6, 1.0), 5e6);
        assert_eq!(nice_max(7.0e6, 1.0), 1e7);
        assert_eq!(nice_max(1.0e6, 1.0), 1e6);
    }

    #[test]
    fn byte_rate_bounds_are_round_in_binary_units() {
        assert_eq!(bytes(nice_bytes_max(3.6 * 1048576.0) as f64), "5.0 MiB");
        assert_eq!(bytes(nice_bytes_max(0.0) as f64), "1 B");
        assert_eq!(bytes(nice_bytes_max(900.0 * 1048576.0) as f64), "1000 MiB");
    }

    #[test]
    fn uptime_format() {
        assert_eq!(uptime(59), "0m");
        assert_eq!(uptime(3 * 3600 + 4 * 60), "3h 04m");
        assert_eq!(uptime(2 * 86400 + 7 * 3600), "2d 07h");
    }
}

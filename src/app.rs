//! The window: header, unified memory, GPU, CPU, network and disks.
//!
//! The layout is dense on purpose: it should fit where the GNOME System
//! Monitor fits, about 1100 × 720.

use std::f32::consts::PI;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use egui::{Color32, Painter, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};

use crate::kit::{self, Series, Stat};
use crate::sampler::{self, HostInfo, Sample, Shared};
use crate::sys::CoreKind;
use crate::theme::{self, alpha, mix};

/// Space between cards.
const GAP: f32 = 8.0;
/// Height of the GPU and CPU charts.
const CHART_H: f32 = 72.0;
/// GPU processes listed before "+ n more".
const PROCS_SHOWN: usize = 4;
const PROC_ROW_H: f32 = 22.0;

#[derive(Clone, Copy, PartialEq)]
enum Span {
    OneMinute,
    FiveMinutes,
}

impl Span {
    fn slots(self) -> usize {
        match self {
            Span::OneMinute => 120,
            Span::FiveMinutes => sampler::CAPACITY,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Span::OneMinute => "LAST MINUTE",
            Span::FiveMinutes => "LAST 5 MINUTES",
        }
    }
}

/// Command-line options for README screenshots.
#[derive(Default)]
pub struct Launch {
    pub screenshot: Option<PathBuf>,
    pub after: f32,
    pub size: Option<[f32; 2]>,
}

pub struct App {
    shared: Arc<Mutex<Shared>>,
    span: Span,
    launch: Launch,
    started: Instant,
    shot_sent: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext, launch: Launch) -> Self {
        theme::install(&cc.egui_ctx);
        App {
            shared: sampler::spawn(cc.egui_ctx.clone()),
            span: Span::OneMinute,
            launch,
            started: Instant::now(),
            shot_sent: false,
        }
    }

    fn screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.launch.screenshot.clone() else {
            return;
        };
        if !self.shot_sent {
            if self.started.elapsed().as_secs_f32() >= self.launch.after {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                self.shot_sent = true;
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        }
        let shot = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| {
                if let egui::Event::Screenshot { image, .. } = e {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let Some(img) = shot {
            let [w, h] = img.size;
            if let Some(mut pm) = resvg::tiny_skia::Pixmap::new(w as u32, h as u32) {
                for (dst, src) in pm.pixels_mut().iter_mut().zip(img.pixels.iter()) {
                    *dst = resvg::tiny_skia::PremultipliedColorU8::from_rgba(
                        src.r(),
                        src.g(),
                        src.b(),
                        src.a(),
                    )
                    .unwrap_or(resvg::tiny_skia::PremultipliedColorU8::TRANSPARENT);
                }
                if let Err(e) = pm.save_png(&path) {
                    eprintln!("screenshot: {e}");
                }
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::BG0.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.screenshot(ctx);
        let shared = self.shared.clone();
        let s = shared.lock().unwrap_or_else(|e| e.into_inner());
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG1)
                    .inner_margin(egui::Margin::symmetric(12, 10)),
            )
            .show(ctx, |ui| {
                header(ui, &s, &mut self.span);
                ui.add_space(GAP);
                let Some(latest) = s.latest() else {
                    ui.centered_and_justified(|ui| {
                        ui.label(egui::RichText::new("Reading the machine…").color(theme::TEXT_DIM))
                    });
                    return;
                };
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = vec2(GAP, 0.0);
                        let slots = self.span.slots();
                        let hist: Vec<&Sample> = s.history.iter().rev().take(slots).rev().collect();
                        memory_card(ui, latest, &hist, slots, self.span);
                        ui.add_space(GAP);
                        ui.columns(2, |c| {
                            gpu_card(&mut c[0], &s.info, latest, &hist, slots);
                            cpu_card(&mut c[1], &s.info, latest, &hist, slots);
                        });
                        ui.add_space(GAP);
                        ui.columns(2, |c| {
                            io_card(
                                &mut c[0],
                                "NETWORK",
                                "physical interfaces",
                                ("DOWN", "UP"),
                                (latest.net_rx, latest.net_tx),
                                hist.iter().map(|x| (x.net_rx, x.net_tx)).collect(),
                                slots,
                            );
                            io_card(
                                &mut c[1],
                                "DISKS",
                                &s.info.disks.join(" · "),
                                ("READ", "WRITE"),
                                (latest.disk_read, latest.disk_write),
                                hist.iter().map(|x| (x.disk_read, x.disk_write)).collect(),
                                slots,
                            );
                        });
                    });
            });
    }
}

/// The mark: two concentric arcs, GPU outside, CPU inside.
fn logo(p: &Painter, c: Pos2, r: f32) {
    let arc = |radius: f32, to: f32, color: Color32, width: f32| {
        let n = 40;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let a = 0.75 * PI + 1.5 * PI * to * i as f32 / n as f32;
                pos2(c.x + radius * a.cos(), c.y + radius * a.sin())
            })
            .collect();
        p.add(egui::Shape::line(pts, Stroke::new(width, color)));
    };
    arc(r, 1.0, theme::BG3, r * 0.26);
    arc(r, 0.7, theme::GPU, r * 0.26);
    arc(r * 0.58, 1.0, theme::BG3, r * 0.22);
    arc(r * 0.58, 0.45, theme::CPU, r * 0.22);
}

fn header(ui: &mut Ui, s: &Shared, span: &mut Span) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
    let p = ui.painter().clone();
    logo(&p, pos2(rect.left() + 13.0, rect.center().y + 1.0), 11.5);
    let g = p.layout_job(kit::job(
        "SPARKGAUGE",
        theme::cond_bold(20.0),
        theme::TEXT,
        2.6,
        300.0,
    ));
    let wx = rect.left() + 34.0;
    p.galley(
        pos2(wx, rect.center().y - g.size().y / 2.0),
        g.clone(),
        theme::TEXT,
    );
    let where_ = match &s.info.machine {
        Some(m) => format!("{m} · {}", s.info.hostname),
        None => s.info.hostname.clone(),
    };

    // Right-hand side, laid out from the right edge.
    let seg_w = 130.0;
    let seg = Rect::from_min_size(
        pos2(rect.right() - seg_w, rect.center().y - 11.0),
        vec2(seg_w, 22.0),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(seg), |ui| {
        kit::segmented(
            ui,
            seg_w,
            span,
            &[(Span::OneMinute, "1 MIN"), (Span::FiveMinutes, "5 MIN")],
        );
    });
    let mut chips: Vec<(String, Color32)> = Vec::new();
    match &s.info.gpu {
        Ok(g) => {
            chips.push((g.name.clone(), theme::GPU));
            if !g.driver.is_empty() {
                chips.push((format!("DRIVER {}", g.driver), theme::TEXT_DIM));
            }
            if !g.cuda.is_empty() {
                chips.push((format!("CUDA {}", g.cuda), theme::TEXT_DIM));
            }
        }
        Err(_) => chips.push(("NO GPU".into(), theme::WARN)),
    }
    if let Some(u) = s.uptime {
        chips.push((format!("UP {}", kit::uptime(u)), theme::TEXT_DIM));
    }
    let mut x = seg.left() - 10.0;
    for (label, color) in chips.iter().rev() {
        let w = kit::chip_width(&p, label);
        x -= w;
        kit::chip(&p, pos2(x, rect.center().y), label, *color);
        x -= 5.0;
    }
    let tx = wx + g.size().x + 14.0;
    kit::text(
        &p,
        pos2(tx, rect.center().y + 1.0),
        &where_,
        theme::medium(13.0),
        theme::TEXT_DIM,
        x - tx - 10.0,
    );
}

/// Colour of the n-th GPU process in the memory bar.
fn proc_color(i: usize) -> Color32 {
    if i.is_multiple_of(2) {
        theme::GPU
    } else {
        mix(theme::GPU, theme::BG2, 0.3)
    }
}

fn memory_card(ui: &mut Ui, latest: &Sample, hist: &[&Sample], slots: usize, span: Span) {
    let m = latest.mem;
    kit::card(ui, |ui| {
        kit::section(
            ui,
            "UNIFIED MEMORY",
            theme::GPU,
            &format!(
                "{} SHARED BY CPU AND GPU · {}",
                kit::bytes(m.total as f64),
                span.label()
            ),
        );
        ui.add_space(4.0);

        // Legend: swatch, name, size and share on one line per part.
        let (rect, legend_resp) =
            ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
        let p = ui.painter();
        let parts = [
            (
                "GPU",
                m.gpu,
                theme::GPU,
                "Memory allocated by GPU processes, as reported by NVML. The GB10 has no VRAM of its own.",
            ),
            (
                "CPU",
                m.cpu,
                theme::CPU,
                "Everything else in use: programs, kernel, CPU side of GPU processes",
            ),
            (
                "CACHE",
                m.cache,
                theme::CACHE,
                "Page cache and buffers the kernel gives back on demand",
            ),
            ("FREE", m.free, theme::TEXT_FAINT, "Not used at all"),
        ];
        let w = rect.width() / 4.0;
        let cy = rect.center().y;
        for (i, (label, v, color, _)) in parts.iter().enumerate() {
            let x = rect.left() + w * i as f32;
            p.rect_filled(
                Rect::from_center_size(pos2(x + 4.0, cy), vec2(8.0, 8.0)),
                2,
                *color,
            );
            let g = p.layout_job(kit::job(
                label,
                theme::cond_bold(12.5),
                theme::TEXT_DIM,
                1.4,
                w,
            ));
            let lw = g.size().x;
            p.galley(pos2(x + 14.0, cy - g.size().y / 2.0), g, theme::TEXT_DIM);
            let r = kit::text(
                p,
                pos2(x + 14.0 + lw + 8.0, cy),
                &kit::bytes(*v as f64),
                theme::mono_medium(15.0),
                theme::TEXT,
                w,
            );
            let share = *v as f32 / m.total.max(1) as f32;
            kit::text(
                p,
                pos2(r.right() + 6.0, cy + 1.0),
                &kit::pct(share),
                theme::mono(11.5),
                theme::TEXT_FAINT,
                50.0,
            );
        }
        if let Some(ptr) = legend_resp.hover_pos() {
            let i = (((ptr.x - rect.left()) / w) as usize).min(3);
            legend_resp.on_hover_text_at_pointer(parts[i].3);
        }
        ui.add_space(4.0);

        // The bar: GPU split per process, then CPU, cache and free.
        let (bar, bar_resp) =
            ui.allocate_exact_size(vec2(ui.available_width(), 16.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(bar, 5, theme::BG0);
        let total = m.total.max(1) as f32;
        let mut segs: Vec<(f32, Color32, String)> = Vec::new();
        if let Some(g) = &latest.gpu {
            let mut left = m.gpu;
            for (i, pr) in g.procs.iter().enumerate() {
                let v = pr.mem.min(left);
                left -= v;
                segs.push((
                    v as f32,
                    proc_color(i),
                    format!(
                        "GPU · {} (pid {}) · {}",
                        pr.name,
                        pr.pid,
                        kit::bytes(v as f64)
                    ),
                ));
            }
        }
        segs.push((
            m.cpu as f32,
            theme::CPU,
            format!("CPU · {}", kit::bytes(m.cpu as f64)),
        ));
        segs.push((
            m.cache as f32,
            theme::CACHE,
            format!("Cache · {}", kit::bytes(m.cache as f64)),
        ));
        let mut x = bar.left();
        let mut hovered: Option<String> = None;
        let clip = p.with_clip_rect(bar);
        for (v, color, label) in &segs {
            let w = bar.width() * v / total;
            if w <= 0.0 {
                continue;
            }
            let r = Rect::from_min_max(pos2(x, bar.top()), pos2(x + w, bar.bottom()));
            clip.rect_filled(r, 0, *color);
            if w > 3.0 {
                clip.vline(r.right(), bar.y_range(), Stroke::new(1.5_f32, theme::BG2));
            }
            if bar_resp.hover_pos().is_some_and(|q| r.contains(q)) {
                hovered = Some(label.clone());
            }
            x += w;
        }
        // Rounded corners painted over the square segments.
        p.rect_stroke(
            bar.expand(1.0),
            6,
            Stroke::new(2.0_f32, theme::BG2),
            egui::StrokeKind::Inside,
        );
        let hovered = hovered.or_else(|| {
            bar_resp
                .hover_pos()
                .map(|_| format!("Free · {}", kit::bytes(m.free as f64)))
        });
        if let Some(h) = hovered {
            bar_resp.on_hover_text_at_pointer(h);
        }
        ui.add_space(6.0);

        // History, stacked in the same order as the bar.
        let gpu: Vec<f32> = hist.iter().map(|s| kit::gib(s.mem.gpu)).collect();
        let cpu: Vec<f32> = hist.iter().map(|s| kit::gib(s.mem.cpu)).collect();
        let cache: Vec<f32> = hist.iter().map(|s| kit::gib(s.mem.cache)).collect();
        let used: Vec<f32> = gpu.iter().zip(&cpu).map(|(a, b)| a + b).collect();
        let with_cache: Vec<f32> = used.iter().zip(&cache).map(|(a, b)| a + b).collect();
        let series = [
            Series {
                label: "Cache",
                color: theme::CACHE,
                top: with_cache,
                base: Some(used.clone()),
                shown: cache,
                line: false,
            },
            Series {
                label: "CPU",
                color: theme::CPU,
                top: used,
                base: Some(gpu.clone()),
                shown: cpu,
                line: true,
            },
            Series {
                label: "GPU",
                color: theme::GPU,
                top: gpu.clone(),
                base: None,
                shown: gpu,
                line: true,
            },
        ];
        kit::chart(ui, 64.0, slots, kit::gib(m.total), &series, &|v| {
            kit::bytes(v as f64 * (1u64 << 30) as f64)
        });
    });
}

/// The headline reading on the left, the other readings beside it.
fn headline(ui: &mut Ui, value: &str, color: Color32, stats: &[Stat]) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
    let p = ui.painter();
    let big_w = 86.0;
    kit::text(
        p,
        pos2(rect.left(), rect.center().y),
        value,
        theme::mono_medium(28.0),
        color,
        big_w,
    );
    let rest = Rect::from_min_max(pos2(rect.left() + big_w + 8.0, rect.top()), rect.max);
    kit::stats_in(p, rest, stats);
}

fn dash() -> String {
    "—".into()
}

fn gpu_card(ui: &mut Ui, info: &HostInfo, latest: &Sample, hist: &[&Sample], slots: usize) {
    kit::card(ui, |ui| {
        let g = match (&info.gpu, &latest.gpu) {
            (Ok(_), Some(g)) => g,
            (Err(e), _) => {
                kit::section(ui, "GPU", theme::GPU, "");
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("NVML could not be loaded, so the GPU cannot be read.")
                        .color(theme::TEXT_DIM),
                );
                ui.label(
                    egui::RichText::new(e)
                        .font(theme::mono(12.0))
                        .color(theme::TEXT_FAINT),
                );
                return;
            }
            _ => return,
        };
        kit::section(ui, "GPU", theme::GPU, "UTILIZATION");
        ui.add_space(2.0);
        headline(
            ui,
            &g.util.map(kit::pct).unwrap_or_else(dash),
            theme::GPU,
            &[
                Stat::new(
                    "TEMP",
                    g.temp_c.map(|t| format!("{t} °C")).unwrap_or_else(dash),
                    g.temp_c
                        .map(|t| theme::temp_color(t as f32))
                        .unwrap_or(theme::TEXT),
                ),
                Stat::new(
                    "POWER",
                    g.power_w.map(|w| format!("{w:.1} W")).unwrap_or_else(dash),
                    theme::TEXT,
                ),
                Stat::new(
                    "CLOCK",
                    g.clock_mhz.map(|c| format!("{c} MHz")).unwrap_or_else(dash),
                    theme::TEXT,
                ),
                Stat::new(
                    "STATE",
                    g.pstate.map(|s| format!("P{s}")).unwrap_or_else(dash),
                    theme::TEXT,
                ),
            ],
        );
        ui.add_space(4.0);
        let util: Vec<f32> = hist
            .iter()
            .map(|s| s.gpu.as_ref().and_then(|g| g.util).unwrap_or(0.0))
            .collect();
        kit::chart(
            ui,
            CHART_H,
            slots,
            1.0,
            &[Series::line("GPU", theme::GPU, util)],
            &kit::pct,
        );
        ui.add_space(6.0);

        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 14.0), Sense::hover());
        let p = ui.painter();
        let title = p.layout_job(kit::job(
            "PROCESSES",
            theme::cond(11.5),
            theme::TEXT_FAINT,
            1.2,
            200.0,
        ));
        p.galley(
            pos2(rect.left(), rect.center().y - title.size().y / 2.0),
            title,
            theme::TEXT_FAINT,
        );
        kit::text_right(
            p,
            rect.right_center(),
            "GPU MEMORY",
            theme::cond(11.5),
            theme::TEXT_FAINT,
        );
        // Fixed height so both cards of the row line up whatever the count.
        let (list, _) = ui.allocate_exact_size(
            vec2(ui.available_width(), PROC_ROW_H * PROCS_SHOWN as f32 + 16.0),
            Sense::hover(),
        );
        if g.procs.is_empty() {
            kit::text(
                ui.painter(),
                pos2(list.left(), list.top() + PROC_ROW_H / 2.0),
                "No process is using the GPU.",
                theme::body(13.0),
                theme::TEXT_FAINT,
                list.width(),
            );
        }
        let max = g.procs.first().map(|p| p.mem).unwrap_or(1).max(1);
        for (i, pr) in g.procs.iter().take(PROCS_SHOWN).enumerate() {
            let row = Rect::from_min_size(
                pos2(list.left(), list.top() + PROC_ROW_H * i as f32),
                vec2(list.width(), PROC_ROW_H),
            );
            let resp = ui.interact(row, ui.id().with(("proc", pr.pid)), Sense::hover());
            let p = ui.painter();
            if resp.hovered() {
                p.rect_filled(row, 4, theme::BG3);
            }
            let cy = row.center().y;
            p.rect_filled(
                Rect::from_center_size(pos2(row.left() + 5.0, cy), vec2(3.0, 12.0)),
                1,
                proc_color(i),
            );
            let name = kit::text(
                p,
                pos2(row.left() + 14.0, cy),
                &pr.name,
                theme::semibold(13.5),
                theme::TEXT,
                160.0,
            );
            let pid = kit::text(
                p,
                pos2(name.right() + 7.0, cy + 1.0),
                &pr.pid.to_string(),
                theme::mono(11.5),
                theme::TEXT_FAINT,
                80.0,
            );
            let mem_w = 70.0;
            let bar_w = 56.0;
            let bar = Rect::from_min_size(
                pos2(row.right() - mem_w - bar_w - 6.0, cy - 2.5),
                vec2(bar_w, 5.0),
            );
            kit::text(
                p,
                pos2(pid.right() + 10.0, cy),
                &pr.cmdline,
                theme::body(12.5),
                theme::TEXT_FAINT,
                bar.left() - pid.right() - 20.0,
            );
            p.rect_filled(bar, 2, theme::BG0);
            p.rect_filled(
                Rect::from_min_size(bar.min, vec2(bar_w * pr.mem as f32 / max as f32, 5.0)),
                2,
                alpha(theme::GPU, 0.8),
            );
            kit::text_right(
                p,
                pos2(row.right() - 4.0, cy),
                &kit::bytes(pr.mem as f64),
                theme::mono(12.5),
                theme::TEXT,
            );
            if !pr.cmdline.is_empty() {
                resp.on_hover_text_at_pointer(&pr.cmdline);
            }
        }
        if g.procs.len() > PROCS_SHOWN {
            let rest: u64 = g.procs.iter().skip(PROCS_SHOWN).map(|p| p.mem).sum();
            kit::text(
                ui.painter(),
                pos2(list.left() + 14.0, list.bottom() - 8.0),
                &format!(
                    "+ {} more · {}",
                    g.procs.len() - PROCS_SHOWN,
                    kit::bytes(rest as f64)
                ),
                theme::body(12.5),
                theme::TEXT_FAINT,
                list.width(),
            );
        }
    });
}

fn cpu_card(ui: &mut Ui, info: &HostInfo, latest: &Sample, hist: &[&Sample], slots: usize) {
    kit::card(ui, |ui| {
        let n = latest.cores.len();
        kit::section(ui, "CPU", theme::CPU, &format!("AVERAGE OF {n} CORES"));
        ui.add_space(2.0);
        let freqs = sampler::cluster_freqs(info, latest);
        let ghz = |k: CoreKind| {
            freqs
                .get(&k)
                .map(|f| format!("{:.2} GHz", *f as f32 / 1000.0))
                .unwrap_or_else(dash)
        };
        let busiest = latest
            .cores
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, u)| format!("#{i} {}", kit::pct(*u)))
            .unwrap_or_default();
        let big_little = freqs.contains_key(&CoreKind::Performance);
        let mut row = vec![Stat::new(
            "SOC TEMP",
            latest
                .soc_temp
                .map(|t| format!("{t:.0} °C"))
                .unwrap_or_else(dash),
            latest
                .soc_temp
                .map(theme::temp_color)
                .unwrap_or(theme::TEXT),
        )];
        if big_little {
            row.push(Stat::new("PERF", ghz(CoreKind::Performance), theme::TEXT));
            row.push(Stat::new(
                "EFFICIENCY",
                ghz(CoreKind::Efficiency),
                theme::TEXT,
            ));
        } else {
            row.push(Stat::new("FREQUENCY", ghz(CoreKind::Other), theme::TEXT));
        }
        row.push(Stat::new("BUSIEST", busiest, theme::TEXT));
        headline(ui, &kit::pct(latest.cpu), theme::CPU, &row);
        ui.add_space(4.0);
        let cpu: Vec<f32> = hist.iter().map(|s| s.cpu).collect();
        kit::chart(
            ui,
            CHART_H,
            slots,
            1.0,
            &[Series::line("CPU", theme::CPU, cpu)],
            &kit::pct,
        );
        ui.add_space(6.0);

        // Per-core bars, one row per cluster on big.LITTLE chips. The block
        // is as tall as the GPU process list so the two cards line up.
        let block_h = 14.0 + PROC_ROW_H * PROCS_SHOWN as f32 + 16.0;
        let groups: Vec<(CoreKind, &str, &str)> = if big_little {
            let model = |k: CoreKind| {
                info.cores
                    .iter()
                    .find(|c| c.0 == k)
                    .map(|c| c.1)
                    .unwrap_or("")
            };
            vec![
                (CoreKind::Performance, "PERF", model(CoreKind::Performance)),
                (
                    CoreKind::Efficiency,
                    "EFFICIENCY",
                    model(CoreKind::Efficiency),
                ),
            ]
        } else {
            vec![(CoreKind::Other, "CORES", "")]
        };
        let row_h = (block_h - 4.0 * (groups.len() as f32 - 1.0)) / groups.len() as f32;
        for (k, (kind, title, model)) in groups.iter().enumerate() {
            if k > 0 {
                ui.add_space(4.0);
            }
            let ids: Vec<usize> = (0..n)
                .filter(|&i| info.cores.get(i).map(|c| c.0).unwrap_or(CoreKind::Other) == *kind)
                .collect();
            let color = if *kind == CoreKind::Efficiency {
                theme::CPU_E
            } else {
                theme::CPU
            };
            core_row(ui, row_h, title, model, &ids, latest, color);
        }
    });
}

/// One cluster: its name on the left, then a vertical bar per core.
fn core_row(
    ui: &mut Ui,
    height: f32,
    title: &str,
    model: &str,
    ids: &[usize],
    s: &Sample,
    color: Color32,
) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let p = ui.painter();
    let label_w = 74.0;
    let g = p.layout_job(kit::job(
        title,
        theme::cond_bold(12.5),
        theme::TEXT_DIM,
        1.2,
        label_w,
    ));
    p.galley(
        pos2(rect.left(), rect.center().y - g.size().y),
        g,
        theme::TEXT_DIM,
    );
    kit::text(
        p,
        pos2(rect.left(), rect.center().y + 8.0),
        model,
        theme::body(11.5),
        theme::TEXT_FAINT,
        label_w,
    );
    if ids.is_empty() {
        return;
    }
    let area = Rect::from_min_max(pos2(rect.left() + label_w, rect.top()), rect.max);
    let gap = 4.0;
    let w = ((area.width() - gap * (ids.len() as f32 - 1.0)) / ids.len() as f32).max(4.0);
    let mut tip = None;
    for (k, &cpu) in ids.iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(area.left() + k as f32 * (w + gap), area.top()),
            vec2(w, area.height()),
        );
        let u = s.cores.get(cpu).copied().unwrap_or(0.0);
        p.rect_filled(r, 3, theme::BG0);
        if u > 0.005 {
            let fill = Rect::from_min_max(
                pos2(r.left(), r.bottom() - (r.height() * u).max(2.0)),
                r.max,
            );
            p.rect_filled(
                fill,
                3,
                if u >= 0.9 {
                    mix(color, theme::TEXT, 0.35)
                } else {
                    color
                },
            );
        }
        // Core number at the top, over the empty part unless nearly full.
        let ink = if u > 0.8 {
            theme::BG0
        } else {
            theme::TEXT_FAINT
        };
        p.text(
            pos2(r.center().x, r.top() + 8.0),
            egui::Align2::CENTER_CENTER,
            cpu.to_string(),
            theme::mono(10.0),
            ink,
        );
        if resp
            .hover_pos()
            .is_some_and(|q| q.x >= r.left() && q.x <= r.right())
        {
            let f = s.freqs.get(cpu).copied().unwrap_or(0);
            tip = Some(format!("CPU {cpu} · {} · {f} MHz", kit::pct(u)));
        }
    }
    if let Some(t) = tip {
        resp.on_hover_text_at_pointer(t);
    }
}

fn io_card(
    ui: &mut Ui,
    title: &str,
    note: &str,
    labels: (&str, &str),
    now: (f64, f64),
    hist: Vec<(f64, f64)>,
    slots: usize,
) {
    kit::card(ui, |ui| {
        // Title and note on the left, the two live rates on the right.
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
        let p = ui.painter();
        let cy = rect.center().y;
        p.rect_filled(
            Rect::from_center_size(pos2(rect.left() + 4.0, cy), vec2(8.0, 8.0)),
            2,
            theme::IN,
        );
        let g = p.layout_job(kit::job(
            title,
            theme::cond_bold(14.0),
            theme::TEXT_DIM,
            2.2,
            200.0,
        ));
        let tw = g.size().x;
        p.galley(
            pos2(rect.left() + 16.0, cy - g.size().y / 2.0),
            g,
            theme::TEXT_DIM,
        );
        let mut x = rect.right();
        for (label, value, color) in [(labels.1, now.1, theme::OUT), (labels.0, now.0, theme::IN)] {
            let v = kit::text_right(
                p,
                pos2(x, cy),
                &kit::rate(value),
                theme::mono_medium(14.0),
                color,
            );
            let l = kit::text_right(
                p,
                pos2(v.left() - 6.0, cy + 1.0),
                label,
                theme::cond(12.0),
                theme::TEXT_FAINT,
            );
            x = l.left() - 16.0;
        }
        kit::text(
            p,
            pos2(rect.left() + 16.0 + tw + 10.0, cy + 1.0),
            &note.to_uppercase(),
            theme::cond(12.0),
            theme::TEXT_FAINT,
            x - (rect.left() + 26.0 + tw),
        );
        ui.add_space(6.0);
        let ins: Vec<f32> = hist.iter().map(|h| h.0 as f32).collect();
        let outs: Vec<f32> = hist.iter().map(|h| h.1 as f32).collect();
        let peak = ins.iter().chain(&outs).copied().fold(1024.0, f32::max);
        kit::chart(
            ui,
            56.0,
            slots,
            kit::nice_bytes_max(peak),
            &[
                Series::line(labels.0, theme::IN, ins),
                Series::line(labels.1, theme::OUT, outs),
            ],
            &|v| kit::rate(v as f64),
        );
    });
}

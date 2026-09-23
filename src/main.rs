//! SparkGauge: a desktop system monitor for NVIDIA GB10 machines that shows
//! the GPU next to the CPU and splits the unified memory between them.

mod app;
mod gpu;
mod kit;
mod sampler;
mod sys;
mod theme;

use app::{App, Launch};

const HELP: &str = "\
SparkGauge — desktop system monitor for NVIDIA GB10 machines

USAGE:
    sparkgauge [OPTIONS]

OPTIONS:
    --screenshot <FILE>   Save a PNG of the window, then quit
    --after <SECONDS>     Wait this long before the screenshot [default: 3]
    --size <WxH>          Window size, e.g. 1280x900
    -V, --version         Print the version
    -h, --help            Print this help
";

fn main() -> eframe::Result {
    let launch = parse_args();
    let size = launch.size.unwrap_or([1120.0, 640.0]);
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size(size)
        .with_min_inner_size([900.0, 600.0])
        .with_title("SparkGauge")
        .with_app_id("sparkgauge");
    let viewport = match icon() {
        Some(i) => viewport.with_icon(i),
        None => viewport,
    };
    // A screenshot run stays behind the other windows and never takes focus.
    let viewport = if launch.screenshot.is_some() {
        viewport
            .with_active(false)
            .with_window_level(egui::WindowLevel::AlwaysOnBottom)
    } else {
        viewport
    };
    let options = eframe::NativeOptions {
        viewport,
        // The sampler drives repaints; without vsync X11 does not stall an
        // unfocused window.
        vsync: false,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "sparkgauge",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc, launch)))),
    )
}

/// Window icon, rendered from the SVG mark.
fn icon() -> Option<egui::IconData> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_str(
        include_str!("../assets/sparkgauge.svg"),
        &usvg::Options::default(),
    )
    .ok()?;
    let mut pm = tiny_skia::Pixmap::new(256, 256)?;
    resvg::render(&tree, tiny_skia::Transform::identity(), &mut pm.as_mut());
    // tiny-skia works in premultiplied alpha; the icon wants straight RGBA.
    let rgba = pm.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    });
    Some(egui::IconData {
        rgba: rgba.collect(),
        width: 256,
        height: 256,
    })
}

fn parse_args() -> Launch {
    let mut l = Launch {
        after: 3.0,
        ..Default::default()
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--screenshot" => l.screenshot = args.next().map(Into::into),
            "--after" => l.after = args.next().and_then(|s| s.parse().ok()).unwrap_or(3.0),
            "--size" => {
                l.size = args.next().and_then(|s| {
                    let (w, h) = s.split_once('x')?;
                    Some([w.parse().ok()?, h.parse().ok()?])
                })
            }
            "-V" | "--version" => {
                println!("sparkgauge {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "-h" | "--help" => {
                print!("{HELP}");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown option: {other}\n\n{HELP}");
                std::process::exit(2);
            }
        }
    }
    l
}

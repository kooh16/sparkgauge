//! Background thread that reads the machine twice a second and keeps the
//! last five minutes. The window only repaints when a new sample lands.

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::gpu::{Gpu, GpuInfo, GpuSample};
use crate::sys::{self, CoreKind, CpuTimes, MemInfo};

pub const PERIOD: Duration = Duration::from_millis(500);
/// Five minutes of history.
pub const CAPACITY: usize = 600;

/// The unified memory split into four parts that add up to the total.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MemSplit {
    pub total: u64,
    /// Allocated by GPU processes.
    pub gpu: u64,
    /// Everything else in use: programs, kernel, CPU side of GPU processes.
    pub cpu: u64,
    /// Page cache and other memory the kernel hands back on demand.
    pub cache: u64,
    pub free: u64,
}

impl MemSplit {
    pub fn new(m: MemInfo, gpu_procs: u64) -> Self {
        let available = m.available.min(m.total);
        let free = m.free.min(available);
        let used = m.total - available;
        let gpu = gpu_procs.min(used);
        MemSplit {
            total: m.total,
            gpu,
            cpu: used - gpu,
            cache: available - free,
            free,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sample {
    pub mem: MemSplit,
    pub cpu: f32,
    /// Busy share per CPU number, 0..1.
    pub cores: Vec<f32>,
    /// Mean frequency per CPU number, MHz.
    pub freqs: Vec<u32>,
    pub soc_temp: Option<f32>,
    /// Bytes per second over the physical interfaces and disks.
    pub net_rx: f64,
    pub net_tx: f64,
    pub disk_read: f64,
    pub disk_write: f64,
    pub gpu: Option<GpuSample>,
}

/// What does not change while the program runs.
#[derive(Clone, Debug)]
pub struct HostInfo {
    pub hostname: String,
    pub machine: Option<String>,
    pub cores: Vec<(CoreKind, &'static str)>,
    /// NVML's error message when the GPU cannot be read.
    pub gpu: Result<GpuInfo, String>,
    pub disks: Vec<String>,
}

pub struct Shared {
    pub info: HostInfo,
    pub history: VecDeque<Sample>,
    pub uptime: Option<u64>,
}

impl Shared {
    pub fn latest(&self) -> Option<&Sample> {
        self.history.back()
    }
}

/// Counters that only make sense as a difference between two readings.
struct Counters {
    at: Instant,
    cpu: CpuTimes,
    cores: Vec<CpuTimes>,
    net: (u64, u64),
    disk: (u64, u64),
}

fn read_counters(disks: &[String]) -> Counters {
    let (cpu, cores) = sys::parse_stat(&fs::read_to_string("/proc/stat").unwrap_or_default());
    let net_dev = fs::read_to_string("/proc/net/dev").unwrap_or_default();
    let net = sys::parse_net_dev(&net_dev)
        .filter(|(n, _, _)| sys::is_physical_iface(n))
        .fold((0, 0), |a, (_, rx, tx)| (a.0 + rx, a.1 + tx));
    let diskstats = fs::read_to_string("/proc/diskstats").unwrap_or_default();
    let disk = sys::parse_diskstats(&diskstats)
        .filter(|(n, _, _)| disks.iter().any(|d| d == n))
        .fold((0, 0), |a, (_, r, w)| (a.0 + r, a.1 + w));
    Counters {
        at: Instant::now(),
        cpu,
        cores,
        net,
        disk,
    }
}

pub fn spawn(ctx: egui::Context) -> Arc<Mutex<Shared>> {
    let gpu = Gpu::open();
    let info = HostInfo {
        hostname: sys::hostname(),
        machine: sys::machine(),
        cores: sys::parse_cpuinfo(&fs::read_to_string("/proc/cpuinfo").unwrap_or_default()),
        gpu: gpu.as_ref().map(|(_, i)| i.clone()).map_err(Clone::clone),
        disks: sys::physical_disks(),
    };
    let gpu = gpu.ok().map(|(g, _)| g);
    let disks = info.disks.clone();
    let shared = Arc::new(Mutex::new(Shared {
        info,
        history: VecDeque::with_capacity(CAPACITY),
        uptime: sys::uptime_secs(),
    }));
    let out = shared.clone();
    thread::Builder::new()
        .name("sampler".into())
        .spawn(move || {
            let mut prev = read_counters(&disks);
            let mut next = Instant::now() + PERIOD;
            loop {
                thread::sleep(next.saturating_duration_since(Instant::now()));
                next += PERIOD;
                let now = read_counters(&disks);
                let dt = now.at.duration_since(prev.at).as_secs_f64().max(1e-3);
                let rate = |a: u64, b: u64| a.saturating_sub(b) as f64 / dt;
                let gpu = gpu.as_ref().map(Gpu::sample);
                let mem =
                    sys::parse_meminfo(&fs::read_to_string("/proc/meminfo").unwrap_or_default());
                let sample = Sample {
                    mem: MemSplit::new(mem, gpu.as_ref().map(GpuSample::used).unwrap_or(0)),
                    cpu: now.cpu.usage_since(&prev.cpu),
                    cores: now
                        .cores
                        .iter()
                        .zip(
                            prev.cores
                                .iter()
                                .chain(std::iter::repeat(&CpuTimes::default())),
                        )
                        .map(|(n, p)| n.usage_since(p))
                        .collect(),
                    freqs: (0..now.cores.len())
                        .map(|i| sys::cpu_freq_mhz(i).unwrap_or(0))
                        .collect(),
                    soc_temp: sys::soc_temp(),
                    net_rx: rate(now.net.0, prev.net.0),
                    net_tx: rate(now.net.1, prev.net.1),
                    disk_read: rate(now.disk.0, prev.disk.0),
                    disk_write: rate(now.disk.1, prev.disk.1),
                    gpu,
                };
                prev = now;
                {
                    let mut s = out.lock().unwrap_or_else(|e| e.into_inner());
                    if s.history.len() == CAPACITY {
                        s.history.pop_front();
                    }
                    s.history.push_back(sample);
                    s.uptime = sys::uptime_secs();
                }
                ctx.request_repaint();
            }
        })
        .expect("sampler thread");
    shared
}

/// Mean frequency of each core kind, in MHz.
pub fn cluster_freqs(info: &HostInfo, s: &Sample) -> HashMap<CoreKind, u32> {
    let mut acc: HashMap<CoreKind, (u64, u64)> = HashMap::new();
    for (i, f) in s.freqs.iter().enumerate() {
        if *f == 0 {
            continue;
        }
        let kind = info.cores.get(i).map(|c| c.0).unwrap_or(CoreKind::Other);
        let e = acc.entry(kind).or_default();
        e.0 += *f as u64;
        e.1 += 1;
    }
    acc.into_iter()
        .map(|(k, (sum, n))| (k, (sum / n) as u32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1 << 30;

    #[test]
    fn split_adds_up_to_total() {
        let m = MemInfo {
            total: 120 * GIB,
            free: 60 * GIB,
            available: 95 * GIB,
        };
        let s = MemSplit::new(m, 17 * GIB);
        assert_eq!(
            s,
            MemSplit {
                total: 120 * GIB,
                gpu: 17 * GIB,
                cpu: 8 * GIB,
                cache: 35 * GIB,
                free: 60 * GIB
            }
        );
        assert_eq!(s.gpu + s.cpu + s.cache + s.free, s.total);
    }

    #[test]
    fn gpu_share_never_exceeds_used_memory() {
        let m = MemInfo {
            total: 100,
            free: 50,
            available: 90,
        };
        let s = MemSplit::new(m, 40);
        assert_eq!((s.gpu, s.cpu), (10, 0));
        assert_eq!(s.gpu + s.cpu + s.cache + s.free, s.total);
    }

    #[test]
    fn inconsistent_meminfo_does_not_underflow() {
        let m = MemInfo {
            total: 100,
            free: 120,
            available: 110,
        };
        let s = MemSplit::new(m, 0);
        assert_eq!(s.gpu + s.cpu + s.cache + s.free, s.total);
    }
}

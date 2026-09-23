//! GPU readings through NVML, loaded at run time from the driver.
//!
//! On the GB10 NVML reports utilization, clocks, power and per-process
//! memory, but not the device memory totals: the GPU has no VRAM of its own
//! and `nvidia-smi` prints "Not Supported". The GPU share of the unified
//! memory is therefore the sum of what each process has allocated.

use std::collections::BTreeMap;
use std::ffi::OsStr;

use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::{Clock, PerformanceState, TemperatureSensor};
use nvml_wrapper::enums::device::UsedGpuMemory;

use crate::sys;

pub struct Gpu {
    nvml: Nvml,
}

/// What does not change while the program runs.
#[derive(Clone, Debug, Default)]
pub struct GpuInfo {
    pub name: String,
    pub driver: String,
    pub cuda: String,
}

#[derive(Clone, Debug, Default)]
pub struct GpuSample {
    /// Busy share of the last sampling period, 0..1.
    pub util: Option<f32>,
    pub temp_c: Option<u32>,
    pub power_w: Option<f32>,
    pub clock_mhz: Option<u32>,
    pub pstate: Option<u8>,
    /// Processes holding GPU memory, largest first.
    pub procs: Vec<GpuProc>,
}

impl GpuSample {
    /// Memory held by all GPU processes, in bytes.
    pub fn used(&self) -> u64 {
        self.procs.iter().map(|p| p.mem).sum()
    }
}

#[derive(Clone, Debug)]
pub struct GpuProc {
    pub pid: u32,
    pub name: String,
    pub cmdline: String,
    pub mem: u64,
}

impl Gpu {
    /// Loads NVML. Distributions do not always ship the unversioned
    /// `libnvidia-ml.so` link, so the `.so.1` is tried as well.
    pub fn open() -> Result<(Self, GpuInfo), String> {
        let nvml = Nvml::init()
            .or_else(|e| {
                Nvml::builder()
                    .lib_path(OsStr::new("libnvidia-ml.so.1"))
                    .init()
                    .map_err(|_| e)
            })
            .map_err(|e| e.to_string())?;
        let name = nvml
            .device_by_index(0)
            .and_then(|d| d.name())
            .map_err(|e| e.to_string())?;
        let driver = nvml.sys_driver_version().unwrap_or_default();
        let cuda = nvml
            .sys_cuda_driver_version()
            .map(|v| format!("{}.{}", v / 1000, (v % 1000) / 10))
            .unwrap_or_default();
        Ok((Gpu { nvml }, GpuInfo { name, driver, cuda }))
    }

    pub fn sample(&self) -> GpuSample {
        let Ok(d) = self.nvml.device_by_index(0) else {
            return GpuSample::default();
        };
        // A process can hold both a compute and a graphics context: keep the
        // larger figure rather than counting it twice.
        let mut by_pid: BTreeMap<u32, u64> = BTreeMap::new();
        let lists = [
            d.running_compute_processes(),
            d.running_graphics_processes(),
        ];
        for p in lists.into_iter().flatten().flatten() {
            let mem = match p.used_gpu_memory {
                UsedGpuMemory::Used(b) => b,
                UsedGpuMemory::Unavailable => 0,
            };
            let e = by_pid.entry(p.pid).or_default();
            *e = (*e).max(mem);
        }
        let mut procs: Vec<GpuProc> = by_pid
            .into_iter()
            .map(|(pid, mem)| {
                let (name, cmdline) = sys::process_name(pid);
                GpuProc {
                    pid,
                    name,
                    cmdline,
                    mem,
                }
            })
            .collect();
        procs.sort_by(|a, b| b.mem.cmp(&a.mem).then(a.pid.cmp(&b.pid)));
        GpuSample {
            util: d.utilization_rates().ok().map(|u| u.gpu as f32 / 100.0),
            temp_c: d.temperature(TemperatureSensor::Gpu).ok(),
            power_w: d.power_usage().ok().map(|mw| mw as f32 / 1000.0),
            clock_mhz: d.clock_info(Clock::Graphics).ok(),
            pstate: d.performance_state().ok().and_then(pstate_number),
            procs,
        }
    }
}

fn pstate_number(p: PerformanceState) -> Option<u8> {
    use PerformanceState::*;
    Some(match p {
        Zero => 0,
        One => 1,
        Two => 2,
        Three => 3,
        Four => 4,
        Five => 5,
        Six => 6,
        Seven => 7,
        Eight => 8,
        Nine => 9,
        Ten => 10,
        Eleven => 11,
        Twelve => 12,
        Thirteen => 13,
        Fourteen => 14,
        Fifteen => 15,
        Unknown => return None,
    })
}

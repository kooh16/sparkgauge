//! Everything the kernel tells us about the host: memory, CPU, disks,
//! network, temperatures. Parsers take the file contents as a string so they
//! can be tested against fixtures.

use std::fs;
use std::path::Path;

/// `/proc/meminfo`, in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MemInfo {
    pub total: u64,
    pub free: u64,
    pub available: u64,
}

pub fn parse_meminfo(s: &str) -> MemInfo {
    let mut m = MemInfo::default();
    for line in s.lines() {
        let mut it = line.split_whitespace();
        let (Some(key), Some(value)) = (it.next(), it.next()) else {
            continue;
        };
        let Ok(kib) = value.parse::<u64>() else {
            continue;
        };
        match key {
            "MemTotal:" => m.total = kib * 1024,
            "MemFree:" => m.free = kib * 1024,
            "MemAvailable:" => m.available = kib * 1024,
            _ => {}
        }
    }
    m
}

/// Cumulative CPU time of one line of `/proc/stat`, in ticks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CpuTimes {
    pub busy: u64,
    pub total: u64,
}

impl CpuTimes {
    /// Busy share between two readings, 0..1.
    pub fn usage_since(&self, prev: &CpuTimes) -> f32 {
        let total = self.total.saturating_sub(prev.total);
        if total == 0 {
            return 0.0;
        }
        (self.busy.saturating_sub(prev.busy) as f32 / total as f32).clamp(0.0, 1.0)
    }
}

/// The aggregate line, then one entry per core indexed by CPU number.
pub fn parse_stat(s: &str) -> (CpuTimes, Vec<CpuTimes>) {
    let mut all = CpuTimes::default();
    let mut cores = Vec::new();
    for line in s.lines() {
        let mut it = line.split_whitespace();
        let Some(name) = it.next() else { continue };
        let Some(rest) = name.strip_prefix("cpu") else {
            continue;
        };
        // user nice system idle iowait irq softirq steal (guest time is
        // already counted in user and nice).
        let v: Vec<u64> = it.take(8).filter_map(|x| x.parse().ok()).collect();
        if v.len() < 4 {
            continue;
        }
        let total: u64 = v.iter().sum();
        let idle = v[3] + v.get(4).copied().unwrap_or(0);
        let t = CpuTimes {
            busy: total - idle,
            total,
        };
        if rest.is_empty() {
            all = t;
        } else if let Ok(i) = rest.parse::<usize>() {
            if cores.len() <= i {
                cores.resize(i + 1, CpuTimes::default());
            }
            cores[i] = t;
        }
    }
    (all, cores)
}

/// Kind of core in a big.LITTLE layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CoreKind {
    Performance,
    Efficiency,
    Other,
}

/// Core type and model name per CPU number, from `/proc/cpuinfo`.
pub fn parse_cpuinfo(s: &str) -> Vec<(CoreKind, &'static str)> {
    let mut out = Vec::new();
    let mut current: Option<usize> = None;
    for line in s.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        if k == "processor" {
            current = v.parse().ok();
        } else if k == "CPU part"
            && let Some(i) = current
        {
            if out.len() <= i {
                out.resize(i + 1, (CoreKind::Other, "core"));
            }
            out[i] = match v {
                "0xd85" => (CoreKind::Performance, "Cortex-X925"),
                "0xd87" => (CoreKind::Efficiency, "Cortex-A725"),
                _ => (CoreKind::Other, "core"),
            };
        }
    }
    out
}

/// Bytes read and written by one block device, from `/proc/diskstats`.
pub fn parse_diskstats<'a>(s: &'a str) -> impl Iterator<Item = (&'a str, u64, u64)> + 'a {
    s.lines().filter_map(|line| {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 10 {
            return None;
        }
        // Sectors are always 512 bytes in this file, whatever the device.
        let read = f[5].parse::<u64>().ok()? * 512;
        let written = f[9].parse::<u64>().ok()? * 512;
        Some((f[2], read, written))
    })
}

/// Bytes received and sent per interface, from `/proc/net/dev`.
pub fn parse_net_dev<'a>(s: &'a str) -> impl Iterator<Item = (&'a str, u64, u64)> + 'a {
    s.lines().skip(2).filter_map(|line| {
        let (name, rest) = line.split_once(':')?;
        let f: Vec<u64> = rest
            .split_whitespace()
            .filter_map(|x| x.parse().ok())
            .collect();
        if f.len() < 9 {
            return None;
        }
        Some((name.trim(), f[0], f[8]))
    })
}

/// Whole physical disks: no partitions, loop devices, RAM disks or
/// device-mapper volumes, which would count the same bytes twice.
pub fn physical_disks() -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir("/sys/block")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| {
            !["loop", "ram", "zram", "dm-", "md", "sr", "nbd"]
                .iter()
                .any(|p| n.starts_with(p))
        })
        .collect();
    v.sort();
    v
}

/// Interfaces backed by hardware (Ethernet, Wi-Fi, ConnectX), not bridges,
/// veth pairs or loopback.
pub fn is_physical_iface(name: &str) -> bool {
    Path::new("/sys/class/net")
        .join(name)
        .join("device")
        .exists()
}

/// Current frequency of one CPU, in MHz.
pub fn cpu_freq_mhz(cpu: usize) -> Option<u32> {
    let s = fs::read_to_string(format!(
        "/sys/devices/system/cpu/cpu{cpu}/cpufreq/scaling_cur_freq"
    ))
    .ok()?;
    s.trim().parse::<u32>().ok().map(|khz| khz / 1000)
}

/// Hottest ACPI thermal zone, in °C: the GB10 exposes its SoC sensors there.
pub fn soc_temp() -> Option<f32> {
    fs::read_dir("/sys/class/thermal")
        .ok()?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("thermal_zone"))
        .filter_map(|e| fs::read_to_string(e.path().join("temp")).ok())
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .map(|milli| milli as f32 / 1000.0)
        .reduce(f32::max)
}

pub fn uptime_secs() -> Option<u64> {
    let s = fs::read_to_string("/proc/uptime").ok()?;
    s.split_whitespace()
        .next()?
        .parse::<f64>()
        .ok()
        .map(|x| x as u64)
}

pub fn hostname() -> String {
    fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_owned())
        .unwrap_or_default()
}

/// Vendor and model from DMI, e.g. "ASUSTeK GX10" or "NVIDIA DGX Spark".
pub fn machine() -> Option<String> {
    let read = |f: &str| {
        fs::read_to_string(format!("/sys/class/dmi/id/{f}"))
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    };
    let product = read("product_name")?;
    let vendor = read("sys_vendor").map(|v| {
        v.replace("COMPUTER INC.", "")
            .replace("Corporation", "")
            .trim()
            .to_owned()
    });
    Some(match vendor {
        Some(v) if !product.starts_with(&v) => format!("{v} {product}"),
        _ => product,
    })
}

/// Short command name and full command line of a process.
pub fn process_name(pid: u32) -> (String, String) {
    let comm = fs::read_to_string(format!("/proc/{pid}/comm"))
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|_| format!("pid {pid}"));
    let cmdline = fs::read(format!("/proc/{pid}/cmdline"))
        .map(|b| {
            b.split(|&c| c == 0)
                .filter(|a| !a.is_empty())
                .map(|a| String::from_utf8_lossy(a).into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    (comm, cmdline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meminfo_in_bytes() {
        let s = "MemTotal:       127535084 kB\nMemFree:        66684848 kB\nMemAvailable:   102010424 kB\nBuffers:         1690660 kB\n";
        let m = parse_meminfo(s);
        assert_eq!(m.total, 127535084 * 1024);
        assert_eq!(m.free, 66684848 * 1024);
        assert_eq!(m.available, 102010424 * 1024);
    }

    #[test]
    fn stat_counts_iowait_as_idle() {
        let s = "cpu  100 0 50 800 50 0 0 0 0 0\ncpu0 60 0 20 400 20 0 0 0 0 0\ncpu1 40 0 30 400 30 0 0 0 0 0\nintr 12345\n";
        let (all, cores) = parse_stat(s);
        assert_eq!(
            all,
            CpuTimes {
                busy: 150,
                total: 1000
            }
        );
        assert_eq!(cores.len(), 2);
        assert_eq!(
            cores[1],
            CpuTimes {
                busy: 70,
                total: 500
            }
        );
    }

    #[test]
    fn usage_between_two_readings() {
        let a = CpuTimes {
            busy: 100,
            total: 1000,
        };
        let b = CpuTimes {
            busy: 175,
            total: 1100,
        };
        assert!((b.usage_since(&a) - 0.75).abs() < 1e-6);
        assert_eq!(a.usage_since(&a), 0.0);
    }

    #[test]
    fn cpuinfo_splits_gb10_clusters() {
        let s = "processor\t: 0\nCPU part\t: 0xd87\n\nprocessor\t: 1\nCPU part\t: 0xd85\n\nprocessor\t: 2\nCPU part\t: 0xd03\n";
        let v = parse_cpuinfo(s);
        assert_eq!(
            v,
            vec![
                (CoreKind::Efficiency, "Cortex-A725"),
                (CoreKind::Performance, "Cortex-X925"),
                (CoreKind::Other, "core")
            ]
        );
    }

    #[test]
    fn diskstats_sectors_to_bytes() {
        let s = " 259       0 nvme0n1 1000 0 2048 10 500 0 4096 20 0 30 30 0 0 0 0 0 0\n";
        let v: Vec<_> = parse_diskstats(s).collect();
        assert_eq!(v, vec![("nvme0n1", 2048 * 512, 4096 * 512)]);
    }

    #[test]
    fn net_dev_rx_tx() {
        let s = "Inter-|   Receive                                                |  Transmit\n face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed\n  enP7s7: 1234 10 0 0 0 0 0 0 5678 20 0 0 0 0 0 0\n";
        let v: Vec<_> = parse_net_dev(s).collect();
        assert_eq!(v, vec![("enP7s7", 1234, 5678)]);
    }
}

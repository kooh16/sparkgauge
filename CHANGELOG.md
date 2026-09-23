# Changelog

## 0.1.0 — 2026-09-23

First version.

- Unified memory split into GPU, CPU, cache and free, with a per-process GPU
  breakdown and a stacked history.
- GPU utilization, temperature, power, clock and performance state via NVML,
  and the processes holding GPU memory.
- CPU usage with per-core bars grouped by cluster (Cortex-X925 and
  Cortex-A725 on the GB10), cluster frequencies and SoC temperature.
- Network and disk throughput over physical devices only.
- One-minute and five-minute history, hover read-outs on every chart.

Download `sparkgauge-0.1.0-linux-aarch64.tar.gz`, extract it and run
`./install.sh`: the prebuilt binary is installed for your user, with an icon
and a launcher entry. Built on Ubuntu 24.04 for aarch64, so it runs on DGX OS
and other recent distributions of the GB10 machines.

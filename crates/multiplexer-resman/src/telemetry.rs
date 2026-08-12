//! Per-core CPU telemetry for the resource inspector.

use sysinfo::{Cpu, System};

/// One logical core's sampled usage and reservation flag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreSample {
    pub index: usize,
    pub usage: f32,
    pub reserved: bool,
}

/// Build [`CoreSample`]s from already-measured per-core usages.
///
/// `reserved` is a list of logical core indices. Out-of-range entries are ignored.
pub fn sample_cores_from(usages: &[f32], reserved: &[usize]) -> Vec<CoreSample> {
    usages
        .iter()
        .enumerate()
        .map(|(index, &usage)| CoreSample {
            index,
            usage,
            reserved: reserved.contains(&index),
        })
        .collect()
}

/// Sample per-core CPU usage via sysinfo.
///
/// The first call may report 0% because sysinfo computes usage as a delta
/// between two refreshes. This function does not sleep: callers that need a
/// non-zero first sample should wait `sysinfo::MINIMUM_CPU_UPDATE_INTERVAL`
/// and call again.
pub fn sample_cores(reserved: &[usize]) -> Vec<CoreSample> {
    let mut sys = System::new();
    sys.refresh_cpu_usage();
    let usages: Vec<f32> = sys.cpus().iter().map(Cpu::cpu_usage).collect();
    sample_cores_from(&usages, reserved)
}

const BAR_TICKS: usize = 10;

/// Ten-tick inspector bar plus a rounded integer percent, e.g. `████░░░░░░ 41%`.
pub fn format_core_bar(usage: f32) -> String {
    // NaN.clamp stays NaN; the integer casts below treat NaN as 0.
    let usage = usage.clamp(0.0, 100.0);
    let filled = (usage / 10.0).round() as usize;
    let mut bar = String::with_capacity(BAR_TICKS);
    for i in 0..BAR_TICKS {
        bar.push(if i < filled { '█' } else { '░' });
    }
    format!("{bar} {}%", usage.round() as u32)
}

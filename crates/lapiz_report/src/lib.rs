use std::fmt::Write;

use anyhow::anyhow;
use chrono::Local;
use gfxinfo::active_gpu;
use sysinfo::{System, get_current_pid};
use wgpu::{AllocatorReport, Device};

pub fn report(device: &Device) -> anyhow::Result<String> {
    let mut buf = String::new();

    let w = &mut buf;
    writeln!(w, "Generated at {}", Local::now())?;

    sysinfo_report(w)?;
    wgpu_report(w, device)?;
    Ok(buf)
}

fn sysinfo_report(w: &mut dyn Write) -> anyhow::Result<()> {
    let sys = System::new_all();
    writeln!(w, "System Info")?;

    writeln!(w, "System CPUs")?;
    for cpu in sys.cpus().iter() {
        writeln!(
            w,
            "  {} {}@{}Hz {}%",
            cpu.name(),
            cpu.brand(),
            cpu.frequency(),
            cpu.cpu_usage()
        )?;
    }

    writeln!(
        w,
        "System Memory: {} / {}",
        FmtBytes(sys.used_memory()),
        FmtBytes(sys.total_memory())
    )?;

    writeln!(
        w,
        "System Swap: {} / {}",
        FmtBytes(sys.used_swap()),
        FmtBytes(sys.total_swap())
    )?;

    let gpu = active_gpu().map_err(|e| anyhow!("{}", e))?;
    let gpu_info = gpu.info();
    writeln!(w, "System GPU: {} {}%", gpu.model(), gpu_info.load_pct())?;
    writeln!(
        w,
        "System VRAM: {} / {}",
        FmtBytes(gpu_info.used_vram()),
        FmtBytes(gpu_info.total_vram())
    )?;

    let cur_pid = get_current_pid().map_err(|e| anyhow!("{}", e))?;
    let process = sys.process(cur_pid).unwrap();

    writeln!(w, "Process CPU: {}%", process.cpu_usage())?;
    writeln!(w, "Process Memory: {}", FmtBytes(process.memory()))?;

    writeln!(w)?;

    Ok(())
}

fn wgpu_report(w: &mut dyn Write, device: &Device) -> anyhow::Result<()> {
    let Some(AllocatorReport {
        mut allocations,
        blocks,
        total_allocated_bytes,
        total_reserved_bytes,
    }) = device.generate_allocator_report()
    else {
        writeln!(w, "No WGPU report available")?;
        return Ok(());
    };
    writeln!(w, "WGPU Report")?;

    allocations.sort_by_key(|alloc| core::cmp::Reverse(alloc.size));

    writeln!(
        w,
        "Summary: {} / {}",
        FmtBytes(total_allocated_bytes),
        FmtBytes(total_reserved_bytes)
    )?;
    writeln!(w, "Blocks: {}", blocks.len())?;
    writeln!(w, "Allocations: {}", allocations.len())?;
    for (i, alloc) in allocations.iter().enumerate() {
        writeln!(w, "  #{} {}: {}", i, alloc.name, FmtBytes(alloc.size))?;
    }

    writeln!(w)?;

    Ok(())
}

struct FmtBytes(u64);

impl std::fmt::Display for FmtBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const SUFFIX: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
        let mut idx = 0;
        let mut amount = self.0 as f64;
        loop {
            if amount < 1024.0 || idx == SUFFIX.len() - 1 {
                return write!(f, "{:.2} {} ({} bytes)", amount, SUFFIX[idx], self.0);
            }

            amount /= 1024.0;
            idx += 1;
        }
    }
}

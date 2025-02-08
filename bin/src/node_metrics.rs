use std::{
    sync::mpsc::{channel, Receiver},
    time::Duration,
};

use media_server_gateway::NodeMetrics;
use systemstat::{Platform, System};

const REFRESH_INTERVAL_SECONDS: u64 = 2;

pub struct NodeMetricsCollector {
    rx: Receiver<NodeMetrics>,
}

impl Default for NodeMetricsCollector {
    fn default() -> Self {
        let (tx, rx) = channel();
        let sys = System::new();

        std::thread::spawn(move || loop {
            match measure(&sys, Duration::from_secs(REFRESH_INTERVAL_SECONDS)) {
                Ok(metric) => {
                    let _ = tx.send(metric);
                }
                Err(e) => {
                    log::error!("[NodeMetricsCollector] failed to collect metrics {e:?}")
                }
            }
        });

        Self { rx }
    }
}

impl NodeMetricsCollector {
    /// Only return data in each interval, if not return None.
    /// Node that this method must node blocking thread
    pub fn pop_measure(&mut self) -> Option<NodeMetrics> {
        self.rx.try_recv().ok()
    }
}

fn measure(sys: &impl Platform, delay: Duration) -> std::io::Result<NodeMetrics> {
    let mounts = sys.mounts()?;
    #[cfg(not(target_os = "macos"))]
    let cpu = {
        let cpu = sys.cpu_load_aggregate()?;
        std::thread::sleep(delay);
        let cpu = cpu.done()?;
        ((1.0 - cpu.idle) * 100.0) as u8
    };
    //TODO implement macos get CPU usage
    #[cfg(target_os = "macos")]
    let cpu = {
        std::thread::sleep(delay);
        0
    };
    let memory = sys.memory()?;

    let mut disk_used = 0;
    let mut disk_sum = 0;
    for mount in mounts {
        disk_sum += mount.total.as_u64();
        disk_used += mount.total.as_u64() - mount.avail.as_u64();
    }

    Ok(NodeMetrics {
        cpu,
        memory: (100 * (memory.total.as_u64() - memory.free.as_u64()) / memory.total.as_u64()) as u8,
        disk: (100 * disk_used / disk_sum) as u8,
    })
}

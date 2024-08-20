use std::{
    sync::mpsc::{channel, Receiver},
    time::Duration,
};

use media_server_gateway::NodeMetrics;
use sans_io_runtime::ErrorDebugger2;
use sysinfo::{Disks, System};

const REFRESH_INTERVAL_SECONDS: u64 = 2;

pub struct NodeMetricsCollector {
    rx: Receiver<NodeMetrics>,
}

impl Default for NodeMetricsCollector {
    fn default() -> Self {
        let (tx, rx) = channel();
        let mut sys = System::new_all();
        let mut disks = Disks::new();

        disks.refresh_list();
        sys.refresh_all();
        sys.refresh_cpu_all();

        std::thread::spawn(move || {
            loop {
                disks.refresh();
                sys.refresh_all();
                sys.refresh_cpu_all();

                let mut sum = 0.0;
                for cpu in sys.cpus() {
                    sum += cpu.cpu_usage();
                }

                let mut disk_used = 0;
                let mut disk_sum = 0;
                for disk in disks.iter() {
                    disk_sum += disk.total_space();
                    disk_used += disk.total_space() - disk.available_space();
                }

                tx.send(NodeMetrics {
                    cpu: (sum as usize / sys.cpus().len()) as u8,
                    memory: (100 * sys.used_memory() / sys.total_memory()) as u8,
                    disk: (100 * disk_used / disk_sum) as u8,
                })
                .print_err2("Collect node metrics error");

                // Sleeping to let time for the system to run for long
                // enough to have useful information.
                std::thread::sleep(Duration::from_secs(REFRESH_INTERVAL_SECONDS));
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

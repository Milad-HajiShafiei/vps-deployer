//! Background task: metrics every second, deep probes every 5 seconds.

use crate::actions;
use crate::app::{AppMessage, HISTORY, Metrics};
use crate::config::ProjectConfig;
use std::time::Duration;
use sysinfo::{Networks, System};
use tokio::sync::{mpsc, watch};

pub async fn monitor(cfg_rx: watch::Receiver<Option<ProjectConfig>>, tx: mpsc::Sender<AppMessage>) {
    let mut iv = tokio::time::interval(Duration::from_secs(1));
    let mut sys = System::new();
    let mut prev: Option<(u64, u64)> = None;
    let mut n = 0u64;

    loop {
        iv.tick().await;
        n += 1;

        sys.refresh_memory();
        sys.refresh_cpu_usage();
        let mut m = Metrics::default();
        m.mem_total = sys.total_memory();
        m.mem_used = sys.used_memory();
        m.mem_percent = if m.mem_total > 0 {
            m.mem_used as f64 / m.mem_total as f64 * 100.0
        } else {
            0.0
        };
        m.cpu_percent = sys.global_cpu_usage() as f64;

        let nets = Networks::new_with_refreshed_list();
        let (mut rx_t, mut tx_t) = (0u64, 0u64);
        for (_, d) in nets.iter() {
            rx_t += d.total_received();
            tx_t += d.total_transmitted();
        }
        if let Some((pr, pt)) = prev {
            m.rx_rate = rx_t.saturating_sub(pr) / 1024;
            m.tx_rate = tx_t.saturating_sub(pt) / 1024;
        }
        prev = Some((rx_t, tx_t));

        let cfg = cfg_rx.borrow().clone();
        let probe = cfg
            .as_ref()
            .map(|c| c.target_dir.clone())
            .unwrap_or_else(|| "/".into());
        if let Some((total, used)) = actions::disk_usage(&probe) {
            m.disk_total = total;
            m.disk_used = used;
        }
        let _ = tx.send(AppMessage::Metrics(m)).await;

        if n % 5 == 0 {
            let services = actions::service_statuses(cfg.as_ref()).await;
            let mut db = 0;
            let mut up = 0;
            if let Some(c) = &cfg {
                db = actions::dir_size(&c.db_dir).await.unwrap_or(0);
                up = actions::dir_size(&c.uploads_dir).await.unwrap_or(0);
                let (ok, detail) = actions::check_api_health(c).await;
                let _ = tx.send(AppMessage::ApiHealth { ok, detail }).await;
            }
            let _ = tx
                .send(AppMessage::Heavy {
                    services,
                    db_bytes: db,
                    uploads_bytes: up,
                })
                .await;
        }
        let _ = HISTORY; // keep constant referenced
    }
}

use std::str::FromStr;

use atm0s_sdn::{NodeAddr, NodeId};
use tokio::sync::mpsc::Sender;

/// Fetch node addrs from the given url.
/// The url should return a list of node addrs in JSON format or a single node addr.
async fn fetch_node_addrs_from_api(url: &str) -> Result<Vec<NodeAddr>, String> {
    let resp = reqwest::get(url).await.map_err(|e| e.to_string())?;
    let content = resp.text().await.map_err(|e| e.to_string())?;
    if content.starts_with('[') {
        let node_addrs: Vec<String> = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        Ok(node_addrs.into_iter().flat_map(|addr| NodeAddr::from_str(&addr)).collect())
    } else {
        Ok(vec![NodeAddr::from_str(&content).map_err(|e| e.to_string())?])
    }
}

/// Refresh seeds from url and send them to seed_tx
pub fn refresh_seeds(node_id: NodeId, seeds: &[NodeAddr], url: Option<&str>, seed_tx: Sender<NodeAddr>) {
    for seed in seeds.iter() {
        let _ = seed_tx.try_send(seed.clone());
    }

    if let Some(url) = url {
        let seed_tx = seed_tx.clone();
        let url = url.to_string();
        tokio::spawn(async move {
            log::info!("Generate seeds from uri {}", url);
            match fetch_node_addrs_from_api(&url).await {
                Ok(seeds) => {
                    log::info!("Generated seeds {:?}", seeds);
                    for seed in seeds.into_iter().filter(|s| s.node_id() != node_id) {
                        seed_tx.send(seed).await.expect("Should send seed");
                    }
                }
                Err(e) => {
                    log::error!("Failed to fetch seeds from uri: {}", e);
                }
            }
        });
    }
}

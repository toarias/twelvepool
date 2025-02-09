use std::collections::HashMap;

use futures::{stream, StreamExt};
use reqwest::Client;
use tokio::sync::mpsc;
use tokio::time;

use crate::cache::Cache;
use crate::mempoolitem::MempoolItem;
use crate::terra::Terra;
use crate::tx::Tx;

#[derive(Debug)]
pub struct Watcher {
    terra: Terra,
    cache: Cache,
    interval: time::Interval,
}

impl Watcher {
    pub fn new(
        rpc_url: String,
        lcd_url: String,
        http_client: Option<Client>,
        interval_duration: Option<time::Duration>,
    ) -> Watcher {
        Watcher {
            terra: Terra::new(rpc_url, lcd_url, http_client.unwrap_or_default()),
            cache: Cache::new(30),
            interval: time::interval(
                interval_duration.unwrap_or_else(|| time::Duration::from_millis(100)),
            ),
        }
    }

    pub fn run(mut self) -> mpsc::UnboundedReceiver<MempoolItem> {
        env_logger::init();
        let (sender, receiver) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            loop {
                match self.terra.get_unconfirmed_txs().await {
                    Ok(tx_strings) => {
                        let mut raw_txs = self.get_tx_hashes(tx_strings).await;
                        raw_txs.retain(|tx_hash, _| {
                            if self.cache.get(tx_hash).is_none() {
                                true
                            } else {
                                log::debug!("tx {} already sent", tx_hash);
                                false
                            }
                        });

                        let txs = self.get_decoded_txs(raw_txs).await;
                        txs.iter().for_each(|(tx_hash, tx)| {
                            match sender.send(MempoolItem::new(tx.clone(), tx_hash.clone())) {
                                Ok(_) => {
                                    log::info!("new tx {}", tx_hash);
                                    self.cache.set(tx_hash.clone(), tx.clone());
                                }
                                Err(err) => {
                                    log::error!("couldn't send tx {}: {}", tx_hash, err)
                                }
                            }
                        });
                    }
                    Err(err) => log::error!("couldn't get unconfirmed txs: {}", err),
                }
                let cleaned = self.cache.clear_expired();
                log::debug!("cleaned {} tx from cache", cleaned);
                self.interval.tick().await;
            }
        });

        receiver
    }

    async fn get_tx_hashes(&self, tx_strings: Vec<String>) -> HashMap<String, String> {
        let mut raw_txs: HashMap<String, String> = HashMap::new();

        let items: Vec<Option<(String, String)>> = stream::iter(tx_strings)
            .map(|tx_string| async move {
                if let Ok(tx_hash) = self.terra.get_tx_hash(&tx_string).await {
                    Some((tx_hash, tx_string))
                } else {
                    None
                }
            })
            .buffered(usize::MAX)
            .collect()
            .await;

        items.into_iter().for_each(|item| {
            if let Some((tx_hash, tx_string)) = item {
                raw_txs.insert(tx_hash, tx_string);
            };
        });

        raw_txs
    }

    async fn get_decoded_txs(&self, raw_txs: HashMap<String, String>) -> HashMap<String, Tx> {
        let mut txs: HashMap<String, Tx> = HashMap::new();
        let items: Vec<Option<(String, Tx)>> = stream::iter(raw_txs)
            .map(|(tx_hash, tx_string)| async move {
                if let Ok(tx) = self.terra.decode_tx(&tx_string).await {
                    Some((tx_hash, tx))
                } else {
                    None
                }
            })
            .buffered(usize::MAX)
            .collect()
            .await;

        items.into_iter().for_each(|item| {
            if let Some((tx_hash, tx)) = item {
                txs.insert(tx_hash, tx);
            };
        });

        txs
    }
}

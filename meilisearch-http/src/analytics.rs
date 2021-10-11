use segment::message::{Identify, Track, User};
use segment::{AutoBatcher, Batcher, Client, HttpClient, Message};
use serde_json::{json, Value};
use std::fmt::Display;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use sysinfo::DiskExt;
use sysinfo::ProcessorExt;
use sysinfo::System;
use sysinfo::SystemExt;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{Data, Opt};

const SEGMENT_API_KEY: &str = "vHi89WrNDckHSQssyUJqLvIyp2QFITSC";

pub struct Analytics {
    user: User,
    batcher: Mutex<AutoBatcher>,
}

impl Analytics {
    pub fn publish(&'static self, event_name: String, send: Value) {
        tokio::spawn(async move {
            let _ = self
                .batcher
                .lock()
                .await
                .push(Track {
                    user: self.user.clone(),
                    event: event_name.clone(),
                    properties: send,
                    ..Default::default()
                })
                .await;
            println!("ANALYTICS: {} added to batch", event_name)
        });
    }

    // this will be deleted in the final implementation
    pub fn send_identify(&'static self) {}

    pub fn tick(&'static self, data: Data) {
        tokio::spawn(async move {
            let first_start = Instant::now();

            loop {
                tokio::time::sleep(Duration::from_secs(60)).await; // 1 minutes
                println!("ANALYTICS: should do things");

                if let Ok(stats) = dbg!(data.index_controller.get_all_stats().await) {
                    let number_of_documents = stats
                        .indexes
                        .values()
                        .map(|index| index.number_of_documents)
                        .collect::<Vec<u64>>();
                    let value = json!({
                       "Elapsed since start (in secs)": first_start.elapsed().as_secs(),
                       "Number of indexes": stats.indexes.len(),
                       "Number of documents": number_of_documents,
                       "Database size": stats.database_size,
                       "User email": std::env::var("MEILI_USER_EMAIL").ok(),
                       "Server provider": std::env::var("MEILI_SERVER_PROVIDER").ok(),
                    });
                    let user = self.user.clone();
                    println!("ANALYTICS: Pushing our identify tick");
                    let _ = self
                        .batcher
                        .lock()
                        .await
                        .push(Identify {
                            user,
                            traits: value,
                            ..Default::default()
                        })
                        .await;
                }
                println!("ANALYTICS: Pushing our batch");
                let _ = self.batcher.lock().await.flush().await;
            }
        });
    }
}

impl Analytics {
    pub async fn new(opt: Opt) -> &'static Self {
        let user_id = std::fs::read_to_string(opt.db_path.join("user-id"));
        let first_time_run = user_id.is_err();
        let user_id = user_id.unwrap_or(Uuid::new_v4().to_string());
        let _ = std::fs::write(opt.db_path.join("user-id"), user_id.as_bytes());
        let client = HttpClient::default();
        let user = User::UserId {
            user_id: user_id.clone(),
        };
        let batcher = Batcher::new(None);
        let batcher = Mutex::new(AutoBatcher::new(
            client,
            batcher,
            SEGMENT_API_KEY.to_string(),
        ));
        let segment = Box::new(Self { user, batcher });
        let segment = Box::leak(segment);

        // send an identify event
        let _ = segment
            .batcher
            .lock()
            .await
            .push(Identify {
                user: segment.user.clone(),
                traits: Self::compute_traits(&opt),
                ..Default::default()
            })
            .await;
        println!("ANALYTICS: pushed the identify event");

        // send the associated track event
        if first_time_run {
            segment.publish("Launched for the first time".to_string(), json!({}));
        }
        segment
    }

    fn compute_traits(opt: &Opt) -> Value {
        let mut sys = System::new_all();
        sys.refresh_all();

        json!({
            "System configuration": {
                "Distribution": sys.name(),
                "Kernel Version": sys.kernel_version(),
                "OS Version": sys.os_version(),
                "Total RAM (in KB)": sys.total_memory(),
                "Used RAM (in KB)": sys.used_memory(),
                "Nb CPUs": sys.processors().len(),
                "Avg CPU frequency": sys.processors().iter().map(|cpu| cpu.frequency()).sum::<u64>() / sys.processors().len() as u64,
                "Total disk space (in bytes)": sys.disks().iter().map(|disk| disk.total_space()).sum::<u64>(),
                "Available memory (in bytes)": sys.disks().iter().map(|disk| disk.available_space()).sum::<u64>(),
            },
            "Meilisearch configuration": {
                "Package version": env!("CARGO_PKG_VERSION").to_string(),
                "Environment": opt.env.clone(),
                "Max index size": opt.max_index_size.get_bytes(),
                "Max udb size": opt.max_udb_size.get_bytes(),
                "HTTP payload size limit": opt.http_payload_size_limit.get_bytes(),
                "Snapshot enabled": opt.schedule_snapshot,
            },
        })
    }
}

impl Display for Analytics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.user)
    }
}

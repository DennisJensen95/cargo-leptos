use std::sync::LazyLock;
use tokio::{
    signal,
    sync::{broadcast, RwLock},
    task::JoinHandle,
};

use crate::compile::{Change, ChangeSet};
use crate::internal_prelude::*;

static ANY_INTERRUPT: LazyLock<broadcast::Sender<()>> = LazyLock::new(|| broadcast::channel(10).0);
static SHUTDOWN: LazyLock<broadcast::Sender<()>> = LazyLock::new(|| broadcast::channel(1).0);

static SHUTDOWN_REQUESTED: LazyLock<RwLock<bool>> = LazyLock::new(|| RwLock::new(false));
static SOURCE_CHANGES: LazyLock<RwLock<ChangeSet>> =
    LazyLock::new(|| RwLock::new(ChangeSet::default()));

pub struct Interrupt {}

impl Interrupt {
    pub async fn is_shutdown_requested() -> bool {
        *SHUTDOWN_REQUESTED.read().await
    }

    pub fn subscribe_any() -> broadcast::Receiver<()> {
        ANY_INTERRUPT.subscribe()
    }

    pub fn subscribe_shutdown() -> broadcast::Receiver<()> {
        SHUTDOWN.subscribe()
    }

    pub async fn get_source_changes() -> ChangeSet {
        SOURCE_CHANGES.read().await.clone()
    }

    pub async fn clear_source_changes() {
        let mut ch = SOURCE_CHANGES.write().await;
        ch.clear();
        trace!("Interrupt source changed cleared");
    }

    pub fn send_all_changed() {
        let mut ch = SOURCE_CHANGES.blocking_write();
        *ch = ChangeSet::all_changes();
        drop(ch);
        Self::send_any()
    }

    pub fn send(changes: &[Change]) {
        let mut ch = SOURCE_CHANGES.blocking_write();
        for change in changes {
            ch.add(change.clone());
        }
        drop(ch);

        Self::send_any();
    }

    fn send_any() {
        if let Err(e) = ANY_INTERRUPT.send(()) {
            error!("Interrupt error could not send due to: {e}");
        } else {
            trace!("Interrupt send done");
        }
    }

    pub async fn request_shutdown() {
        {
            *SHUTDOWN_REQUESTED.write().await = true;
        }
        _ = SHUTDOWN.send(());
        _ = ANY_INTERRUPT.send(());
    }

    pub fn run_ctrl_c_monitor() -> JoinHandle<()> {
        tokio::spawn(async move {
            signal::ctrl_c().await.expect("failed to listen for event");
            info!("Leptos ctrl-c received");
            Interrupt::request_shutdown().await;
        })
    }
}

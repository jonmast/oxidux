use std::time::Duration;
use tokio::signal;
use tokio::sync::oneshot;
use tokio::time::timeout;

use crate::process_manager::ProcessManager;
use crate::tmux;

pub(crate) async fn ctrlc_listener() {
    let (tx, rx) = oneshot::channel::<()>();

    let mut shutdown_tx = Some(tx);

    let signal_handler = async move {
        loop {
            signal::ctrl_c().await.unwrap();

            if let Some(tx) = shutdown_tx.take() {
                eprintln!("Gracefully shutting down");

                timeout(
                    Duration::from_secs(10),
                    ProcessManager::global().write().await.shutdown(),
                )
                .await
                .ok();

                tmux::kill_server().await.ok();
                eprintln!("All processes have stopped");

                tx.send(()).unwrap();
            } else {
                eprintln!("Forcibly shutting down");
                std::process::exit(1);
            }
        }
    };

    tokio::spawn(signal_handler);

    rx.await.ok();
}

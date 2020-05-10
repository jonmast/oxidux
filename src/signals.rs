use tokio::signal;
use tokio::sync::oneshot;

use crate::process_manager::ProcessManager;

pub(crate) async fn ctrlc_listener() {
    let (tx, rx) = oneshot::channel::<()>();

    let mut shutdown_tx = Some(tx);

    let signal_handler = async move {
        loop {
            signal::ctrl_c().await.unwrap();

            if let Some(tx) = shutdown_tx.take() {
                eprintln!("Gracefully shutting down");

                ProcessManager::global().write().await.shutdown().await;

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

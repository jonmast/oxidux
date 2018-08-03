use config;
use futures::future::Future;
use futures::Stream;
use std::fs;
use std::str;
use tokio;
use tokio_uds::UnixListener;

pub fn start_ipc_sock() {
    let path = config::config_dir().join("oxidux.sock");
    fs::remove_file(&path).ok();

    let sock = UnixListener::bind(&path).expect("Failed to open socket");

    let listener = sock
        .incoming()
        .for_each(|connection| {
            let buf = vec![];
            let reader = tokio::io::read_to_end(connection, buf)
                .and_then(|(_, buf)| {
                    let txt = str::from_utf8(&buf);
                    println!("Got {:?}", txt);
                    Ok(())
                })
                .map_err(|err| println!("Couldn't read message, got error: {}", err));

            tokio::spawn(reader);

            Ok(())
        })
        .map_err(|err| eprintln!("Failed to open socket, got error {:?}", err));

    tokio::spawn(listener);
}

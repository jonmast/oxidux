use config;
use ipc_command::IPCCommand;

use futures::future::Future;
use futures::Stream;
use process_manager::ProcessManager;
use serde_json;
use std::fs;
use std::str;
use tokio;
use tokio_uds::UnixListener;

pub fn start_ipc_sock(process_manager: ProcessManager) {
    let path = config::socket_path();
    fs::remove_file(&path).ok();

    let sock = UnixListener::bind(&path).expect("Failed to open socket");

    let listener = sock
        .incoming()
        .for_each(move |connection| {
            let connection_pm = process_manager.clone();
            let buf = vec![];
            let reader = tokio::io::read_to_end(connection, buf)
                .and_then(move |(_, buf)| {
                    parse_incoming_command(&buf, &connection_pm);

                    Ok(())
                }).map_err(|err| eprintln!("Couldn't read message, got error: {}", err));

            tokio::spawn(reader);

            Ok(())
        }).map_err(|err| eprintln!("Failed to open socket, got error {:?}", err));

    tokio::spawn(listener);
}

fn parse_incoming_command(buf: &[u8], process_manager: &ProcessManager) {
    let txt = str::from_utf8(&buf);

    if let Ok(raw_json) = txt {
        let command: IPCCommand =
            serde_json::from_str(raw_json).expect("Failed to parse, is it a valid JSON command?");

        run_command(&command, process_manager);
    }
}

fn run_command(command: &IPCCommand, process_manager: &ProcessManager) {
    match command.command.as_ref() {
        "restart" => {
            eprintln!("Restarting {}", command.args[0]);
            process_manager
                .find_process(&command.args[0])
                .expect("Failed to find app to restart")
                .restart();
        }
        cmd_str => eprintln!("Unknown command {}", cmd_str),
    }
}

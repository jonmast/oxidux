use crate::config;
use crate::ipc_command::IPCCommand;

use failure::{err_msg, Error};
use futures::future::Future;
use futures::Stream;
use serde_json;
use std::fs;
use std::io::{BufReader, Write};
use std::str;
use tokio;
use tokio_io::io::WriteHalf;
use tokio_io::AsyncRead;
use tokio_uds::{UnixListener, UnixStream};

use crate::ipc_response::IPCResponse;
use crate::process::Process;
use crate::process_manager::ProcessManager;

fn read_command(process_manager: &ProcessManager, connection: UnixStream) {
    let (reader, writer) = connection.split();
    let reader = BufReader::new(reader);
    let msg = vec![];

    let process_manager = process_manager.clone();

    tokio::spawn(
        tokio::io::read_until(reader, b'\n', msg)
            .and_then(move |(_, buf)| {
                let command = parse_incoming_command(&buf)
                    .expect("Failed to parse command, is it valid JSON?");

                run_command(&process_manager, &command, writer);

                Ok(())
            })
            .map_err(|e| eprintln!("Got error reading command {}", e)),
    );
}

fn run_command(
    process_manager: &ProcessManager,
    command: &IPCCommand,
    writer: WriteHalf<UnixStream>,
) {
    match command.command.as_ref() {
        "restart" => restart_app(process_manager, command, writer),
        "connect" => connect_output(process_manager, command, writer),
        "ping" => heartbeat_response(writer),
        cmd_str => eprintln!("Unknown command {}", cmd_str),
    }
}

fn parse_incoming_command(buf: &[u8]) -> Result<IPCCommand, Error> {
    let raw_json = str::from_utf8(&buf)?;

    let command: IPCCommand = serde_json::from_str(raw_json)?;

    Ok(command)
}

pub fn start_ipc_sock(process_manager: ProcessManager) {
    let path = config::socket_path();
    fs::remove_file(&path).ok();

    let sock = UnixListener::bind(&path).expect("Failed to open socket");

    let listener = sock
        .incoming()
        .for_each(move |connection| {
            read_command(&process_manager.clone(), connection);

            Ok(())
        })
        .map_err(|err| eprintln!("Failed to open socket, got error {:?}", err));

    tokio::spawn(listener);
}

fn send_response(process: Result<&Process, Error>, mut writer: WriteHalf<UnixStream>) {
    let response = IPCResponse::for_process(process);

    let json = serde_json::to_string(&response).unwrap();
    writer
        .write_all(&json.as_ref())
        .map_err(|_| eprintln!("Error writing IPC response"))
        .unwrap();
}

fn connect_output(
    process_manager: &ProcessManager,
    command: &IPCCommand,
    writer: WriteHalf<UnixStream>,
) {
    let process = lookup_process(process_manager, &command.args);

    send_response(
        process.ok_or_else(|| err_msg("Failed to find app to connect to")),
        writer,
    );

    if process.is_none() {
        eprintln!("Failed to find app to connect to");
    }
}

fn restart_app(
    process_manager: &ProcessManager,
    command: &IPCCommand,
    writer: WriteHalf<UnixStream>,
) {
    let process = lookup_process(&process_manager, &command.args);

    send_response(
        process.ok_or_else(|| err_msg("Failed to find app to restart")),
        writer,
    );

    match process {
        Some(process) => {
            process.restart();
        }
        None => eprintln!("Failed to find app to restart"),
    }
}

fn lookup_process<'a>(process_manager: &'a ProcessManager, args: &[String]) -> Option<&'a Process> {
    let app = process_manager.find_app_for_directory(&args[1])?;

    match args[0].as_ref() {
        "" => app.default_process(),
        name => app.find_process(name),
    }
}

fn heartbeat_response(mut writer: WriteHalf<UnixStream>) {
    writer
        .write_all(b"pong")
        .map_err(|_| eprintln!("Failed to send heartbeat response"))
        .unwrap();
}

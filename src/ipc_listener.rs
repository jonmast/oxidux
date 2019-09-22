use crate::config;
use crate::ipc_command::IPCCommand;

use failure::{err_msg, Error};
use futures::StreamExt;
use serde_json;
use std::fs;
use std::str;
use tokio;
use tokio::io::BufReader;
use tokio::net::{UnixListener, UnixStream};
use tokio::prelude::*;

use crate::ipc_response::IPCResponse;
use crate::process::Process;
use crate::process_manager::ProcessManager;

fn read_command(process_manager: &ProcessManager, mut connection: UnixStream) {
    let process_manager = process_manager.clone();

    tokio::spawn(async move {
        let (reader, writer) = connection.split();
        let mut msg = vec![];

        let mut reader = BufReader::new(reader);

        reader.read_until(b'\n', &mut msg).await.unwrap();
        let command =
            parse_incoming_command(&msg).expect("Failed to parse command, is it valid JSON?");

        run_command(&process_manager, &command, writer).await;
    });
}

async fn run_command<T>(process_manager: &ProcessManager, command: &IPCCommand, writer: T)
where
    T: AsyncWrite + Unpin,
{
    match command.command.as_ref() {
        "restart" => restart_app(process_manager, command, writer).await,
        "connect" => connect_output(process_manager, command, writer).await,
        "ping" => heartbeat_response(writer).await,
        cmd_str => eprintln!("Unknown command {}", cmd_str),
    }
}

fn parse_incoming_command(buf: &[u8]) -> Result<IPCCommand, Error> {
    let raw_json = str::from_utf8(&buf)?;

    let command: IPCCommand = serde_json::from_str(raw_json)?;

    Ok(command)
}

pub fn start_ipc_sock(process_manager: ProcessManager) {
    let listener = async move {
        let path = config::socket_path();
        fs::remove_file(&path).ok();
        let mut sock = UnixListener::bind(&path).expect("Failed to create IPC socket");
        let mut incoming = sock.incoming();

        while let Some(connection) = incoming.next().await {
            match connection {
                Ok(connection) => read_command(&process_manager.clone(), connection),
                Err(err) => eprintln!("Failed to read from IPC socket, got error {:?}", err),
            };
        }
    };

    tokio::spawn(listener);
}

async fn send_response<T>(process: Result<&Process, Error>, mut writer: T)
where
    T: AsyncWrite + Unpin,
{
    let response = IPCResponse::for_process(process);

    let json = serde_json::to_string(&response).unwrap();
    writer.write_all(&json.as_ref()).await.unwrap();
}

async fn connect_output(
    process_manager: &ProcessManager,
    command: &IPCCommand,
    writer: impl AsyncWrite + Unpin,
) {
    let process = lookup_process(process_manager, &command.args);

    send_response(
        process.ok_or_else(|| err_msg("Failed to find app to connect to")),
        writer,
    )
    .await;

    if process.is_none() {
        eprintln!("Failed to find app to connect to");
    }
}

async fn restart_app(
    process_manager: &ProcessManager,
    command: &IPCCommand,
    writer: impl AsyncWrite + Unpin,
) {
    let process = lookup_process(&process_manager, &command.args);

    send_response(
        process.ok_or_else(|| err_msg("Failed to find app to restart")),
        writer,
    )
    .await;

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

async fn heartbeat_response(mut writer: impl AsyncWrite + Unpin) {
    writer
        .write_all(b"pong")
        .await
        .expect("Failed to send heartbeat response")
}

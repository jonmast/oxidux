use crate::config;
use crate::ipc_command::IPCCommand;

use failure::{err_msg, Error};
use futures::StreamExt;
use serde_json;
use std::str;
use tokio;
use tokio::fs;
use tokio::io::BufReader;
use tokio::net::{UnixListener, UnixStream};
use tokio::prelude::*;

use crate::ipc_response::IPCResponse;
use crate::process::Process;
use crate::process_manager::ProcessManager;

fn read_command(mut connection: UnixStream) {
    tokio::spawn(async move {
        let (reader, writer) = connection.split();
        let mut msg = vec![];

        let mut reader = BufReader::new(reader);

        reader.read_until(b'\n', &mut msg).await.unwrap();
        let command =
            parse_incoming_command(&msg).expect("Failed to parse command, is it valid JSON?");

        run_command(&command, writer).await;
    });
}

async fn run_command<T>(command: &IPCCommand, writer: T)
where
    T: AsyncWrite + Unpin,
{
    match command.command.as_ref() {
        "restart" => restart_app(command, writer).await,
        "connect" => connect_output(command, writer).await,
        "ping" => heartbeat_response(writer).await,
        cmd_str => eprintln!("Unknown command {}", cmd_str),
    }
}

fn parse_incoming_command(buf: &[u8]) -> Result<IPCCommand, Error> {
    let raw_json = str::from_utf8(&buf)?;

    let command: IPCCommand = serde_json::from_str(raw_json)?;

    Ok(command)
}

pub fn start_ipc_sock() {
    let listener = async move {
        let path = config::socket_path();
        fs::remove_file(&path).await.ok();
        let mut sock = UnixListener::bind(&path).expect("Failed to create IPC socket");
        let mut incoming = sock.incoming();

        while let Some(connection) = incoming.next().await {
            match connection {
                Ok(connection) => read_command(connection),
                Err(err) => eprintln!("Failed to read from IPC socket, got error {:?}", err),
            };
        }
    };

    tokio::spawn(listener);
}

async fn send_response<T>(process: &Result<Process, Error>, mut writer: T)
where
    T: AsyncWrite + Unpin,
{
    let response = IPCResponse::for_process(process).await;

    let json = serde_json::to_string(&response).unwrap();
    writer.write_all(&json.as_ref()).await.unwrap();
}

async fn connect_output(command: &IPCCommand, writer: impl AsyncWrite + Unpin) {
    let process = {
        let process_manager = ProcessManager::global().read().await;
        lookup_process(&process_manager, &command.args).await
    };

    let process = process.ok_or_else(|| err_msg("Failed to find app to connect to"));
    send_response(&process, writer).await;

    if let Err(e) = process {
        eprintln!("{}", e);
    }
}

async fn restart_app(command: &IPCCommand, writer: impl AsyncWrite + Unpin) {
    let process = {
        let process_manager = ProcessManager::global().read().await;
        lookup_process(&process_manager, &command.args).await
    };

    let process = process.ok_or_else(|| err_msg("Failed to find app to restart"));

    send_response(&process, writer).await;

    match process {
        Ok(process) => {
            process.restart().await;
        }
        Err(e) => eprintln!("{}", e),
    }
}

async fn lookup_process(process_manager: &ProcessManager, args: &[String]) -> Option<Process> {
    let app = process_manager.find_app_for_directory(&args[1])?;

    match args[0].as_ref() {
        "" => app.default_process(),
        name => app.find_process(name).await,
    }
    .cloned()
}

async fn heartbeat_response(mut writer: impl AsyncWrite + Unpin) {
    writer
        .write_all(b"pong")
        .await
        .expect("Failed to send heartbeat response")
}

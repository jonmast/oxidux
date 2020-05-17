use crate::config;
use crate::ipc_command::IPCCommand;

use failure::{err_msg, Error};
use futures::StreamExt;
use std::str;
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
    match command {
        IPCCommand::Restart {
            process_name,
            directory,
        } => restart_app(process_name, directory, writer).await,
        IPCCommand::Connect {
            process_name,
            directory,
        } => connect_output(process_name, directory, writer).await,
        IPCCommand::Stop {
            app_name,
            directory,
        } => stop_app(app_name, directory, writer).await,
        IPCCommand::Ping => heartbeat_response(writer).await,
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

async fn connect_output(
    process_name: &Option<String>,
    directory: &str,
    writer: impl AsyncWrite + Unpin,
) {
    let process = {
        let process_manager = ProcessManager::global().read().await;
        lookup_process(&process_manager, process_name, directory).await
    };

    let process = process.ok_or_else(|| err_msg("Failed to find app to connect to"));
    send_response(&process, writer).await;

    if let Err(e) = process {
        eprintln!("{}", e);
    }
}

async fn restart_app(
    process_name: &Option<String>,
    directory: &str,
    writer: impl AsyncWrite + Unpin,
) {
    let process = {
        let process_manager = ProcessManager::global().read().await;
        lookup_process(&process_manager, process_name, directory).await
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

async fn stop_app(app_name: &Option<String>, directory: &str, mut writer: impl AsyncWrite + Unpin) {
    let mut process_manager = ProcessManager::global().write().await;
    let app = {
        match app_name {
            Some(app_name) => process_manager.find_app_by_name(app_name),
            None => process_manager.find_app_for_directory(directory),
        }
    }
    .cloned();

    let response = match app {
        Some(app) => {
            app.stop().await;
            (&mut process_manager).remove_app_by_name(app.name());
            format!("Stopping {}", app.name())
        }
        None => "Failed to find app to stop".to_string(),
    };

    let json = serde_json::to_string(&IPCResponse::Status(response)).unwrap();

    writer
        .write_all(json.as_ref())
        .await
        .expect("Failed to send stop response")
}

async fn lookup_process(
    process_manager: &ProcessManager,
    process_name: &Option<String>,
    directory: &str,
) -> Option<Process> {
    let app = process_manager.find_app_for_directory(directory)?;

    match process_name {
        Some(name) => app.find_process(name).await,
        None => app.default_process().await,
    }
    .cloned()
}

async fn heartbeat_response(mut writer: impl AsyncWrite + Unpin) {
    writer
        .write_all(b"pong")
        .await
        .expect("Failed to send heartbeat response")
}

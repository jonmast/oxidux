use std::fs::File;
use std::io::Write;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::process::Command;

use eyre::{bail, eyre, WrapErr};

use super::SetupArgs;
use super::SetupResult;

pub(super) fn setup(args: &SetupArgs) -> SetupResult {
    verify_tld_resolver(args)?;
    configure_systemd(args)?;

    Ok(())
}

fn verify_tld_resolver(args: &SetupArgs) -> SetupResult {
    println!("Checking \".{}\" DNS resolution", args.domain);

    let lookup = (format!("oxidux.{}", args.domain), 0).to_socket_addrs();

    if lookup.is_ok() {
        Ok(())
    } else {
        Err(eyre!(
            "Unable to resolve {} domain, install and configure dev-tld-resolver to proceed\nhttps://github.com/puma/dev-tld-resolver",
            args.domain
        ))
    }
}

fn configure_systemd(args: &SetupArgs) -> SetupResult {
    configure_systemd_socket()?;
    configure_systemd_service(args)
}

fn configure_systemd_socket() -> SetupResult {
    let socket_path = "/etc/systemd/system/oxidux.socket";
    if Path::new(socket_path).exists() {
        println!(
            "WARNING: Socket file {} already exists, overwriting it",
            socket_path
        );
    }
    let mut file = match File::create(socket_path) {
        Err(e) => {
            if let std::io::ErrorKind::PermissionDenied = e.kind() {
                return Err(e).context("Permissions error, try running with sudo");
            } else {
                return Err(e).context("Error creating sytemd socket file");
            }
        }
        Ok(file) => file,
    };

    file.write_all(SYSTEMD_SOCKET.as_bytes())
        .context("Error writing systemd socket file")?;

    Ok(())
}

fn configure_systemd_service(args: &SetupArgs) -> SetupResult {
    let service_path = "/etc/systemd/system/oxidux.service";
    if Path::new(service_path).exists() {
        println!(
            "WARNING: Service file {} already exists, overwriting it",
            service_path
        );
    }
    let mut file = match File::create(service_path) {
        Err(e) => {
            if let std::io::ErrorKind::PermissionDenied = e.kind() {
                return Err(e).context("Permissions error, try running with sudo");
            } else {
                return Err(e).context("Error creating sytemd service file");
            }
        }
        Ok(file) => file,
    };

    let path = std::env::var("PATH")?;
    let exe_path = std::env::current_exe()?;

    file.write_all(
        systemd_service(
            &args.user,
            &args.home_dir.to_str().unwrap(),
            &path,
            &exe_path.to_str().unwrap(),
            &args.config_file.to_str().unwrap(),
        )
        .as_bytes(),
    )
    .context("Error writing systemd service file")?;

    enable_service()?;

    Ok(())
}

const SYSTEMD_SOCKET: &str = "[Unit]
Description=Oxidux Server Activation Socket

[Socket]
ListenStream=80
# TODO https

[Install]
WantedBy=default.target";

fn systemd_service(
    user: &str,
    home: &str,
    path: &str,
    exe_path: &str,
    config_file: &str,
) -> String {
    format!(
        "[Unit]
Description=Oxidux
After=network.target

[Service]
User={user}
Environment=HOME={home}
Environment=PATH={path}
ExecStart={exe_path} server {config_file}

[Install]
WantedBy=multi-user.target",
        user = user,
        home = home,
        path = path,
        exe_path = exe_path,
        config_file = config_file
    )
}

fn enable_service() -> SetupResult {
    daemon_reload()?;
    enable_socket()?;
    start_socket()
}

fn daemon_reload() -> SetupResult {
    println!("Reloading systemd daemon");
    let result = Command::new("systemctl")
        .arg("daemon-reload")
        .status()
        .map(|status| status.success());

    if let Ok(true) = result {
        Ok(())
    } else {
        bail!("Error reloading systemd")
    }
}

fn enable_socket() -> SetupResult {
    println!("Enabling socket");
    let result = Command::new("systemctl")
        .arg("enable")
        .arg("oxidux.socket")
        .status()
        .map(|status| status.success());

    if let Ok(true) = result {
        Ok(())
    } else {
        bail!("Error in enabling socket")
    }
}

fn start_socket() -> SetupResult {
    println!("Starting socket");
    let result = Command::new("systemctl")
        .arg("start")
        .arg("oxidux.socket")
        .status()
        .map(|status| status.success());

    if let Ok(true) = result {
        Ok(())
    } else {
        bail!("Error in starting socket")
    }
}

use std::fs::{create_dir, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use eyre::{bail, eyre, WrapErr};
use hyper::Client;
use nix::unistd::User;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

pub fn setup() {
    println!("Welcome to Oxidux!");
    if let Err(e) = try_setup() {
        eprintln!("Error: {:?}", e);
        eprintln!("Aborting setup");
    } else {
        println!("Setup complete");
    }
}

type SetupResult = color_eyre::Result<()>;

struct SetupArgs {
    domain: String,
    home_dir: PathBuf,
    config_file: PathBuf,
    user: String,
}

fn try_setup() -> SetupResult {
    let domain = dialoguer::Input::new()
        .with_prompt("Top level domain for your apps")
        .default(crate::config::default_domain())
        .interact_text()
        .context("Domain input failed")?;

    verify_tmux()?;

    let user = std::env::var("SUDO_USER").context("Unable to determine user account")?;
    let home_dir = User::from_name(&user)?
        .ok_or_else(|| eyre!("User has no configured home directory"))?
        .dir;
    let config_dir = home_dir.join(".oxidux");
    if !config_dir.is_dir() {
        create_dir(&config_dir)?;
    }
    let config_file = config_dir.join("config.toml");

    let args = SetupArgs {
        domain,
        home_dir,
        config_file,
        user,
    };

    write_config(&args)?;

    imp::setup(&args)?;

    test_connection(&args)?;

    Ok(())
}

fn verify_tmux() -> SetupResult {
    println!("Checking for tmux command");
    let result = Command::new("tmux").arg("-V").status();

    if let Ok(status) = result {
        if status.success() {
            return Ok(());
        }
    }

    bail!("Tmux command not found\nPlease install tmux before proceeding")
}

#[tokio::main]
async fn test_connection(args: &SetupArgs) -> SetupResult {
    let url = format!("http://connection-test.{}/", args.domain);
    println!("Checking connection to {}", url);

    let response = Client::new().get(url.parse()?).await?;

    let bytes = hyper::body::to_bytes(response.into_body()).await?;
    let body = std::str::from_utf8(&bytes)?;

    if body.contains("App not found") {
        Ok(())
    } else {
        bail!("Unexpected response body\n {}", body)
    }
}

fn write_config(args: &SetupArgs) -> SetupResult {
    println!("Writing config to {}", args.config_file.to_string_lossy());
    let toml = format!("[general]\ndomain = \"{}\"", args.domain);

    let mut config_file = File::create(&args.config_file)?;
    config_file.write_all(&toml.as_bytes())?;

    Ok(())
}

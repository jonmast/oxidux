extern crate clap;
extern crate oxidux;

use clap::{App, AppSettings, Arg, SubCommand};
use oxidux::config;

fn main() {
    let matches = App::new("oxidux")
        .about("Manage processes in development")
        .subcommand(
            SubCommand::with_name("server")
                .about("Start proxy server")
                .arg(
                    Arg::with_name("config")
                        .value_name("CONFIG_FILE")
                        .help("App config file")
                        .default_value("apps.toml"),
                ),
        ).subcommand(
            SubCommand::with_name("restart")
                .about("Restart a process")
                .arg(
                    Arg::with_name("process")
                        .value_name("PROCESS_NAME")
                        .help("Name of process to restart")
                        .required(true),
                ),
        ).setting(AppSettings::SubcommandRequiredElseHelp)
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("server") {
        let config_file = matches.value_of("config").unwrap();
        let config = config::read_config(config_file);
        oxidux::run_server(config);
    } else if let Some(matches) = matches.subcommand_matches("restart") {
        let process_name = matches.value_of("process").unwrap();
        oxidux::client::restart_process(process_name);
    }
}

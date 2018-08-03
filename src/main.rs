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
        )
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("server") {
        let config_file = matches.value_of("config").unwrap();
        let config = config::read_config(config_file);
        oxidux::run_server(config);
    }
}

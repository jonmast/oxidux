use clap::{App, AppSettings, Arg, SubCommand};
use oxidux::config;

fn main() -> color_eyre::Result<()> {
    let matches = App::new("oxidux")
        .about("Manage processes in development")
        .subcommand(
            SubCommand::with_name("server")
                .about("Start proxy server")
                .arg(
                    Arg::with_name("config")
                        .value_name("CONFIG_FILE")
                        .help("App config file"),
                ),
        )
        .subcommand(
            SubCommand::with_name("restart")
                .about("Restart a process")
                .arg(
                    Arg::with_name("process")
                        .value_name("PROCESS_NAME")
                        .help("Name of process to restart"),
                ),
        )
        .subcommand(
            SubCommand::with_name("connect")
                .about("Connect to STDIN/STDOUT of a running process")
                .arg(
                    Arg::with_name("process")
                        .value_name("PROCESS_NAME")
                        .help("Name of process to connect to"),
                ),
        )
        .subcommand(
            SubCommand::with_name("stop").about("Shut down app").arg(
                Arg::with_name("app_name")
                    .value_name("APP_NAME")
                    .help("Name of app to stop (defaults to app for current directory)"),
            ),
        )
        .subcommand(SubCommand::with_name("setup").about("Configure Oxidux"))
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .get_matches();

    match matches.subcommand() {
        ("server", Some(matches)) => {
            let config_file = matches.value_of("config").unwrap();
            let config = config::read_config(config_file);
            oxidux::run_server(config);
        }
        ("restart", Some(matches)) => {
            let process_name = matches.value_of("process");
            oxidux::client::restart_process(process_name)?;
        }
        ("connect", Some(matches)) => {
            let process_name = matches.value_of("process");
            oxidux::client::connect_to_process(process_name)?;
        }
        ("stop", Some(matches)) => {
            let app_name = matches.value_of("app_name");
            oxidux::client::stop_app(app_name)?;
        }
        ("setup", Some(_matches)) => {
            oxidux::setup::setup();
        }
        (command, _) => panic!("Unrecognized command {}", command),
    }

    Ok(())
}

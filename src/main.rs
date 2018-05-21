extern crate oxidux;
use oxidux::config;

fn main() {
    let config = config::read_config();
    oxidux::run_server(config);
}

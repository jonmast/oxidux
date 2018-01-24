extern crate futures;
extern crate hyper;
extern crate tokio_core;
extern crate toml;
extern crate url;

#[macro_use]
extern crate serde_derive;

use futures::future::Future;
use futures::Stream;

use tokio_core::reactor::Core;

use hyper::server::Http;

mod proxy;
use proxy::Proxy;

mod process;
mod process_manager;
use process_manager::ProcessManager;
mod config;

fn main() {
    let mut core = Core::new().unwrap();
    let server_handle = core.handle();
    let client_handle = core.handle();

    let config = config::read_config();
    let port = config.general.proxy_port;
    let addr = format!("127.0.0.1:{}", port).parse().unwrap();

    let process_manager = ProcessManager::new(&config);
    let server = Http::new()
        .serve_addr_handle(&addr, &server_handle, move || {
            Ok(Proxy::new(client_handle.clone(), process_manager.clone()))
        })
        .unwrap();

    let spawn_handle = server_handle.clone();

    server_handle.spawn(
        server
            .for_each(move |conn| {
                spawn_handle.spawn(
                    conn.map(|_| ())
                        .map_err(|err| println!("serve error: {:?}", err)),
                );
                Ok(())
            })
            .map_err(|_| ()),
    );

    core.run(futures::future::empty::<(), ()>()).unwrap();
}

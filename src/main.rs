extern crate futures;
extern crate hyper;
extern crate tokio_core;

use futures::future::Future;
use futures::Stream;

use tokio_core::reactor::Core;

use hyper::server::Http;

mod proxy;

use proxy::Proxy;

fn main() {
    let addr = "127.0.0.1:1234".parse().unwrap();

    let mut core = Core::new().unwrap();
    let server_handle = core.handle();
    let client_handle = core.handle();

    let server = Http::new()
        .serve_addr_handle(&addr, &server_handle, move || {
            Ok(Proxy::new(client_handle.clone()))
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

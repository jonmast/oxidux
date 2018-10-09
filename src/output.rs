use futures::{Future, Stream};
use tokio;
use tokio_codec::{Framed, LinesCodec};
use tokio_pty_process;

type OutputStream = Framed<tokio_pty_process::AsyncPtyMaster, LinesCodec>;
pub struct Output {
    name: String,
    stream: OutputStream,
}

impl Output {
    pub fn new(name: String, pty: tokio_pty_process::AsyncPtyMaster) -> Self {
        let stream = Framed::new(pty, LinesCodec::new());

        Output { name, stream }
    }

    pub fn setup_writer(self) {
        let name = self.name.clone();
        let printer = self.stream.for_each(move |line| {
            println!("{}: {}", name, line);
            Ok(())
        });

        let mapped = printer.map_err(|_| ());

        tokio::spawn(mapped);
    }
}

use std::io::BufReader;

use futures::{Future, Stream};
use tokio;
use tokio_io::io;
use tokio_process::ChildStdout;

pub struct Output {
    name: String,
    stream: io::Lines<BufReader<ChildStdout>>,
}

impl Output {
    pub fn new(name: String, stdout: ChildStdout) -> Self {
        let reader = BufReader::new(stdout);
        let stream = io::lines(reader);
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

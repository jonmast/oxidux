use std::fs::File as StdFile;

use crate::process::Process;
use ansi_term::Color;
use futures::{Future, Stream};
use tokio;
use tokio::fs::File;
use tokio_codec::{Framed, LinesCodec};
use tokio_pty_process;

type OutputStream = Framed<File, LinesCodec>;
pub struct Output {
    name: String,
    process: Process,
}

impl Output {
    pub fn for_stream(fifo: StdFile, process: Process) -> Self {
        let index = process.port();
        let stream = Framed::new(File::from_std(fifo), LinesCodec::new());

        let name = pick_color(index).paint(process.app_name()).to_string();

        let output = Output { name, process };

        output.setup_writer(stream);

        output
    }

    fn setup_writer(&self, stream: OutputStream) {
        let name = self.name.clone();

        let printer = stream.for_each(move |line| {
            println!("{}: {}", pick_color(1).paint(&name), line);
            Ok(())
        });

        let process = self.process.clone();
        let mapped = printer
            .map(move |_| process.process_died())
            .map_err(|e| eprintln!("Output failed with error {}", e));

        tokio::spawn(mapped);
    }
}

fn pick_color(idx: u16) -> Color {
    let colors = [
        Color::Blue,
        Color::Green,
        Color::Purple,
        Color::Cyan,
        Color::Red,
        Color::Yellow,
    ];

    let bounded_index = (idx as usize) % colors.len();

    colors[bounded_index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_color_is_deterministic() {
        assert_eq!(pick_color(1), pick_color(1))
    }

    #[test]
    fn pick_color_different_for_different_index() {
        assert_ne!(pick_color(1), pick_color(2))
    }
}

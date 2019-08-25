use crate::process::Process;
use ansi_term::Color;
use futures::future;
use tokio::codec::{FramedRead, LinesCodec};
use tokio::prelude::*;

type OutputStream<T> = FramedRead<T, LinesCodec>;
pub struct Output {
    name: String,
    process: Process,
}

impl Output {
    pub fn for_stream<T: 'static + AsyncRead + Unpin + Send>(fifo: T, process: Process) -> Self {
        let index = process.port();
        let stream = FramedRead::new(fifo, LinesCodec::new());

        let name = pick_color(index).paint(process.name()).to_string();

        let output = Output { name, process };

        output.setup_writer(stream);

        output
    }

    fn setup_writer<T>(&self, stream: OutputStream<T>)
    where
        T: AsyncRead + Unpin + Send + 'static,
    {
        let name = self.name.clone();

        let printer = stream.for_each(move |line| {
            match line {
                Ok(line) => println!("{}: {}", pick_color(1).paint(&name), line),
                Err(error) => eprintln!("Error in log output: {}", error),
            }

            future::ready(())
        });

        let process = self.process.clone();
        let mapped = printer.map(move |_| process.process_died());

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

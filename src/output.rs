use crate::process::Process;
use ansi_term::Color;
use tokio::io::{BufReader, Lines};
use tokio::prelude::*;

type OutputStream<T> = Lines<T>;
pub struct Output {
    name: String,
    process: Process,
}

impl Output {
    pub fn for_stream<T: 'static + AsyncRead + Unpin + Send>(fifo: T, process: Process) {
        let index = process.port();
        let stream = BufReader::new(fifo).lines();

        let name = pick_color(index).paint(process.name()).to_string();

        let output = Output { name, process };

        tokio::spawn(output.setup_writer(stream));
    }

    async fn setup_writer<T>(self, mut stream: OutputStream<T>)
    where
        T: AsyncBufRead + Unpin + Send + 'static,
    {
        while let Some(line) = stream.next_line().await.transpose() {
            match line {
                Ok(line) => {
                    println!("{}: {}", pick_color(1).paint(&self.name), line);
                    self.process.output_line(line);
                }
                Err(error) => eprintln!("Error in log output: {}", error),
            }
        }

        self.process.process_died();
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

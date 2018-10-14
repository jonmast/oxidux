use ansi_term::Color;
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
    pub fn for_pty(pty: tokio_pty_process::AsyncPtyMaster, name: String, index: u16) {
        let stream = Framed::new(pty, LinesCodec::new());

        let name = pick_color(index).paint(name).to_string();

        let output = Output { name, stream };

        output.setup_writer();
    }

    fn setup_writer(self) {
        let name = self.name.clone();
        let printer = self.stream.for_each(move |line| {
            println!("{}: {}", pick_color(1).paint(&name), line);
            Ok(())
        });

        let mapped = printer.map_err(|_| ());

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

use std::cell::RefCell;
use std::process::{Command, Stdio};
use std::rc::Rc;

use hyper::Uri;
use tokio_core::reactor::Handle;
use tokio_process::{Child, CommandExt};
use url::Url;

use config;
use output::Output;

#[derive(Clone)]
pub struct Process {
    inner: Rc<RefCell<Inner>>,
}

struct Inner {
    app_name: String,
    port: u16,
    command: String,
    directory: String,
    process: Option<Child>,
}

impl Process {
    pub fn from_config(app_config: &config::App) -> Self {
        let data = Inner {
            app_name: app_config.name.clone(),
            port: app_config.port,
            command: app_config.command.clone(),
            directory: app_config.directory.clone(),
            process: None,
        };

        Process {
            inner: Rc::new(RefCell::new(data)),
        }
    }

    pub fn url(&self, request_path: &Uri) -> Uri {
        let base_url = Url::parse("http://localhost/").unwrap();

        let mut destination_url = base_url
            .join(request_path.as_ref())
            .expect("Invalid request URL");

        destination_url.set_port(Some(self.port())).unwrap();

        println!("Starting request to backend {}", destination_url);

        destination_url.as_str().parse().unwrap()
    }

    pub fn app_name(&self) -> String {
        self.inner.borrow().app_name.clone()
    }

    pub fn start(&self, handle: &Handle) {
        let mut child_process = self.build_command()
            .spawn_async(&handle.clone())
            .expect("Failed to start app process");

        let stdout = child_process.stdout().take().unwrap();
        let app_name = self.app_name();
        let output = Output::new(app_name, stdout);

        output.setup_writer(&handle);
        self.set_child(child_process);
    }

    fn build_command(&self) -> Command {
        let full_command = format!(
            "cd {directory}; {command}",
            directory = self.directory(),
            command = self.command(),
        );
        println!("Starting command {}", full_command);

        let shell = "/bin/sh";

        let mut cmd = Command::new(shell);

        cmd.env("PORT", self.port().to_string())
            .arg("-c")
            .arg(full_command)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        cmd
    }

    fn port(&self) -> u16 {
        self.inner.borrow().port
    }

    fn directory(&self) -> String {
        self.inner.borrow().directory.clone()
    }

    fn command(&self) -> String {
        self.inner.borrow().command.clone()
    }

    fn set_child(&self, process: Child) {
        self.inner.borrow_mut().process = Some(process);
    }
}

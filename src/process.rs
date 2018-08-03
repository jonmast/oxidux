use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use futures::Future;
use hyper::Uri;
use tokio;
use tokio_process::CommandExt;
use url::Url;

use config;
use output::Output;

#[derive(Clone)]
pub struct Process {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    app_name: String,
    port: u16,
    command: String,
    directory: String,
}

impl Process {
    pub fn from_config(app_config: &config::App) -> Self {
        let data = Inner {
            app_name: app_config.name.clone(),
            port: app_config.port,
            command: app_config.command.clone(),
            directory: app_config.directory.clone(),
        };

        Process {
            inner: Arc::new(Mutex::new(data)),
        }
    }

    pub fn url(&self, request_path: &Uri) -> Uri {
        let base_url = Url::parse("http://localhost/").unwrap();

        let mut destination_url = base_url
            .join(request_path.path_and_query().unwrap().as_str())
            .expect("Invalid request URL");

        destination_url.set_port(Some(self.port())).unwrap();

        println!("Starting request to backend {}", destination_url);

        destination_url.as_str().parse().unwrap()
    }

    fn inner(&self) -> MutexGuard<Inner> {
        self.inner.lock().unwrap()
    }

    pub fn app_name(&self) -> String {
        self.inner().app_name.clone()
    }

    pub fn start(&self) {
        let mut child_process = self
            .build_command()
            .spawn_async()
            .expect("Failed to start app process");

        let stdout = child_process.stdout().take().unwrap();
        let app_name = self.app_name();
        let output = Output::new(app_name.clone(), stdout);

        let child_future = child_process
            .map(move |status| println!("Process {} exited with {}", app_name, status))
            .map_err(|e| panic!("failed to wait for exit: {}", e));

        output.setup_writer();

        tokio::spawn(child_future);
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
        self.inner().port
    }

    fn directory(&self) -> String {
        self.inner().directory.clone()
    }

    fn command(&self) -> String {
        self.inner().command.clone()
    }
}

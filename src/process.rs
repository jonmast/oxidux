use std::cell::RefCell;
use std::rc::Rc;
use std::process::{Child, Command};

use hyper::Uri;
use url::Url;

use config;

#[derive(Clone)]
pub struct Process {
    inner: Rc<RefCell<Inner>>,
}

struct Inner {
    app_name: String,
    port: u16,
    command: String,
    process: Option<Child>,
}

impl Process {
    pub fn from_config(app_config: &config::App) -> Self {
        let data = Inner {
            app_name: app_config.name.clone(),
            port: app_config.port,
            command: app_config.command.clone(),
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

    // TODO: use tokio_process to run commands
    pub fn start(&self) {
        let shell = "/bin/sh";
        let child_process = Command::new(shell)
            .arg("-c")
            .env("PORT", self.port().to_string())
            .arg(self.command())
            .spawn()
            .expect("Failed to start app process");

        self.set_child(child_process);
    }

    fn port(&self) -> u16 {
        self.inner.borrow().port
    }

    fn command(&self) -> String {
        self.inner.borrow().command.clone()
    }

    fn set_child(&self, process: Child) {
        self.inner.borrow_mut().process = Some(process);
    }
}

use hyper::Uri;
use url::Url;

pub struct Process {
    pub app_name: String,
    port: u16,
}

impl Process {
    pub fn new(app_name: String, port: u16) -> Process {
        Process { app_name, port }
    }

    pub fn url(&self, request_path: &Uri) -> Uri {
        let base_url = Url::parse("http://localhost/").unwrap();

        let mut destination_url = base_url
            .join(request_path.as_ref())
            .expect("Invalid request URL");

        destination_url.set_port(Some(self.port)).unwrap();

        println!("Starting request to backend {}", destination_url);

        destination_url.as_str().parse().unwrap()
    }
}

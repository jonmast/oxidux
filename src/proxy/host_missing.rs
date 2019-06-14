use crate::process_manager::ProcessManager;
use hyper::{Body, Response};

const PRELUDE: &str = "
<!doctype html>
<html>
    <head>
        <title>App not found | Oxidux</title>
        <style>
            h1 {
                font-size: 1.5em;
            }
            .applist th, .applist td {
                text-align: left;
                padding: 5px;
            }
        </style>
    </head>
    <body>
";

const POSTLUDE: &str = "
    </body>
</html>
";

pub fn missing_host_response(app_name: &str, process_manager: &ProcessManager) -> Response<Body> {
    let mut html = String::new();

    html.push_str(PRELUDE);
    html.push_str(&format!(
        "<h1>Couldn't find app {}, did you mean one of these?</h1>",
        app_name
    ));
    html.push_str(&process_list(process_manager));
    html.push_str(POSTLUDE);

    let body = Body::from(html);

    Response::new(body)
}

const TABLE_HEADER: &str = "
<table class=\"applist\">
    <thead>
        <tr>
            <th>App</th>
            <th>Status</th>
        </tr>
    </thead>
";

// TODO: this should be configurable
const TLD: &str = ".test";

fn process_list(process_manager: &ProcessManager) -> String {
    let mut table = String::new();

    table.push_str(TABLE_HEADER);

    for app in process_manager.apps.iter() {
        let status = if app.is_running() {
            "Running"
        } else {
            "Stopped"
        };

        table.push_str(&format!(
            "<tr><td><a href=\"http://{}{}\">{}</a></td><td>{}</td></tr>",
            app.name(),
            TLD,
            app.name(),
            status
        ));
    }

    table.push_str("</table>");

    table
}

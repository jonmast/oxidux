use hyper::{Body, Response};

const RESTART_RESPONSE: &str = "
<!doctype html>
<html>
    <head>
        <title>App not running, trying to start it | Oxidux</title>
        <style>
            h1 {
                font-size: 1.5em;
            }
        </style>
    </head>
    <body>
        <h1>App doesn't seem to be running, trying to start it now.</h1>
        <p>Refreshing in <span id='time'>5</span> seconds</p>

        <script>
            var timeEl = document.getElementById('time');
            var seconds = parseInt(timeEl.textContent);

            var intervalId = setInterval(function() {
                seconds -= 1;
                timeEl.innerText = seconds;

                if (seconds < 1) {
                    location.reload();
                    clearInterval(intervalId);
                }
            }, 1000);
        </script>
    </body>
</html>
";

pub fn autostart_response() -> Response<Body> {
    let body = Body::from(RESTART_RESPONSE);
    Response::new(body)
}

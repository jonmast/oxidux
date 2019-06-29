Oxidux
======

Proxy server and process manager for developing web apps inspired by
[invoker](http://invoker.codemancers.com/) and
[overmind](https://github.com/DarthSim/overmind).


**Warning** this is still very much a work in progress, there are a lot of rough
edges.

## Installation

Download the [latest release](https://github.com/jonmast/oxidux/releases/latest)
for your platform and place it in your PATH.

You'll also need:
- A server (like Apache) running on port 80 to proxy to oxidux. Support for
  binding directly to port 80 may be added in the future.
  Example config for Apache:
  ```apache
  <VirtualHost *:80>
    ServerName oxidux.test
    ServerAlias *.test
    ProxyPass "/"  "http://localhost:8080/"
    ProxyPreserveHost On
    ProxyTimeout 600
    ErrorDocument 503 "Oxidux is not running :("
  </VirtualHost>
  ```
- DNS resolution for the `.test` TLD.
  - For Linux the [dev-tld-resolver](https://github.com/puma/dev-tld-resolver)
    tool is recommended, add `test` to the `DEV_TLD_DOMAINS` environment
    variable to enable support for `.test` domains.
  - For MacOS, support is planned for hooking into the native DNS resolver
    system, but not implemented at this time. Use one of the following for now:
    - Add each `app_name.test` to the `/etc/hosts` file
    - Set up a local DNS server (`dnsmasq` or similar) and configure it to
      resolve `.test` domains to localhost.

## Configuration
```toml
# apps.toml

[general]
# The proxy server will run on this port. This is intended to be used behind
# Apache or Nginx to make the app accessible on port 80.
proxy_port = 8080

[[apps]]
# Name is used when proxying requests, this app will be available at app.test
name = "first-app"
# Root directory of app
directory = "/path/to/app/"
# Commands to start app processes
# dynamically generated port is passed in as an environment variable
commands = { app = "scripts/server -p $PORT" }

# Another app, with different config options
[[apps]]
name = "second-app"
directory = "/path/to/app/"
# A Procfile can be used instead of specifying the commands
procfile = true
# If the app cannot run on a dynamic port it can be set explicitly here
port = 3000
```

## Usage

### Start the server
```bash
oxidux server apps.toml
```

Note that app processes will not be started immediately, they are automatically
started when a network request comes in for them.

### Restart a process
From the app directory, run
```bash
oxidux restart web
```

The terminal will be connected to the Tmux session for that process.

### Connect to process session
From the app directory, run
```bash
oxidux connect web
```

Connects to the Tmux session for a given process. If the process name is omitted
the first process for the app will be used.

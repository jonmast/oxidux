Oxidux
======

Proxy server and process manager for developing web apps inspired by
[invoker](http://invoker.codemancers.com/) and
[overmind](https://github.com/DarthSim/overmind).


**Warning** this is still very much a work in progress, there are a lot of rough
edges.

## Installation

**HomeBrew or LinuxBrew**:
```sh
brew tap jonmast/oxidux https://github.com/jonmast/oxidux.git
brew install oxidux
```

**Manual**: Download the [latest
release](https://github.com/jonmast/oxidux/releases/latest) for your platform
and place it in your PATH.

**Note**: Windows isn't supported at this time, but I'm happy to assist if
someone wants to work on porting it.

You'll also need:
- Tmux - all apps are run within a tmux session.

## Setup
### Linux

#### DNS Resolution

The [dev-tld-resolver](https://github.com/puma/dev-tld-resolver) tool is
recommended for resolving `*.test` domains to localhost. You'll need to add
`test` to the `DEV_TLD_DOMAINS` environment variable to enable support for
`.test` domains.

#### Service management

Oxidux can be run manually from the terminal, but using SystemD socket
activation is recommended. See example [socket](examples/oxidux.socket) and
[service](examples/oxidux.service) files.

These files should be added to the `/etc/systemd/system/` directory and
enabled with the following commands:
```bash
sudo systemctl daemon-reload
sudo systemctl enable oxidux.socket
sudo systemctl start oxidux.socket
```


### MacOS
#### DNS Resolution

Oxidux has a builtin DNS resolver. Add the following config to
`/etc/resolver/test`:
```
nameserver 127.0.0.1
port 6153
```

#### Service management

Starting via Launchd is recommended. See example [plist
file](examples/oxidux.plist).

The plist file should be added to `~/Library/LaunchAgents/` and loaded with the
following command:
```bash
launchctl load ~/Library/LaunchAgents/oxidux.plist
```

## Configuration
```toml
# config.toml

[general]
# The proxy server will run on this port. Ignored if socket activation is used.
proxy_port = 80
# DNS server port for MacOS. Also ignored with socket activation.
dns_port = 6153
# TLD for apps. Defaults to "test".
domain = "test"
```

### App configuration

Each app should have a config file in `~/.oxidux/apps`. Example:
```toml
# ~/.oxidux/apps/my-app.toml

# Unique identifer and domain for app (this will be available at "my-app.test")
name="my-app"
# App root directory
directory = "/path/to/app/"
# Commands to start app processes
# dynamically generated port is passed in as an environment variable
commands = { web = "scripts/server -p $PORT", worker = "scripts/worker" }
# Alternatively, load commands from Procfile on app directory
procfile = true
# Alternate domains for app
aliases = ["othername", "yetanother"]
```

## Usage

### Restart a process
From the app directory, run
```bash
oxidux restart     # Restart all processes for app
# Or
oxidux restart web # Restart just the process named "web"
```

The terminal will be connected to the Tmux session for that process.

### Connect to process session
From the app directory, run
```bash
oxidux connect web
```

Connects to the Tmux session for a given process. If the process name is omitted
the first process for the app will be used.

## License
Licensed under GPL version 3 or later, see [LICENSE](LICENSE.md).

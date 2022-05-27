<h3 align="center">
	<br>
	taurus
	<br>
</h3>

<p align="center">the functional backend utility</p>

<p align="center">
	<a href="./LICENSE"><img src="https://img.shields.io/badge/license-GPL%20v3.0-blue.svg"></a>
</p>

Taurus is a utility to administer backups with future integration with rsync. Many other features are either included or planned such as chat bridges using rcon and/or parsing the pipe output from programs/games. 

This bot is meant to be interacted with through a 'frontend' using a websocket. A javascript example of this is included in [js](./js/frontend.js) folder. Future additions will allow copies of itself to communicate with each other across servers.

Doing this has a few effects, primarily, it isolates a whole set of issues to the code that is running in the front. Stability like this is important with things that handle file system operations such as backups, as this is why I've split like this. 

This has similar use cases to [this](https://github.com/NotCreative21/hypnos_core), however I feel it is far more efficient.

#### installation

it is recommended you have [cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) installed

ensure that you have `tar`, `tmux`, and `git`; this was only tested on linux, however, any *nix based system should support it, and wsl2 for windows should also function fine. 

```
$ wget https://github.com/NotCreative21/taurus/blob/master/install.sh -sSf | sh
```

* the lld linker can be used to improve compiling times

The required configuration files will be generated on the initial run, additional optional configs can be filled out for more features. 

Websocket command info: 

|Command | arguments | response | description |
|--------|-----------|----------|-------------|
|MSG     | message to send | None | send a chat message to any session labeled "game" |
|CMD     | <SESSION_NAME> command | None | send a command to a certain session, can be shell or in game command |
|RESTART | None | restarting... or failed to execute restart script| executes restart script|
|SHELL | <command> | None | execute a shell command |
|HEARTBEAT| None | true or false | determines if the system has high ram usage, storage usage, etc. |
|CHECK| None | string of info about system | shows the ram usage, cpu usage, storage usage of the server etc. |
|PING| None | PONG timestamp | returns unix timestamp in ms of system time |

#### current features
* interacted with through a websocket
* unified chat bridge between minecraft, discord, and other games
* async code base
* execute in-game commands, shell commands, etc, via discord
* server monitor, checks server health and warns if there are issues
* backup manager, create, delete, and list backups from discord
* backup scheduler, create backups on intervals

#### currently under development
* recompiling system
* upload updated world file or regions to an smp copy
* region backup system, save/load backups
* event handling for talking through the webserver

#### future features
* chest searcher per a region
* scoreboard comparison from two different dates
* future phone app for push notifications

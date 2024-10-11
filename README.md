# A meager IRC client

This TUI client can currently connect to one or more servers, join channels, send and receive messages, and disconnect. That's about it.

The design is inspired—or pretty much lifted—from that of the Tiny IRC client. See [here](https://github.com/osa1/tiny).

## Usage

Run with `cargo run`. Tabs are displayed on top, the input bar is at the bottom.

Commands:

- `/connect <server>` - Connect to a server and open up a new server tab.
- `/join <channel>` - Join a channel on the server to which the tab belongs.
- `/quit <message>` — Quit the current tab's server with the given message.

`TAB` switches between tabs.

Default nick/user/real name are hardcoded, but can be overridden with the environment variables `IRC_NICK`, `IRC_USER`, and `IRC_REAL`.

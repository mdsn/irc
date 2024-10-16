use crate::protocol::{parse_msg, MsgTarget, Prefix, ServCmd, ServMsg};
use crate::ui::UI;
use tokio::io;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{Receiver, Sender};

#[derive(Debug)]
pub enum Event {
    Msg { msg: ServMsg },
    Disconnected,
}

#[derive(Debug)]
pub struct ServInfo {
    pub addr: String,
    pub port: u16,
    pub nick: String,
    pub user: String,
    pub real: String,
}

impl ServInfo {
    pub fn name(&self) -> &str {
        &self.addr
    }
}

pub struct Client {
    pub name: String,
    pub cur_nick: String,
    cmd_tx: Sender<String>,
}

impl Client {
    pub fn new(serv_info: ServInfo) -> (Self, Receiver<Event>, Receiver<String>) {
        connect(serv_info)
    }

    fn send(&self, msg: &str) {
        self.cmd_tx
            .try_send(msg.to_string())
            .expect("failed to send message");
    }

    pub fn quit(&self, msg: &str) {
        self.send(&format!("QUIT :{}\r\n", msg));
    }

    pub fn join(&self, chan: &str) {
        self.send(&format!("JOIN {}\r\n", chan));
    }

    pub fn nick(&mut self, nick: &str) {
        self.send(&format!("NICK {}\r\n", nick));
        self.cur_nick = nick.to_string();
    }

    pub fn privmsg(&self, target: &str, msg: &str) {
        self.send(&format!("PRIVMSG {} :{}\r\n", target, msg));
    }
}

fn connect(serv_info: ServInfo) -> (Client, Receiver<Event>, Receiver<String>) {
    // Channel for messages from the server.
    let (ev_tx, ev_rx) = tokio::sync::mpsc::channel(100);
    // Channel for commands from the app.
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(100);
    // Channel to output all network activity as debug messages.
    let (dbg_tx, dbg_rx) = tokio::sync::mpsc::channel(100);

    let name = serv_info.addr.clone();
    let nick = serv_info.nick.clone();
    tokio::task::spawn_local(network_loop(serv_info, ev_tx, dbg_tx, cmd_rx));

    (
        Client {
            name,
            cur_nick: nick,
            cmd_tx,
        },
        ev_rx,
        dbg_rx,
    )
}

/// Manipulate the client and UI based on network activity.
pub async fn handle_network_events(
    mut ev_rx: Receiver<Event>,
    mut dbg_rx: Receiver<String>,
    tui: UI,
    serv_name: String,
) {
    loop {
        tokio::select! {
            Some(ev) = ev_rx.recv() => {
                match ev {
                    Event::Disconnected => {
                        tui.dbg(&format!("{}: TcpStream disconnected", &serv_name));
                        tui.draw();
                        break;
                    }
                    Event::Msg { msg } => {
                        let ServMsg {
                            prefix,
                            command,
                        } = msg;
                        match command {
                            ServCmd::PrivMsg { target, msg } => {
                                match &prefix {
                                    Some(Prefix::User { nick, .. }) => {
                                        // TODO display @/+/etc
                                        tui.add_msg(&serv_name, target, &format!("<{nick}> {msg}"));
                                    }
                                    Some(Prefix::Server(serv)) => {
                                        tui.add_serv_msg(&serv_name, &format!("[{serv}] {msg}"));
                                    }
                                    _ => tui.dbg(&format!("[{}] PRIVMSG with no prefix {msg:?}", serv_name)),
                                }
                            }
                            ServCmd::Join { chan } => {
                                if let Some(Prefix::User { nick, user, host }) = &prefix {
                                    tui.add_msg(&serv_name, MsgTarget::Chan(chan.clone()),
                                        &format!("{nick} ({user}@{host}) joined {chan}"));
                                }
                            }
                            ServCmd::Part { chan, msg } => {
                                if let Some(Prefix::User { nick, user, host }) = &prefix {
                                    let msg = if msg.is_empty() {
                                        format!("{nick} ({user}@{host}) left {chan}")
                                    } else {
                                        format!("{nick} ({user}@{host}) left {chan} ({msg})")
                                    };
                                    tui.add_msg(&serv_name, MsgTarget::Chan(chan.clone()), &msg);
                                }
                            }
                            ServCmd::Nick { nick } => {
                                // Same message if self or other changes nick.
                                // No channel indication--have to keep track of nicks in each channel.
                                // Print message in relevant channels.
                                // Should solve for self as well, since self is in all channels.
                                if let Some(Prefix::User { nick: old_nick, .. }) = &prefix {
                                    tui.add_msg(&serv_name, MsgTarget::Serv(serv_name.clone()),
                                        &format!("{old_nick} is now known as {nick}"));
                                }
                            }
                            ServCmd::Notice { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::Error { msg } => {
                                tui.add_serv_msg(&serv_name, &msg);
                                // Do not break here--wait for the Event::Disconnected message to
                                // break out of the loop.
                            }
                            ServCmd::RplWelcome { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::RplYourHost { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::RplCreated { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::RplMyInfo { version, umodes, cmodes, cmodes_param } => {
                                tui.add_serv_msg(&serv_name, &format!("{version} {umodes} {cmodes} {cmodes_param}"));
                            }
                            ServCmd::RplISupport { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::RplLuserClient { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::RplLuserOp { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::RplLuserUnknown { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::RplLuserChannels { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::RplLuserMe { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::RplLocalUsers { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::RplGlobalUsers { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::NameReply { sym, chan, nicks } => {
                                let nicks = nicks.join(" ");
                                tui.add_serv_msg(&serv_name, &format!("{sym} {chan} {nicks}"));
                            },
                            ServCmd::EndOfNames { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::MOTDStart { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::Motd { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::MOTDEnd { msg } => tui.add_serv_msg(&serv_name, &msg),
                            ServCmd::DisplayedHost { msg } => tui.add_serv_msg(&serv_name, &msg),
                            _ => tui.dbg(&format!("[{}] unhandled command {command:?}", serv_name)),
                        }
                        tui.draw();
                    }
                }
            }

            Some(msg) = dbg_rx.recv() => {
                tui.dbg(&msg);
                tui.draw();
            }
        }
    }
}

/// Low level communication with the server.
async fn network_loop(
    serv_info: ServInfo,
    ev_tx: Sender<Event>,
    dbg_tx: Sender<String>,
    mut cmd_rx: Receiver<String>,
) {
    let host = format!("{}:{}", serv_info.addr, serv_info.port);
    let stream = TcpStream::connect(host)
        .await
        .expect("failed to connect to server");
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader).lines();

    send(&mut writer, &format!("NICK {}\r\n", serv_info.nick))
        .await
        .expect("network_loop: failed to send NICK");
    send(
        &mut writer,
        &format!("USER {} 0 * :{}\r\n", serv_info.user, serv_info.real),
    )
    .await
    .expect("network_loop: failed to send USER");

    loop {
        tokio::select! {
            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) if line.starts_with("PING") => {
                        let pong = format!("PONG {}\r\n", &line[5..]);
                        send(&mut writer, &pong).await.expect("failed to send PONG");
                    }
                    Ok(Some(line)) => {
                        dbg_tx.send(line.clone()).await.expect("failed to send debug message");

                        let msg = parse_msg(&line);
                        ev_tx
                            .send(Event::Msg { msg })
                            .await
                            .expect("failed to send message");
                    }
                    Ok(None) => {
                        ev_tx.send(Event::Disconnected).await.expect("failed to send message");
                        break;
                    }
                    Err(e) => {
                        eprintln!("error reading from server: {}", e);
                        break;
                    }
                }
            }

            cmd = cmd_rx.recv() => {
                if let Some(cmd) = cmd {
                    send(&mut writer, &cmd).await.expect("failed to send command: {cmd}");
                }
            }
        }
    }
}

async fn send<W>(stream: &mut W, msg: &str) -> io::Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    stream.write_all(msg.as_bytes()).await?;
    Ok(())
}

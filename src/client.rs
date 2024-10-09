use crate::protocol::{parse_msg, MsgTarget, Prefix, ServCmd, ServMsg};
use crate::ui::UI;
use tokio::io;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{Receiver, Sender};

pub struct Event {
    msg: ServMsg,
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

#[derive(Clone)]
pub struct Client {
    pub name: String,
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

    pub fn join(&self, chan: &str) {
        self.send(&format!("JOIN {}\r\n", chan));
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
    tokio::task::spawn_local(network_loop(serv_info, ev_tx, dbg_tx, cmd_rx));

    (Client { name, cmd_tx }, ev_rx, dbg_rx)
}

/// Manipulate the client and UI based on network activity.
pub async fn handle_network_events(
    mut ev_rx: Receiver<Event>,
    mut dbg_rx: Receiver<String>,
    tui: UI,
    client: Client,
) {
    loop {
        tokio::select! {
            Some(Event { msg }) = ev_rx.recv() => {
                match msg {
                    ServMsg {
                        prefix,
                        command,
                        params,
                    } => match command {
                        ServCmd::PrivMsg { target, msg } => {
                            match &prefix {
                                Some(Prefix::User { nick, user, host }) => {
                                    tui.add_msg(&client.name, &prefix, target, &format!("<{nick}> {msg}"));
                                }
                                Some(Prefix::Server(serv)) => {
                                    tui.add_serv_msg(&client.name, &format!("[{serv}] {msg}"));
                                }
                                _ => tui.dbg(&format!("[{}] PRIVMSG with no prefix {msg:?}", client.name)),
                            }
                        }
                        ServCmd::Join { chan } => {
                            if let Some(Prefix::User { nick, user, host }) = &prefix {
                                tui.add_msg(&client.name, &prefix, MsgTarget::Chan(chan.clone()),
                                    &format!("{nick} ({user}@{host}) joined {chan}"));
                            }
                        }
                        ServCmd::Notice { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::RplWelcome { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::RplYourHost { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::RplCreated { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::RplMyInfo { version, umodes, cmodes, cmodes_param } => {
                            tui.add_serv_msg(&client.name, &format!("{version} {umodes} {cmodes} {cmodes_param}"));
                        }
                        ServCmd::RplISupport { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::RplLuserClient { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::RplLuserOp { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::RplLuserUnknown { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::RplLuserChannels { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::RplLuserMe { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::RplLocalUsers { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::RplGlobalUsers { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::NameReply { sym, chan, nicks } => {
                            let nicks = nicks.join(" ");
                            tui.add_serv_msg(&client.name, &format!("{sym} {chan} {nicks}"));
                        },
                        ServCmd::EndOfNames { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::MOTDStart { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::MOTD { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::MOTDEnd { msg } => tui.add_serv_msg(&client.name, &msg),
                        ServCmd::DisplayedHost { msg } => tui.add_serv_msg(&client.name, &msg),
                        _ => tui.dbg(&format!("[{}] unhandled command {command:?}", client.name)),
                    },
                }
                tui.draw();
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
                            .send(Event { msg })
                            .await
                            .expect("failed to send message");
                    }
                    Ok(None) => {
                        eprintln!("server closed connection");
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

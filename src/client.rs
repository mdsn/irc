use crate::ui::UI;
use tokio::io;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{Receiver, Sender};

pub struct Event {
    src: String,
    msg: String,
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
    name: String,
}

impl Client {
    pub fn new(serv_info: ServInfo) -> (Self, Receiver<Event>) {
        connect(serv_info)
    }
}

fn connect(serv_info: ServInfo) -> (Client, Receiver<Event>) {
    let (ev_tx, ev_rx) = tokio::sync::mpsc::channel(100);

    let name = serv_info.addr.clone();
    tokio::task::spawn_local(network_loop(serv_info, ev_tx));

    (Client { name }, ev_rx)
}

/// Manipulate the client and UI based on network activity.
pub async fn handle_network_events(mut ev_rx: Receiver<Event>, mut tui: UI, client: Client) {
    while let Some(Event { src, msg }) = ev_rx.recv().await {
        // For now, send the server messages into that server's tab.
        tui.add_msg(src, msg);
        tui.draw();
    }
}

/// Low level communication with the server.
async fn network_loop(serv_info: ServInfo, ev_tx: Sender<Event>) {
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
        match reader.next_line().await {
            Ok(Some(line)) if line.starts_with("PING") => {
                let pong = format!("PONG {}\r\n", &line[5..]);
                send(&mut writer, &pong).await.expect("failed to send PONG");
            }
            Ok(Some(line)) => {
                ev_tx
                    .send(Event {
                        msg: line,
                        src: serv_info.name().to_string(),
                    })
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
}

async fn send<W>(stream: &mut W, msg: &str) -> io::Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    stream.write_all(msg.as_bytes()).await?;
    Ok(())
}

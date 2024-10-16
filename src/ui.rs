use crate::client::{Client, ServInfo};
use crate::command::Cmd;
use crate::protocol::MsgTarget;
use crate::{client, command, Config};
use crossterm::cursor::MoveTo;
use crossterm::event::KeyCode;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use std::cell::{Ref, RefCell};
use std::collections::VecDeque;
use std::io::Write;
use std::rc::Rc;
use std::{fmt, io};
use tokio::sync::mpsc::Receiver;

pub async fn run(tui: UI, input_rx: Receiver<KeyCode>, clients: Vec<Client>) {
    ui_loop(tui, clients, input_rx).await;
}

async fn ui_loop(tui: UI, mut clients: Vec<Client>, mut input_rx: Receiver<KeyCode>) {
    while let Some(cmd) = input_rx.recv().await {
        match cmd {
            KeyCode::Esc => {
                break;
            }
            KeyCode::Char(c) => {
                tui.push_input(c);
            }
            KeyCode::Enter => {
                tui.commit_input(&mut clients);
            }
            KeyCode::Backspace => {
                tui.pop_input();
            }
            KeyCode::Tab => {
                tui.next_tab();
            }
            _ => {}
        }

        tui.draw();
    }
}

struct InnerUI {
    cur_tab: usize,
    tabs: Vec<Tab>,
}

impl InnerUI {
    fn new() -> Self {
        Self {
            cur_tab: 0,
            tabs: vec![Tab::new(TabKind::Debug)],
        }
    }

    fn dbg(&mut self, msg: &str) {
        self.tabs[0].add_line(msg.to_string());
    }

    fn add_msg(&mut self, serv_name: &str, target: MsgTarget, msg: &str) {
        let tab_id = match &target {
            MsgTarget::Chan(chan) => TabKind::Chan {
                serv: serv_name.to_string(),
                chan: chan.to_string(),
            },
            MsgTarget::User(nick) => TabKind::Query {
                serv: serv_name.to_string(),
                nick: nick.to_string(),
            },
            MsgTarget::Serv(serv) => TabKind::Serv {
                serv: serv.to_string(),
            },
        };

        if let Some(tab) = self.find_tab_mut(&tab_id) {
            tab.add_line(msg.to_string());
        } else {
            self.dbg(&format!("[{serv_name}] No tab found {target:?} ({msg})"));
        }
    }

    fn add_tab(&mut self, id: TabKind) {
        self.tabs.push(Tab::new(id));
    }

    fn change_to_tab(&mut self, id: &TabKind) -> bool {
        if let Some(pos) = self.tab_position(id) {
            self.cur_tab = pos;
            true
        } else {
            false
        }
    }

    fn find_tab_mut(&mut self, id: &TabKind) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|tab| tab.id == *id)
    }

    fn tab_position(&self, id: &TabKind) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.id == *id)
    }

    pub fn next_tab(&mut self) {
        self.cur_tab = (self.cur_tab + 1) % self.tabs.len();
    }

    pub fn push_input(&mut self, c: char) {
        self.tabs[self.cur_tab].input.push(c);
    }

    pub fn pop_input(&mut self) {
        self.tabs[self.cur_tab].input.pop();
    }

    pub fn take_input(&mut self) -> String {
        std::mem::take(&mut self.tabs[self.cur_tab].input)
    }
}

#[derive(Clone)]
pub struct UI {
    inner: Rc<RefCell<InnerUI>>,
    config: Rc<RefCell<Config>>,
}

impl UI {
    pub fn new(config: Rc<RefCell<Config>>) -> Self {
        Self {
            inner: Rc::new(RefCell::new(InnerUI::new())),
            config,
        }
    }

    pub fn dbg(&self, msg: &str) {
        self.inner.borrow_mut().dbg(msg);
    }

    pub fn add_msg(&self, serv_name: &str, target: MsgTarget, msg: &str) {
        self.inner.borrow_mut().add_msg(serv_name, target, msg);
    }

    pub fn add_serv_msg(&self, serv_name: &str, msg: &str) {
        self.add_msg(serv_name, MsgTarget::Serv(serv_name.to_string()), msg);
    }

    pub fn add_tab(&self, id: TabKind) {
        self.inner.borrow_mut().add_tab(id);
    }

    fn current_tab(&self) -> Ref<Tab> {
        let inner = self.inner.borrow();
        Ref::map(inner, |x| &x.tabs[x.cur_tab])
    }

    pub fn next_tab(&self) {
        self.inner.borrow_mut().next_tab();
    }

    pub fn change_to_tab(&self, id: &TabKind) {
        if self.inner.borrow_mut().change_to_tab(id) {
            self.draw();
        } else {
            self.dbg(&format!("change_to_tab: No tab found for {}", id));
        }
    }

    pub fn push_input(&self, c: char) {
        self.inner.borrow_mut().push_input(c);
    }

    pub fn pop_input(&self) {
        self.inner.borrow_mut().pop_input();
    }

    fn take_input(&self) -> String {
        self.inner.borrow_mut().take_input()
    }

    pub fn commit_input(&self, clients: &mut Vec<Client>) {
        let input = self.take_input();
        match command::parse_input(&input) {
            Err(e) => self.dbg(&format!("Command parse error: {e}")),
            Ok(cmd) => match cmd {
                Cmd::Connect(addr) => {
                    self.dbg(&format!("Connecting to {addr}"));
                    let serv_info = ServInfo {
                        addr,
                        port: 6667,
                        nick: self.config.borrow().nick.clone(),
                        user: self.config.borrow().user.clone(),
                        real: self.config.borrow().real.clone(),
                    };
                    self.dbg(&format!("{serv_info:?}"));

                    let serv_name = serv_info.name().to_string();

                    let tab_id = TabKind::Serv {
                        serv: serv_name.clone(),
                    };
                    self.add_tab(tab_id.clone());
                    self.change_to_tab(&tab_id);

                    let (client, ev_rx, dbg_rx) = Client::new(serv_info);
                    tokio::task::spawn_local(client::handle_network_events(
                        ev_rx,
                        dbg_rx,
                        self.clone(),
                        serv_name,
                    ));
                    clients.push(client);
                }
                Cmd::Join(chan) => {
                    let tab_id = self.current_tab().id.clone();
                    match tab_id {
                        TabKind::Serv { serv } => {
                            self.dbg(&format!("Joining {chan} on {serv}"));
                            if let Some(client) = clients.iter().find(|c| c.name == serv) {
                                client.join(&chan);
                                let tab_id = TabKind::Chan { serv, chan };
                                self.add_tab(tab_id.clone());
                                self.change_to_tab(&tab_id);
                            } else {
                                self.dbg(&format!("No client found for server {serv}"));
                            }
                        }
                        _ => {
                            self.dbg("Join command on debug tab");
                        }
                    }
                }
                Cmd::Quit(msg) => {
                    if let Some(client) = self.find_client_for_current_tab(clients) {
                        client.quit(&msg);
                    }
                }
                Cmd::Nick(nick) => {
                    if let Some(client) = self.find_client_for_current_tab_mut(clients) {
                        client.nick(&nick);
                    }
                }
                Cmd::Msg(msg) => {
                    let tab_id = self.current_tab().id.clone();
                    if let Some((serv, msg_target)) = match &tab_id {
                        TabKind::Serv { serv: _ } => {
                            self.dbg(&format!("Message sent on server tab: {msg}"));
                            None
                        }
                        TabKind::Chan { serv, chan } => {
                            self.dbg(&format!("Sending message to {chan} on {serv}: {msg}"));
                            Some((serv, MsgTarget::Chan(chan.clone())))
                        }
                        TabKind::Query { serv, nick } => {
                            self.dbg(&format!("Sending message to {nick} on {serv}: {msg}"));
                            Some((serv, MsgTarget::User(nick.clone())))
                        }
                        _ => {
                            self.dbg("Message command on debug tab");
                            None
                        }
                    } {
                        if let Some(client) = clients.iter().find(|c| c.name == *serv) {
                            // FIXME message formatting sprawled in ui and client modules
                            client.privmsg(msg_target.target(), &msg);
                            let msg = format!("<{}> {msg}", &client.cur_nick);
                            self.add_msg(&client.name, msg_target, &msg);
                        } else {
                            self.dbg(&format!("No client found for server {serv}"));
                        }
                    }
                }
                Cmd::Unsupported { cmd, rest } => {
                    self.dbg(&format!("Unsupported command: {cmd} {rest}"));
                }
            },
        }
    }

    fn find_client_for_current_tab<'a>(&self, clients: &'a [Client]) -> Option<&'a Client> {
        let tab_id = &self.current_tab().id;
        let serv = match tab_id {
            TabKind::Serv { serv } => serv,
            TabKind::Chan { serv, .. } => serv,
            TabKind::Query { serv, .. } => serv,
            _ => return None,
        };
        clients.iter().find(|c| c.name == *serv)
    }

    fn find_client_for_current_tab_mut<'a>(
        &self,
        clients: &'a mut [Client],
    ) -> Option<&'a mut Client> {
        let tab_id = &self.current_tab().id;
        let serv = match tab_id {
            TabKind::Serv { serv } => serv,
            TabKind::Chan { serv, .. } => serv,
            TabKind::Query { serv, .. } => serv,
            _ => return None,
        };
        clients.iter_mut().find(|c| c.name == *serv)
    }

    pub fn draw(&self) {
        let inner = self.inner.borrow();
        // Draw tabs on top
        queue!(io::stdout(), MoveTo(0, 0), Clear(ClearType::CurrentLine),)
            .expect("failed to draw tab");
        for (i, tab) in inner.tabs.iter().enumerate() {
            tab.draw(i == inner.cur_tab);
        }

        // Draw tab content
        let tab = &inner.tabs[inner.cur_tab];
        let (_, rows) = crossterm::terminal::size().expect("failed to get terminal size");

        // Clear initial empty lines if there are fewer lines of text than there are rows
        if tab.lines.len() < rows as usize - 1 {
            for y in 1..=rows - 2 - tab.lines.len() as u16 {
                queue!(io::stdout(), MoveTo(0, y), Clear(ClearType::CurrentLine),)
                    .expect("failed to draw tab content");
            }
        }

        // Draw lines of text
        let mut y = rows - 2;
        let messages = tab.lines.iter().rev().take(rows as usize - 1).peekable();
        for message in messages {
            queue!(
                io::stdout(),
                MoveTo(0, y),
                Clear(ClearType::CurrentLine),
                MoveTo(0, y),
                Print(message),
            )
            .expect("failed to draw tab content");
            if y == 1 {
                break;
            }
            y -= 1;
        }

        // Draw input buffer
        queue!(
            io::stdout(),
            MoveTo(0, rows - 1),
            Clear(ClearType::CurrentLine),
            MoveTo(0, rows - 1),
            Print(&tab.input),
        )
        .expect("failed to draw input buffer");

        io::stdout().flush().expect("failed to flush stdout");
    }
}

struct Tab {
    /// Identifier for the tab
    id: TabKind,
    /// Content of the input buffer associated with this tab
    input: String,
    /// Lines of output associated with this tab
    lines: VecDeque<String>,
}

impl Tab {
    pub fn new(id: TabKind) -> Self {
        Self {
            id,
            input: String::with_capacity(256),
            lines: VecDeque::new(),
        }
    }

    pub fn add_line(&mut self, line: String) {
        self.lines.push_back(line);
    }

    pub fn draw(&self, is_active: bool) {
        queue!(
            io::stdout(),
            Print(if is_active {
                format!("[{}]", &self.id)
            } else {
                format!(" {} ", &self.id)
            })
        )
        .expect("failed to draw tab");
    }
}

#[derive(Clone, PartialEq)]
pub enum TabKind {
    Debug,
    Serv { serv: String },
    Chan { serv: String, chan: String },
    Query { serv: String, nick: String },
}

impl fmt::Display for TabKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Debug => write!(f, "__debug__"),
            Self::Serv { serv } => write!(f, "{}", serv),
            Self::Chan { chan, .. } => write!(f, "{}", chan),
            Self::Query { nick, .. } => write!(f, "{}", nick),
        }
    }
}

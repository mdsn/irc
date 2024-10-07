use crate::client::{Client, ServInfo};
use crate::command::Cmd;
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

    fn add_msg(&mut self, src: String, msg: String) {
        if let Some(tab) = self.find_tab_mut(&src) {
            tab.add_line(format!("<{}> {}", src, msg));
        } else {
            self.dbg(&format!("No tab found for message from {}: {}", src, msg));
        }
    }

    fn add_tab(&mut self, id: TabKind) {
        self.tabs.push(Tab::new(id));
    }

    fn change_to_tab(&mut self, name: &str) -> bool {
        if let Some(pos) = self.tab_position(name) {
            self.cur_tab = pos;
            true
        } else {
            false
        }
    }

    fn find_tab_mut(&mut self, name: &str) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|tab| tab.id == name)
    }

    fn tab_position(&self, name: &str) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.id == name)
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

    pub fn add_msg(&self, src: String, msg: String) {
        self.inner.borrow_mut().add_msg(src, msg);
    }

    pub fn add_serv_tab(&self, name: String) {
        self.dbg(&format!("Adding server tab: {}", name));
        self.inner
            .borrow_mut()
            .add_tab(TabKind::Serv { serv: name });
    }

    fn current_tab(&self) -> Ref<Tab> {
        let inner = self.inner.borrow();
        Ref::map(inner, |x| &x.tabs[x.cur_tab])
    }

    pub fn next_tab(&self) {
        self.inner.borrow_mut().next_tab();
    }

    pub fn change_to_tab(&self, name: &str) {
        if self.inner.borrow_mut().change_to_tab(name) {
            self.draw();
        } else {
            self.dbg(&format!("change_to_tab: No tab found for {}", name));
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

                self.add_serv_tab(serv_info.name().to_string());
                self.change_to_tab(serv_info.name());

                let (client, ev_rx) = Client::new(serv_info);
                tokio::task::spawn_local(client::handle_network_events(
                    ev_rx,
                    self.clone(),
                    client.clone(),
                ));
                clients.push(client);
            }
            Cmd::Join(chan) => {
                // Get the server name from the current tab
                self.dbg(&format!("Joining {}", chan));
                match &self.current_tab().id {
                    TabKind::Serv { serv } => {
                        self.dbg(&format!("Joining {chan} on {serv}"));
                    }
                    _ => {
                        self.dbg("Join command on debug tab");
                    }
                }
            }
            Cmd::Quit(msg) => {
                self.dbg(&format!("Quitting: {}", msg));
            }
            Cmd::Msg(msg) => {
                self.dbg(&format!("Sending message: {}", msg));
            }
        }
    }

    pub fn draw(&self) {
        let inner = self.inner.borrow();
        // Draw tabs on top
        queue!(io::stdout(), MoveTo(0, 0), Clear(ClearType::CurrentLine),)
            .expect("failed to draw tab");
        for (i, tab) in inner.tabs.iter().enumerate() {
            tab.draw(i, i == inner.cur_tab);
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
        let mut messages = tab.lines.iter().rev().take(rows as usize - 1).peekable();
        while let Some(message) = messages.next() {
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
    /// Width of the display name
    width: u16,
    /// Content of the input buffer associated with this tab
    input: String,
    /// Lines of output associated with this tab
    lines: VecDeque<String>,
}

impl Tab {
    pub fn new(id: TabKind) -> Self {
        let width = id.display_width();
        Self {
            id,
            width,
            input: String::with_capacity(256),
            lines: VecDeque::new(),
        }
    }

    pub fn add_line(&mut self, line: String) {
        self.lines.push_back(line);
    }

    pub fn draw(&self, index: usize, is_active: bool) {
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

enum TabKind {
    Debug,
    Serv { serv: String },
    Chan { serv: String, chan: String },
    Query { serv: String, nick: String },
}

impl TabKind {
    fn display_width(&self) -> u16 {
        match self {
            Self::Debug => 9u16, // "__debug__"
            Self::Serv { serv } => serv.len() as u16,
            Self::Chan { chan, .. } => chan.len() as u16,
            Self::Query { nick, .. } => nick.len() as u16,
        }
    }
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

// XXX This will cause equally named tabs in different servers to be selected :|
impl PartialEq<&str> for TabKind {
    fn eq(&self, other: &&str) -> bool {
        match self {
            Self::Debug => *other == "__debug__",
            Self::Serv { serv } => serv == *other,
            Self::Chan { chan, .. } => chan == *other,
            Self::Query { nick, .. } => nick == *other,
        }
    }
}

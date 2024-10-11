use crate::ui::UI;
use anyhow::Result;
use std::cell::RefCell;
use std::rc::Rc;

mod client;
mod command;
mod input;
mod protocol;
mod terminal;
mod ui;

fn main() -> Result<()> {
    terminal::setup()?;

    let config = Rc::new(RefCell::new(Config::from_env()));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let local_set = tokio::task::LocalSet::new();

    local_set.block_on(&runtime, async {
        let input_rx = input::listen();

        let tui = UI::new(config.clone());
        tui.draw();

        let clients = vec![];
        ui::run(tui, input_rx, clients).await;
    });

    terminal::restore()?;
    Ok(())
}

struct Config {
    pub nick: String,
    pub user: String,
    pub real: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            nick: "meager-irc-client".to_string(),
            user: "guest".to_string(),
            real: "Meager".to_string(),
        }
    }
}

impl Config {
    fn from_env() -> Self {
        let mut config = Config::default();
        std::env::var("IRC_NICK")
            .map(|nick| config.nick = nick)
            .ok();
        std::env::var("IRC_USER")
            .map(|user| config.user = user)
            .ok();
        std::env::var("IRC_REAL")
            .map(|real| config.real = real)
            .ok();
        config
    }
}

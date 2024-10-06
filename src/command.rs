/// Input parsing

pub enum Cmd {
    Connect(String),
    Join(String),
    Quit(String),
    Msg(String),
}

pub fn parse_input(input: &str) -> Cmd {
    let mut parts = input.split_whitespace();
    match parts.next() {
        Some("/connect") => Cmd::Connect(parts.collect()),
        Some("/join") => Cmd::Join(parts.collect()),
        Some("/quit") => Cmd::Quit(parts.collect()),
        _ => Cmd::Msg(input.to_string()),
    }
}

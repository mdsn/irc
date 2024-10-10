/// Input parsing

#[derive(Debug, PartialEq)]
pub enum Cmd {
    Connect(String),
    Join(String),
    Quit(String),
    Msg(String),
    Unsupported { cmd: String, rest: String },
}

fn make_cmd(cmd: &str, rest: &str) -> Result<Cmd, &'static str> {
    match cmd {
        "/connect" => (!rest.is_empty())
            .then_some(Cmd::Connect(rest.to_string()))
            .ok_or("No server address provided"),
        "/join" => (!rest.is_empty())
            .then_some(Cmd::Join(rest.to_string()))
            .ok_or("No channel name provided"),
        "/quit" => Ok(Cmd::Quit(rest.to_string())),
        _ => Ok(Cmd::Unsupported {
            cmd: cmd.to_string(),
            rest: rest.to_string(),
        }),
    }
}

pub fn parse_input(input: &str) -> Result<Cmd, &'static str> {
    if !input.starts_with('/') {
        Ok(Cmd::Msg(input.to_string()))
    } else {
        if let Some(ix) = input.find(' ') {
            let (cmd, rest) = input.split_at(ix);
            make_cmd(cmd, &rest[1..].trim())
        } else {
            make_cmd(input, "")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_connect() {
        let input = "/connect irc.freenode.net";
        let cmd = parse_input(input);
        assert_eq!(cmd, Ok(Cmd::Connect("irc.freenode.net".to_string())));
    }

    #[test]
    fn test_parse_connect_err() {
        let input = "/connect";
        let cmd = parse_input(input);
        assert_eq!(cmd, Err("No server address provided"));
    }

    #[test]
    fn test_parse_join() {
        let input = "/join #bobcat";
        let cmd = parse_input(input);
        assert_eq!(cmd, Ok(Cmd::Join("#bobcat".to_string())));
    }

    #[test]
    fn test_parse_join_err() {
        let input = "/join";
        let cmd = parse_input(input);
        assert_eq!(cmd, Err("No channel name provided"));
    }

    #[test]
    fn test_parse_quit() {
        let input = "/quit well I'm out of here bye!!";
        let cmd = parse_input(input);
        assert_eq!(cmd, Ok(Cmd::Quit("well I'm out of here bye!!".to_string())));
    }

    #[test]
    fn test_parse_quit_no_msg() {
        let input = "/quit";
        let cmd = parse_input(input);
        assert_eq!(cmd, Ok(Cmd::Quit("".to_string())));
    }

    #[test]
    fn test_unsupported() {
        let input = "/rhubarb jsjjsjs args";
        let cmd = parse_input(input);
        assert_eq!(
            cmd,
            Ok(Cmd::Unsupported {
                cmd: "/rhubarb".to_string(),
                rest: "jsjjsjs args".to_string(),
            })
        );
    }
}

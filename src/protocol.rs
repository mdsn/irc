/// Parsing IRC messages

// From the RFC:
//     Each IRC message may consist of up to three main parts: the prefix
//     (OPTIONAL), the command, and the command parameters (maximum of
//     fifteen (15)).  The prefix, command, and all parameters are separated
//     by one ASCII space character (0x20) each.
//
//     The presence of a prefix is indicated with a single leading ASCII
//     colon character (':', 0x3b), which MUST be the first character of the
//     message itself.

#[derive(Debug, PartialEq)]
pub enum ServCmd {
    Join,
    PrivMsg { target: MsgTarget, msg: String },
    Part,
    NameReply,  // 353
    EndOfNames, // 366
    Unknown(String),
}

#[derive(Debug, PartialEq)]
enum MsgTarget {
    Chan(String),
    User(String),
}

#[derive(Debug, PartialEq)]
pub enum Prefix {
    Server(String),
    User {
        nick: String,
        user: String,
        host: String,
    },
}

#[derive(Debug, PartialEq)]
pub struct ServMsg {
    pub prefix: Option<Prefix>,
    pub command: ServCmd,
    pub params: Vec<String>,
}

pub fn parse_msg(msg: &str) -> ServMsg {
    let mut parts = msg.split_whitespace();
    let prefix = if parts.clone().next().unwrap().starts_with(':') {
        let p = parts.next().unwrap();
        Some(parse_prefix(&p[1..]))
    } else {
        None
    };

    let cmd = parts.next().unwrap();

    let mut params: Vec<String> = vec![];
    let mut rest = parts.collect::<Vec<&str>>();
    if let Some(trailing_index) = rest.iter().position(|&x| x.starts_with(':')) {
        params.extend(rest[..trailing_index].iter().map(|&x| x.to_string()));
        params.push(rest.split_off(trailing_index).join(" "));
    }

    let (command, params) = parse_cmd(cmd, params);

    ServMsg {
        prefix,
        command,
        params,
    }
}

fn parse_cmd(cmd: &str, params: Vec<String>) -> (ServCmd, Vec<String>) {
    match cmd {
        "JOIN" => (ServCmd::Join, params),
        "PRIVMSG" => {
            let target = if params[0].starts_with('#') {
                MsgTarget::Chan(params[0].to_string())
            } else {
                MsgTarget::User(params[0].to_string())
            };
            (
                ServCmd::PrivMsg {
                    target,
                    msg: params[1][1..].to_string(),
                },
                vec![],
            )
        }
        "PART" => (ServCmd::Part, params),
        "353" => (ServCmd::NameReply, params),
        "366" => (ServCmd::EndOfNames, params),
        _ => (ServCmd::Unknown(cmd.to_string()), params),
    }
}

fn parse_prefix(prefix: &str) -> Prefix {
    if prefix.contains('!') && prefix.contains('@') {
        let mut parts = prefix.splitn(2, '!');
        let nick = parts.next().unwrap().to_string();
        let rest = parts.next().unwrap();
        let mut parts = rest.splitn(2, '@');
        let user = parts.next().unwrap().to_string();
        let host = parts.next().unwrap().to_string();
        Prefix::User { nick, user, host }
    } else {
        Prefix::Server(prefix.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_prefix_serv() {
        let prefix = "*.freenode.net";
        let parsed = parse_prefix(prefix);
        assert_eq!(parsed, Prefix::Server("*.freenode.net".to_string()));
    }

    #[test]
    fn test_parse_prefix_user() {
        let prefix = "MrNickname!~MrUser@freenode-o6n.182.alt94q.IP";
        let parsed = parse_prefix(prefix);
        assert_eq!(
            parsed,
            Prefix::User {
                nick: "MrNickname".to_string(),
                user: "~MrUser".to_string(),
                host: "freenode-o6n.182.alt94q.IP".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_353() {
        let msg = ":*.freenode.net 353 my-nickname = #bobcat :@my-nickname";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(serv_msg.command, ServCmd::NameReply);
        assert_eq!(
            serv_msg.params,
            vec![
                "my-nickname".to_string(),
                "=".to_string(),
                "#bobcat".to_string(),
                ":@my-nickname".to_string()
            ]
        );
    }

    #[test]
    fn test_parse_366() {
        let msg = ":*.freenode.net 366 my-nickname #bobcat :End of /NAMES list.";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(serv_msg.command, ServCmd::EndOfNames);
        assert_eq!(
            serv_msg.params,
            vec![
                "my-nickname".to_string(),
                "#bobcat".to_string(),
                ":End of /NAMES list.".to_string()
            ]
        );
    }

    #[test]
    fn test_parse_join() {
        let msg = ":MrNickname!~MrUser@freenode-o6n.182.alt94q.IP JOIN :#bobcat";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::User {
                nick: "MrNickname".to_string(),
                user: "~MrUser".to_string(),
                host: "freenode-o6n.182.alt94q.IP".to_string(),
            })
        );
        assert_eq!(serv_msg.command, ServCmd::Join);
        assert_eq!(serv_msg.params, vec![":#bobcat".to_string()]);
    }

    #[test]
    fn test_parse_privmsg() {
        let msg = ":MrNickname!~MrUser@freenode-o6n.182.alt94q.IP PRIVMSG #bobcat :this is a wug!!";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::User {
                nick: "MrNickname".to_string(),
                user: "~MrUser".to_string(),
                host: "freenode-o6n.182.alt94q.IP".to_string(),
            })
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::PrivMsg {
                target: MsgTarget::Chan("#bobcat".to_string()),
                msg: "this is a wug!!".to_string(),
            }
        );
        assert!(serv_msg.params.is_empty());
    }

    #[test]
    fn test_parse_part() {
        let msg = ":MrNickname!~MrUser@freenode-o6n.182.alt94q.IP PART :#bobcat";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::User {
                nick: "MrNickname".to_string(),
                user: "~MrUser".to_string(),
                host: "freenode-o6n.182.alt94q.IP".to_string(),
            })
        );
        assert_eq!(serv_msg.command, ServCmd::Part);
        assert_eq!(serv_msg.params, vec![":#bobcat".to_string()]);
    }
}

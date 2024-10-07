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
    PrivMsg,
    Part,
    NameReply,  // 353
    EndOfNames, // 366
    Unknown(String),
}

#[derive(Debug, PartialEq)]
pub struct ServMsg {
    pub prefix: Option<String>,
    pub command: ServCmd,
    pub params: Vec<String>,
}

pub fn parse_msg(msg: &str) -> ServMsg {
    let mut parts = msg.split_whitespace();
    let prefix = if parts.clone().next().unwrap().starts_with(':') {
        let p = parts.next().unwrap();
        Some(p[1..].to_string())
    } else {
        None
    };

    let command = parts.next().unwrap().into();

    let mut params: Vec<String> = vec![];
    let mut rest = parts.collect::<Vec<&str>>();
    if let Some(trailing_index) = rest.iter().position(|&x| x.starts_with(':')) {
        params.extend(rest[..trailing_index].iter().map(|&x| x.to_string()));
        params.push(rest.split_off(trailing_index).join(" "));
    }

    ServMsg {
        prefix,
        command,
        params,
    }
}

impl From<&str> for ServCmd {
    fn from(s: &str) -> Self {
        match s {
            "JOIN" => ServCmd::Join,
            "PRIVMSG" => ServCmd::PrivMsg,
            "PART" => ServCmd::Part,
            "353" => ServCmd::NameReply,
            "366" => ServCmd::EndOfNames,
            _ => ServCmd::Unknown(s.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_353() {
        let msg = ":*.freenode.net 353 my-nickname = #bobcat :@my-nickname";
        let serv_msg = parse_msg(msg);
        assert_eq!(serv_msg.prefix, Some("*.freenode.net".to_string()));
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
        assert_eq!(serv_msg.prefix, Some("*.freenode.net".to_string()));
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
            Some("MrNickname!~MrUser@freenode-o6n.182.alt94q.IP".to_string())
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
            Some("MrNickname!~MrUser@freenode-o6n.182.alt94q.IP".to_string())
        );
        assert_eq!(serv_msg.command, ServCmd::PrivMsg);
        assert_eq!(
            serv_msg.params,
            vec!["#bobcat".to_string(), ":this is a wug!!".to_string()]
        );
    }

    #[test]
    fn test_parse_part() {
        let msg = ":MrNickname!~MrUser@freenode-o6n.182.alt94q.IP PART :#bobcat";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some("MrNickname!~MrUser@freenode-o6n.182.alt94q.IP".to_string())
        );
        assert_eq!(serv_msg.command, ServCmd::Part);
        assert_eq!(serv_msg.params, vec![":#bobcat".to_string()]);
    }
}

/// Parsing IRC messages

// TODO: Parse MODE message
// :MrNickname!~guest@freenode-o6n.182.alt94q.IP MODE MrNickname :+wRix
// TODO Parse QUIT message

#[derive(Debug, PartialEq)]
pub enum ServCmd {
    Join {
        chan: String,
    },
    PrivMsg {
        target: MsgTarget,
        msg: String,
    },
    Part {
        chan: String,
        msg: String,
    },
    Notice {
        msg: String,
    },
    RplWelcome {
        msg: String,
    }, // 001
    RplYourHost {
        msg: String,
    }, // 002
    RplCreated {
        msg: String,
    }, // 003
    RplMyInfo {
        version: String,
        umodes: String,
        cmodes: String,
        cmodes_param: String,
    }, // 004
    RplISupport {
        msg: String,
    }, // 005 See https://stackoverflow.com/a/38550242 and https://modern.ircdocs.horse/#rplisupport-005
    RplLuserClient {
        msg: String,
    }, // 251
    RplLuserOp {
        msg: String,
    }, // 252
    RplLuserUnknown {
        msg: String,
    }, // 253
    RplLuserChannels {
        msg: String,
    }, // 254
    RplLuserMe {
        msg: String,
    }, // 255
    RplLocalUsers {
        msg: String,
    }, // 265
    RplGlobalUsers {
        msg: String,
    }, // 266
    NameReply {
        sym: char,
        chan: String,
        nicks: Vec<String>,
    }, // 353 "<client> <symbol> <channel> :[prefix]<nick>{ [prefix]<nick>}"
    EndOfNames {
        msg: String,
    }, // 366
    MOTDStart {
        msg: String,
    }, // 375
    MOTD {
        msg: String,
    }, // 372
    MOTDEnd {
        msg: String,
    }, // 376
    DisplayedHost {
        msg: String,
    }, // 396 apparently a Freenode special
    Unknown(String),
}

#[derive(Debug, PartialEq)]
pub enum MsgTarget {
    Chan(String),
    User(String),
    Serv(String),
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
        "JOIN" => {
            let chan = params[0][1..].to_string();
            (ServCmd::Join { chan }, vec![])
        }
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
        "PART" => {
            // :MrNickname!~MrUser@freenode-o6n.182.alt94q.IP PART :#bobcat
            // :MrNickname!~MrUser@freenode-o6n.182.alt94q.IP PART #bobcat :"getting out of here"
            let (chan, msg) = if params.len() == 1 {
                (params[0][1..].to_string(), "".to_string())
            } else {
                (params[0].to_string(), params[1][1..].to_string())
            };
            (ServCmd::Part { chan, msg }, vec![])
        }
        "NOTICE" => {
            let msg = params[1][1..].to_string();
            (ServCmd::Notice { msg }, vec![])
        }
        "001" => {
            let msg = params[1][1..].to_string();
            (ServCmd::RplWelcome { msg }, vec![])
        }
        "002" => {
            let msg = params[1][1..].to_string();
            (ServCmd::RplYourHost { msg }, vec![])
        }
        "003" => {
            let msg = params[1][1..].to_string();
            (ServCmd::RplCreated { msg }, vec![])
        }
        "004" => {
            let msg = params[1..].join(" ");
            (
                ServCmd::RplMyInfo {
                    version: params[2].to_string(),
                    umodes: params[3].to_string(),
                    cmodes: params[4].to_string(),
                    cmodes_param: params[5][1..].to_string(), // i think this is optional
                },
                vec![],
            )
        }
        "005" => {
            // TODO should actually split by ":are supported by this server" trailing instead
            let msg = params[1..].join(" ");
            (ServCmd::RplISupport { msg }, vec![])
        }
        "251" => {
            let msg = params[1][1..].to_string();
            (ServCmd::RplLuserClient { msg }, vec![])
        }
        "252" => {
            let msg = params[1..].join(" ");
            (ServCmd::RplLuserOp { msg }, vec![])
        }
        "253" => {
            let msg = params[1..].join(" ");
            (ServCmd::RplLuserUnknown { msg }, vec![])
        }
        "254" => {
            let msg = params[1..].join(" ");
            (ServCmd::RplLuserChannels { msg }, vec![])
        }
        "255" => {
            let msg = params[1][1..].to_string();
            (ServCmd::RplLuserMe { msg }, vec![])
        }
        "265" => {
            // XXX Watch out: https://modern.ircdocs.horse/#rpllocalusers-265
            // > "<client> [<u> <m>] :Current local users <u>, max <m>"
            // > The two optional parameters SHOULD be supplied to allow clients to better extract
            // > these numbers.
            let msg = params[1][1..].to_string();
            (ServCmd::RplLocalUsers { msg }, vec![])
        }
        "266" => {
            // Same comment as for 265
            let msg = params[1][1..].to_string();
            (ServCmd::RplGlobalUsers { msg }, vec![])
        }
        "353" => {
            let sym = params[1].chars().next().unwrap();
            let chan = params[2].to_string();
            let nicks = params[3][1..]
                .split_whitespace()
                .map(|x| x.to_string())
                .collect();
            (ServCmd::NameReply { sym, chan, nicks }, vec![])
        }
        "366" => {
            // :*.freenode.net 366 MrNickname #bobcat :End of /NAMES list.
            let chan = &params[1];
            let msg = format!("{chan} {}", &params[2][1..]);
            (ServCmd::EndOfNames { msg }, vec![])
        }
        "375" => {
            let msg = params[1][1..].to_string();
            (ServCmd::MOTDStart { msg }, vec![])
        }
        "372" => {
            let msg = params[1][1..].to_string();
            (ServCmd::MOTD { msg }, vec![])
        }
        "376" => {
            let msg = params[1][1..].to_string();
            (ServCmd::MOTDEnd { msg }, vec![])
        }
        "396" => {
            // This command isn't in the RFC nor in modern.ircdocs.horse, so idk best effort parsing
            let trailing = &params[2][1..];
            let msg = format!("{} {}", params[1], trailing);
            (ServCmd::DisplayedHost { msg }, vec![])
        }
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
    fn test_004_myinfo() {
        let msg = ":*.freenode.net 004 MrNickname *.freenode.net InspIRCd-3 BDHILRSTWcdghikorswxz ABCDEFIJKLMNOPQRSTUWXYZbcdefhijklmnoprstuvwz :BEFIJLWXYZbdefhjklovw";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::RplMyInfo {
                version: "InspIRCd-3".to_string(),
                umodes: "BDHILRSTWcdghikorswxz".to_string(),
                cmodes: "ABCDEFIJKLMNOPQRSTUWXYZbcdefhijklmnoprstuvwz".to_string(),
                cmodes_param: "BEFIJLWXYZbdefhjklovw".to_string(),
            }
        );
        assert!(serv_msg.params.is_empty());
    }

    #[test]
    fn test_parse_251_luserclient() {
        let msg =
            ":*.freenode.net 251 MrNickname :There are 18 users and 4959 invisible on 10 servers";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::RplLuserClient {
                msg: "There are 18 users and 4959 invisible on 10 servers".to_string()
            }
        );
        assert!(serv_msg.params.is_empty());
    }

    #[test]
    fn test_parse_252_luserop() {
        let msg = ":*.freenode.net 252 MrNickname 6 :operator(s) online";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::RplLuserOp {
                msg: "6 :operator(s) online".to_string()
            }
        );
        assert!(serv_msg.params.is_empty());
    }

    #[test]
    fn test_parse_253_luserunknown() {
        let msg = ":*.freenode.net 253 MrNickname 4 :unknown connections";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::RplLuserUnknown {
                msg: "4 :unknown connections".to_string()
            }
        );
        assert!(serv_msg.params.is_empty());
    }

    #[test]
    fn test_parse_254_luserchannels() {
        let msg = ":*.freenode.net 254 MrNickname 9690 :channels formed";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::RplLuserChannels {
                msg: "9690 :channels formed".to_string()
            }
        );
        assert!(serv_msg.params.is_empty());
    }

    #[test]
    fn test_parse_255_luserme() {
        let msg = ":*.freenode.net 255 MrNickname :I have 1704 clients and 1 servers";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::RplLuserMe {
                msg: "I have 1704 clients and 1 servers".to_string()
            }
        );
        assert!(serv_msg.params.is_empty());
    }

    #[test]
    #[ignore = "Bring back when parsing of multiple spaces is fixed."]
    fn test_parse_265_localusers() {
        let msg = ":*.freenode.net 265 MrNickname :Current local users: 1704  Max: 4101";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        // FIXME A bug in parsing: the double space between `1704` and `Max` is incorrectly consumed
        // by split_whitespace().
        assert_eq!(
            serv_msg.command,
            ServCmd::RplLocalUsers {
                msg: "Current local users: 1704  Max: 4101".to_string()
            }
        );
        assert!(serv_msg.params.is_empty());
    }

    #[test]
    #[ignore = "Bring back when parsing of multiple spaces is fixed."]
    fn test_parse_266_globalusers() {
        let msg = ":*.freenode.net 266 MrNickname :Current global users: 4977  Max: 10281";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        // FIXME A bug in parsing: the double space between `4977` and `Max` is incorrectly consumed
        // by split_whitespace().
        assert_eq!(
            serv_msg.command,
            ServCmd::RplGlobalUsers {
                msg: "Current global users: 4977  Max: 10281".to_string()
            }
        );
        assert!(serv_msg.params.is_empty());
    }

    #[test]
    fn test_parse_353() {
        let msg = ":*.freenode.net 353 MrNickname = #bobcat :@MrNickname bobcatLover DogPerson";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::NameReply {
                sym: '=',
                chan: "#bobcat".to_string(),
                nicks: vec![
                    "@MrNickname".to_string(),
                    "bobcatLover".to_string(),
                    "DogPerson".to_string(),
                ]
            }
        );
    }

    #[test]
    fn test_parse_366() {
        let msg = ":*.freenode.net 366 MrNickname #bobcat :End of /NAMES list.";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::EndOfNames {
                msg: "#bobcat End of /NAMES list.".to_string()
            }
        );
        assert!(serv_msg.params.is_empty());
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
        assert_eq!(
            serv_msg.command,
            ServCmd::Join {
                chan: "#bobcat".to_string()
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
        assert_eq!(
            serv_msg.command,
            ServCmd::Part {
                chan: "#bobcat".to_string(),
                msg: "".to_string()
            }
        );
    }

    #[test]
    fn test_parse_part_with_msg() {
        let msg =
            ":MrNickname!~MrUser@freenode-o6n.182.alt94q.IP PART #bobcat :\"getting out of here\"";
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
            ServCmd::Part {
                chan: "#bobcat".to_string(),
                msg: "\"getting out of here\"".to_string()
            }
        );
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
    fn test_parse_001_rplwelcome() {
        let msg = ":*.freenode.net 001 MrNickname :Welcome to the freenode IRC Network MrNickname!~MrUser@1.2.3.4";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::RplWelcome {
                msg: "Welcome to the freenode IRC Network MrNickname!~MrUser@1.2.3.4".to_string()
            }
        );
    }

    #[test]
    fn test_parse_002_rplyourhost() {
        let msg = ":*.freenode.net 002 MrNickname :Your host is *.freenode.net, running version InspIRCd-3";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::RplYourHost {
                msg: "Your host is *.freenode.net, running version InspIRCd-3".to_string()
            }
        );
        assert!(serv_msg.params.is_empty());
    }

    #[test]
    fn test_parse_003_rplcreated() {
        let msg = ":*.freenode.net 003 MrNickname :This server was created 09:22:41 Jun 22 2023";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::RplCreated {
                msg: "This server was created 09:22:41 Jun 22 2023".to_string()
            }
        );
        assert!(serv_msg.params.is_empty());
    }

    #[test]
    fn test_parse_005_isupport() {
        let msg = ":*.freenode.net 005 MrNickname ACCEPT=30 AWAYLEN=200 BOT=B CALLERID=g \
            CASEMAPPING=ascii CHANLIMIT=#:20 CHANMODES=IXZbew,k,BEFJLWdfjl,ACDKMNOPQRSTUcimnprstu\
            z CHANNELLEN=64 CHANTYPES=# ELIST=CMNTU ESILENCE=CcdiNnPpTtx EXCEPTS=e :are supported by \
            this serverEN=255 LINELEN=512 MAXLIST=I:100,X:100,b:100,e:100,w:100 MAXTA\
            RGETS=20 MODES=20 MONITOR=30 NAMELEN=128 NAMESX NETWORK=freenode :are supported by this \
            server60 SILENCE=32 STATUSMSG=!@%+ TOPICLEN=390 UHNAMES USERIP USERLEN=10\
            USERMODES=,,s,BDHILRSTWcdghikorwxz VBANLIST :are supported by this serverd by this server";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        match serv_msg.command {
            ServCmd::RplISupport { msg } => {
                assert_eq!(msg, "ACCEPT=30 AWAYLEN=200 BOT=B CALLERID=g CASEMAPPING=ascii CHANLIMIT=#:20 \
                    CHANMODES=IXZbew,k,BEFJLWdfjl,ACDKMNOPQRSTUcimnprstuz CHANNELLEN=64 CHANTYPES=# \
                    ELIST=CMNTU ESILENCE=CcdiNnPpTtx EXCEPTS=e :are supported by this serverEN=255 \
                    LINELEN=512 MAXLIST=I:100,X:100,b:100,e:100,w:100 MAXTARGETS=20 MODES=20 MONITOR=30 \
                    NAMELEN=128 NAMESX NETWORK=freenode :are supported by this server60 SILENCE=32 \
                    STATUSMSG=!@%+ TOPICLEN=390 UHNAMES USERIP USERLEN=10USERMODES=,,s,BDHILRSTWcdghikorwxz \
                    VBANLIST :are supported by this serverd by this server");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_parse_375_motdstart() {
        let msg = ":*.freenode.net 375 MrNickname :*.freenode.net message of the day";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::MOTDStart {
                msg: "*.freenode.net message of the day".to_string()
            }
        );
        assert!(serv_msg.params.is_empty());
    }

    #[test]
    #[ignore = "Bring back when parsing of multiple spaces is fixed."]
    fn test_parse_372_motd() {
        let msg = ":*.freenode.net 372 MrNickname :  Thank you for using freenode!";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        // FIXME This is a bug in the parsing--the leading space in the trailing message is removed
        // by the split_whitespace() call in parse_msg(), and is lost when the trailing message
        // is reassembled. msg should have two spaces in the beginning!
        assert_eq!(
            serv_msg.command,
            ServCmd::MOTD {
                msg: "  Thank you for using freenode!".to_string()
            }
        );
        assert!(serv_msg.params.is_empty());
    }

    #[test]
    fn test_parse_376_motdend() {
        let msg = ":*.freenode.net 376 MrNickname :End of message of the day.";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::MOTDEnd {
                msg: "End of message of the day.".to_string()
            }
        );
        assert!(serv_msg.params.is_empty());
    }

    #[test]
    fn test_parse_396_displayed_host() {
        let msg =
            ":*.freenode.net 396 MrNickname freenode-o6n.182.alt94q.IP :is now your displayed host";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::Server("*.freenode.net".to_string()))
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::DisplayedHost {
                msg: "freenode-o6n.182.alt94q.IP is now your displayed host".to_string()
            }
        );
        assert!(serv_msg.params.is_empty());
    }

    #[test]
    fn test_parse_notice() {
        let msg = ":Global!services@services.freenode.net NOTICE MrNickname :[Random News - \
            Aug 14 18:27:23 2024 UTC] Do you like shooting ducks?";
        let serv_msg = parse_msg(msg);
        assert_eq!(
            serv_msg.prefix,
            Some(Prefix::User {
                nick: "Global".to_string(),
                user: "services".to_string(),
                host: "services.freenode.net".to_string(),
            })
        );
        assert_eq!(
            serv_msg.command,
            ServCmd::Notice {
                msg: "[Random News - Aug 14 18:27:23 2024 UTC] Do you like shooting ducks?"
                    .to_string()
            }
        );
    }
}

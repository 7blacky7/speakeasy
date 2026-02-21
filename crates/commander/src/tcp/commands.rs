//! Befehlsausfuehrung fuer das TCP/TLS-Interface
//!
//! Uebersetzt geparste TCP-Befehle in Command-Enum-Werte
//! und delegiert die Ausfuehrung an den CommandExecutor.

use uuid::Uuid;

use crate::commands::types::{BerechtigungsWertInput, Command};
use crate::error::{CommanderError, CommanderResult};
use crate::tcp::parser::ParsedCommand;

/// Konvertiert einen ParsedCommand in einen Command-Enum-Wert
pub fn tcp_befehl_zu_command(cmd: &ParsedCommand) -> CommanderResult<Command> {
    match cmd.name.as_str() {
        // --- Server ---
        "serverinfo" => Ok(Command::ServerInfo),
        "serveredit" => Ok(Command::ServerEdit {
            name: cmd.param("name").map(String::from),
            willkommensnachricht: cmd.param("welcomemsg").map(String::from),
            max_clients: cmd.param("maxclients").and_then(|s| s.parse().ok()),
            host_nachricht: cmd.param("hostmsg").map(String::from),
        }),
        "serverstop" => Ok(Command::ServerStop {
            grund: cmd.param("reason").map(String::from),
        }),

        // --- Kanaele ---
        "channellist" => Ok(Command::KanalListe),
        "channelcreate" => {
            let name = cmd.required_param("name")?.to_string();
            Ok(Command::KanalErstellen {
                name,
                parent_id: cmd.param("cpid").and_then(|s| Uuid::parse_str(s).ok()),
                thema: cmd.param("topic").map(String::from),
                passwort: cmd.param("password").map(String::from),
                max_clients: cmd
                    .param("maxclients")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                sort_order: cmd.param("order").and_then(|s| s.parse().ok()).unwrap_or(0),
                permanent: cmd
                    .param("channel_flag_permanent")
                    .map(|s| s == "1")
                    .unwrap_or(false),
            })
        }
        "channeledit" => {
            let id = cmd.uuid_param("cid")?;
            Ok(Command::KanalBearbeiten {
                id,
                name: cmd.param("name").map(String::from),
                thema: cmd.param("topic").map(|s| Some(s.to_string())),
                max_clients: cmd.param("maxclients").and_then(|s| s.parse().ok()),
                sort_order: cmd.param("order").and_then(|s| s.parse().ok()),
            })
        }
        "channeldelete" => Ok(Command::KanalLoeschen {
            id: cmd.uuid_param("cid")?,
        }),

        // --- Clients ---
        "clientlist" => Ok(Command::ClientListe),
        "clientkick" => Ok(Command::ClientKicken {
            client_id: cmd.uuid_param("clid")?,
            grund: cmd.param("reason").map(String::from),
        }),
        "clientban" | "banclient" => Ok(Command::ClientBannen {
            client_id: cmd.uuid_param("clid")?,
            dauer_secs: cmd.param("duration").and_then(|s| s.parse().ok()),
            grund: cmd.param("reason").map(String::from),
            ip_bannen: cmd.param("banip").map(|s| s == "1").unwrap_or(false),
        }),
        "clientmove" => Ok(Command::ClientVerschieben {
            client_id: cmd.uuid_param("clid")?,
            kanal_id: cmd.uuid_param("cid")?,
        }),
        "clientpoke" => Ok(Command::ClientPoken {
            client_id: cmd.uuid_param("clid")?,
            nachricht: cmd.required_param("msg")?.to_string(),
        }),

        // --- Berechtigungen ---
        "permlist" => Ok(Command::BerechtigungListe {
            ziel: cmd.required_param("target")?.to_string(),
            scope: cmd.param("scope").unwrap_or("server").to_string(),
        }),
        "permset" | "permadd" => Ok(Command::BerechtigungSetzen {
            ziel: cmd.required_param("target")?.to_string(),
            permission: cmd.required_param("permsid")?.to_string(),
            wert: parse_perm_value(cmd)?,
            scope: cmd.param("scope").unwrap_or("server").to_string(),
        }),
        "permdel" | "permremove" => Ok(Command::BerechtigungEntfernen {
            ziel: cmd.required_param("target")?.to_string(),
            permission: cmd.required_param("permsid")?.to_string(),
            scope: cmd.param("scope").unwrap_or("server").to_string(),
        }),

        // --- Dateien ---
        "ftlist" | "filelist" => Ok(Command::DateiListe {
            kanal_id: cmd.uuid_param("cid")?,
        }),
        "ftdeletefile" | "filedelete" => Ok(Command::DateiLoeschen {
            datei_id: cmd.required_param("fid")?.to_string(),
        }),

        // --- Logs ---
        "logview" => Ok(Command::LogAbfragen {
            limit: cmd
                .param("lines")
                .and_then(|s| s.parse().ok())
                .unwrap_or(50),
            offset: cmd
                .param("begin_pos")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            aktion_filter: cmd.param("filter").map(String::from),
        }),

        other => Err(CommanderError::Protokoll(format!(
            "Unbekannter Befehl: {other}"
        ))),
    }
}

fn parse_perm_value(cmd: &ParsedCommand) -> CommanderResult<BerechtigungsWertInput> {
    if let Some(v) = cmd.param("permvalue") {
        match v {
            "grant" => return Ok(BerechtigungsWertInput::Grant),
            "deny" => return Ok(BerechtigungsWertInput::Deny),
            "skip" => return Ok(BerechtigungsWertInput::Skip),
            s => {
                if let Ok(n) = s.parse::<i64>() {
                    return Ok(BerechtigungsWertInput::IntLimit(n));
                }
            }
        }
    }
    // Standard: Grant
    Ok(BerechtigungsWertInput::Grant)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tcp::parser::parse_line;

    #[test]
    fn serverinfo_befehl() {
        let parsed = parse_line("serverinfo").unwrap();
        let cmd = tcp_befehl_zu_command(&parsed).unwrap();
        assert_eq!(cmd, Command::ServerInfo);
    }

    #[test]
    fn channellist_befehl() {
        let parsed = parse_line("channellist").unwrap();
        let cmd = tcp_befehl_zu_command(&parsed).unwrap();
        assert_eq!(cmd, Command::KanalListe);
    }

    #[test]
    fn channelcreate_befehl() {
        let parsed = parse_line("channelcreate name=General").unwrap();
        let cmd = tcp_befehl_zu_command(&parsed).unwrap();
        assert!(matches!(cmd, Command::KanalErstellen { .. }));
        if let Command::KanalErstellen { name, .. } = cmd {
            assert_eq!(name, "General");
        }
    }

    #[test]
    fn channelcreate_ohne_name_gibt_fehler() {
        let parsed = parse_line("channelcreate topic=Test").unwrap();
        assert!(tcp_befehl_zu_command(&parsed).is_err());
    }

    #[test]
    fn clientkick_befehl() {
        let id = Uuid::new_v4();
        let zeile = format!("clientkick clid={id} reason=Spam");
        let parsed = parse_line(&zeile).unwrap();
        let cmd = tcp_befehl_zu_command(&parsed).unwrap();
        if let Command::ClientKicken { client_id, grund } = cmd {
            assert_eq!(client_id, id);
            assert_eq!(grund, Some("Spam".to_string()));
        } else {
            panic!("Falscher Command-Typ");
        }
    }

    #[test]
    fn unbekannter_befehl_gibt_fehler() {
        let parsed = parse_line("unbekannt").unwrap();
        assert!(tcp_befehl_zu_command(&parsed).is_err());
    }

    #[test]
    fn logview_mit_limit() {
        let parsed = parse_line("logview lines=100 begin_pos=50").unwrap();
        let cmd = tcp_befehl_zu_command(&parsed).unwrap();
        if let Command::LogAbfragen { limit, offset, .. } = cmd {
            assert_eq!(limit, 100);
            assert_eq!(offset, 50);
        } else {
            panic!("Falscher Command-Typ");
        }
    }
}

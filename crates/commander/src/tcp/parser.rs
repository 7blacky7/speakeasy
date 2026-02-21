//! Befehlsparser fuer das TCP/TLS-Interface (ServerQuery-Stil)
//!
//! Parst zeilenbasierte Befehle im Format:
//!   befehlsname key1=value1 key2="value with spaces" key3=wert3
//!
//! Sonderzeichen in Werten werden mit Backslash escaped:
//!   \s = Leerzeichen, \n = Newline, \\ = Backslash, \| = Pipe

use std::collections::HashMap;

use crate::error::{CommanderError, CommanderResult};

/// Ein geparster TCP-Befehl
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCommand {
    /// Befehlsname (z.B. "serverinfo", "channellist")
    pub name: String,
    /// Key-Value-Parameter
    pub params: HashMap<String, String>,
}

impl ParsedCommand {
    /// Gibt einen Parameter als String zurueck
    pub fn param(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(|s| s.as_str())
    }

    /// Gibt einen Pflicht-Parameter zurueck oder einen Fehler
    pub fn required_param(&self, key: &str) -> CommanderResult<&str> {
        self.param(key).ok_or_else(|| {
            CommanderError::UngueltigeEingabe(format!("Pflicht-Parameter fehlt: {key}"))
        })
    }

    /// Gibt einen Parameter als UUID zurueck
    pub fn uuid_param(&self, key: &str) -> CommanderResult<uuid::Uuid> {
        let s = self.required_param(key)?;
        uuid::Uuid::parse_str(s)
            .map_err(|_| CommanderError::UngueltigeEingabe(format!("Ungueltige UUID fuer '{key}': {s}")))
    }

    /// Gibt einen Parameter als u64 zurueck
    pub fn u64_param(&self, key: &str) -> CommanderResult<u64> {
        let s = self.required_param(key)?;
        s.parse::<u64>()
            .map_err(|_| CommanderError::UngueltigeEingabe(format!("Ungueltige Zahl fuer '{key}': {s}")))
    }
}

/// Parst eine Befehlszeile im ServerQuery-Format
///
/// Format: `befehlsname [key=value ...]`
/// Werte koennen mit " " gequotet oder mit \s escaped sein.
pub fn parse_line(line: &str) -> CommanderResult<ParsedCommand> {
    let line = line.trim();
    if line.is_empty() {
        return Err(CommanderError::Protokoll("Leere Befehlszeile".into()));
    }

    // Tokens durch Leerzeichen trennen (aber quoted Werte beachten)
    let tokens = tokenize(line);
    if tokens.is_empty() {
        return Err(CommanderError::Protokoll("Kein Befehlsname".into()));
    }

    let name = tokens[0].to_lowercase();
    let mut params = HashMap::new();

    for token in &tokens[1..] {
        if let Some((key, value)) = token.split_once('=') {
            let decoded = decode_value(value);
            params.insert(key.to_lowercase(), decoded);
        }
        // Token ohne '=' werden ignoriert (kein Wert)
    }

    Ok(ParsedCommand { name, params })
}

/// Zerlegt eine Zeile in Tokens, beachtet quoted Strings
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            '\\' => {
                if let Some(&next) = chars.peek() {
                    chars.next();
                    match next {
                        's' => current.push(' '),
                        'n' => current.push('\n'),
                        '\\' => current.push('\\'),
                        '|' => current.push('|'),
                        '"' => current.push('"'),
                        other => {
                            current.push('\\');
                            current.push(other);
                        }
                    }
                }
            }
            _ => current.push(c),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Dekodiert Escape-Sequenzen in einem Wert-String
fn decode_value(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('s') => result.push(' '),
                Some('n') => result.push('\n'),
                Some('\\') => result.push('\\'),
                Some('|') => result.push('|'),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Kodiert einen Wert fuer die Ausgabe (Escape-Sequenzen einfuegen)
pub fn encode_value(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(' ', "\\s")
        .replace('\n', "\\n")
        .replace('|', "\\|")
}

/// Erstellt eine Erfolgs-Antwortzeile
pub fn ok_antwort(params: &[(&str, &str)]) -> String {
    if params.is_empty() {
        "ok\n".to_string()
    } else {
        let kv: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, encode_value(v)))
            .collect();
        format!("ok {}\n", kv.join(" "))
    }
}

/// Erstellt eine Fehler-Antwortzeile
pub fn fehler_antwort_tcp(code: u32, nachricht: &str) -> String {
    format!("error id={} msg={}\n", code, encode_value(nachricht))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_einfacher_befehl() {
        let cmd = parse_line("serverinfo").unwrap();
        assert_eq!(cmd.name, "serverinfo");
        assert!(cmd.params.is_empty());
    }

    #[test]
    fn parse_befehl_mit_params() {
        let cmd = parse_line("login username=admin password=geheim").unwrap();
        assert_eq!(cmd.name, "login");
        assert_eq!(cmd.param("username"), Some("admin"));
        assert_eq!(cmd.param("password"), Some("geheim"));
    }

    #[test]
    fn parse_escaped_leerzeichen() {
        let cmd = parse_line(r"channelcreate name=Mein\sKanal").unwrap();
        assert_eq!(cmd.param("name"), Some("Mein Kanal"));
    }

    #[test]
    fn parse_quoted_wert() {
        let cmd = parse_line(r#"clientkick clid=5 reason="Spam und Werbung""#).unwrap();
        assert_eq!(cmd.param("reason"), Some("Spam und Werbung"));
    }

    #[test]
    fn parse_case_insensitive_name() {
        let cmd = parse_line("ServerInfo").unwrap();
        assert_eq!(cmd.name, "serverinfo");
    }

    #[test]
    fn leere_zeile_gibt_fehler() {
        assert!(parse_line("").is_err());
        assert!(parse_line("   ").is_err());
    }

    #[test]
    fn required_param_fehlt() {
        let cmd = parse_line("login username=admin").unwrap();
        assert!(cmd.required_param("password").is_err());
    }

    #[test]
    fn ok_antwort_ohne_params() {
        assert_eq!(ok_antwort(&[]), "ok\n");
    }

    #[test]
    fn ok_antwort_mit_params() {
        let antwort = ok_antwort(&[("name", "Mein Server"), ("clients", "5")]);
        assert!(antwort.starts_with("ok "));
        assert!(antwort.contains("name=Mein\\sServer"));
        assert!(antwort.contains("clients=5"));
    }

    #[test]
    fn fehler_antwort_format() {
        let antwort = fehler_antwort_tcp(1001, "Ungueltige Anmeldedaten");
        assert!(antwort.starts_with("error id=1001"));
        assert!(antwort.contains("msg="));
    }

    #[test]
    fn encode_decode_roundtrip() {
        let original = "Hallo Welt\nMit Newline";
        let encoded = encode_value(original);
        let decoded = decode_value(&encoded);
        assert_eq!(decoded, original);
    }
}

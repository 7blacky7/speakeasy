//! TCP/TLS-Server fuer den Commander (ServerQuery-Stil)
//!
//! Line-based Protokoll ueber TLS. Kein Plaintext.
//! Format: Befehlsname [key=value ...]\n
//! Antworten: ok [...] oder error id=N msg=...

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use crate::rate_limit::RateLimiter;
use crate::rest::CommanderState;
use crate::tcp::commands::tcp_befehl_zu_command;
use crate::tcp::parser::{fehler_antwort_tcp, ok_antwort, parse_line};
use crate::tcp::session::TcpSession;

/// TCP/TLS-Server-Konfiguration
#[derive(Debug, Clone)]
pub struct TcpServerKonfig {
    pub bind_addr: SocketAddr,
    pub max_verbindungen: usize,
    pub zeilenlimit_bytes: usize,
}

impl Default for TcpServerKonfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:9301".parse().unwrap(),
            max_verbindungen: 100,
            zeilenlimit_bytes: 8192,
        }
    }
}

/// TCP/TLS-Commander-Server
pub struct TcpServer {
    konfig: TcpServerKonfig,
}

impl TcpServer {
    pub fn neu(konfig: TcpServerKonfig) -> Self {
        Self { konfig }
    }

    /// Startet den TCP/TLS-Server
    pub async fn starten(
        self,
        state: CommanderState,
        rate_limiter: Arc<RateLimiter>,
        tls_acceptor: TlsAcceptor,
    ) -> Result<()> {
        let listener = TcpListener::bind(self.konfig.bind_addr).await?;
        tracing::info!(addr = %self.konfig.bind_addr, "TCP/TLS-Commander-Server gestartet");

        // Atomarer Verbindungszaehler fuer max_verbindungen-Enforcement
        let verbindungszaehler = Arc::new(AtomicUsize::new(0));
        let max_verbindungen = self.konfig.max_verbindungen;

        loop {
            let (stream, peer_addr) = listener.accept().await?;

            // Connection-Limit pruefen BEVOR TLS-Handshake
            let aktuelle = verbindungszaehler.fetch_add(1, Ordering::SeqCst);
            if aktuelle >= max_verbindungen {
                verbindungszaehler.fetch_sub(1, Ordering::SeqCst);
                tracing::warn!(
                    peer = %peer_addr,
                    max = max_verbindungen,
                    "Verbindung abgelehnt: Connection-Limit erreicht"
                );
                // Stream wird durch Drop geschlossen
                continue;
            }

            // Per-IP Rate Limit fuer neue Verbindungen
            let ip = peer_addr.ip().to_string();
            if let Err(retry_after) = rate_limiter.pruefe_ip(&ip) {
                verbindungszaehler.fetch_sub(1, Ordering::SeqCst);
                tracing::warn!(
                    peer = %peer_addr,
                    retry_after,
                    "Verbindung abgelehnt: IP-Rate-Limit"
                );
                continue;
            }

            let tls_acceptor = tls_acceptor.clone();
            let state = state.clone();
            let rate_limiter = Arc::clone(&rate_limiter);
            let zaehler = Arc::clone(&verbindungszaehler);
            let zeilenlimit = self.konfig.zeilenlimit_bytes;

            tokio::spawn(async move {
                match tls_acceptor.accept(stream).await {
                    Ok(tls_stream) => {
                        tracing::debug!(peer = %peer_addr, "Neue TCP/TLS-Verbindung");
                        verbindung_behandeln(
                            tls_stream,
                            peer_addr,
                            state,
                            rate_limiter,
                            zeilenlimit,
                        )
                        .await;
                    }
                    Err(e) => {
                        tracing::warn!(peer = %peer_addr, fehler = %e, "TLS-Handshake fehlgeschlagen");
                    }
                }
                // Verbindungszaehler nach Abschluss dekrementieren
                zaehler.fetch_sub(1, Ordering::SeqCst);
            });
        }
    }
}

/// Behandelt eine einzelne TLS-Verbindung
async fn verbindung_behandeln(
    stream: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    peer_addr: SocketAddr,
    state: CommanderState,
    rate_limiter: Arc<RateLimiter>,
    _zeilenlimit: usize,
) {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut buf_reader = BufReader::new(reader);
    let mut session = TcpSession::neu(peer_addr);
    let ip = peer_addr.ip().to_string();

    // Willkommensnachricht
    let _ = writer
        .write_all(b"TS3\nWelcome to the Speakeasy Commander\n")
        .await;

    let mut zeile = String::new();
    loop {
        zeile.clear();
        match buf_reader.read_line(&mut zeile).await {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                tracing::debug!(fehler = %e, "Lesefehler auf TCP-Session");
                break;
            }
        }

        // Per-IP Rate Limit pro Befehl (nach dem Verbindungsaufbau)
        if let Err(retry_after) = rate_limiter.pruefe_ip(&ip) {
            let antwort = fehler_antwort_tcp(
                3008,
                &format!("Rate-Limit ueberschritten, bitte in {retry_after}s erneut versuchen"),
            );
            let _ = writer.write_all(antwort.as_bytes()).await;
            break;
        }

        let antwort = verarbeite_befehl(&zeile, &mut session, &state, &rate_limiter, &ip).await;
        if writer.write_all(antwort.as_bytes()).await.is_err() {
            break;
        }

        if session.zustand == crate::tcp::session::SessionZustand::Beendend {
            break;
        }
    }

    tracing::debug!(peer = %peer_addr, "TCP/TLS-Verbindung beendet");
}

/// Verarbeitet eine einzelne Befehlszeile und gibt die Antwort zurueck
async fn verarbeite_befehl(
    zeile: &str,
    session: &mut TcpSession,
    state: &CommanderState,
    rate_limiter: &RateLimiter,
    ip: &str,
) -> String {
    let parsed = match parse_line(zeile) {
        Ok(p) => p,
        Err(e) => return fehler_antwort_tcp(e.fehler_code(), &e.to_string()),
    };

    // Sonderbefehle: login, quit
    match parsed.name.as_str() {
        "login" => {
            let username = match parsed.param("username") {
                Some(u) => u.to_string(),
                None => return fehler_antwort_tcp(1005, "username fehlt"),
            };
            let passwort = match parsed.param("password") {
                Some(p) => p.to_string(),
                None => return fehler_antwort_tcp(1005, "password fehlt"),
            };
            let token = format!("{username}:{passwort}");
            match (state.token_validator)(&token) {
                Ok(cmd_session) => {
                    let session_id = uuid::Uuid::new_v4().to_string();
                    session.anmelden(cmd_session);
                    return ok_antwort(&[("session_token", &session_id)]);
                }
                Err(e) => return fehler_antwort_tcp(e.fehler_code(), &e.to_string()),
            }
        }
        "quit" => {
            session.zustand = crate::tcp::session::SessionZustand::Beendend;
            return ok_antwort(&[("msg", "bye")]);
        }
        _ => {}
    }

    // Alle anderen Befehle erfordern Authentifizierung
    let cmd_session = match &session.commander_session {
        Some(s) => s.clone(),
        None => {
            return fehler_antwort_tcp(1001, "Nicht eingeloggt. Bitte zuerst 'login' aufrufen.")
        }
    };

    // Befehl konvertieren
    let command = match tcp_befehl_zu_command(&parsed) {
        Ok(c) => c,
        Err(e) => return fehler_antwort_tcp(e.fehler_code(), &e.to_string()),
    };

    // Zusaetzliches Rate Limiting fuer teure Operationen
    if command.ist_teure_operation() {
        if let Err(retry_after) = rate_limiter.pruefe_teure_operation(ip) {
            return fehler_antwort_tcp(3008, &format!(
                "Rate-Limit fuer diese Operation ueberschritten, bitte in {retry_after}s erneut versuchen"
            ));
        }
    }

    match state.ausfuehren(command, cmd_session).await {
        Ok(resp) => format_tcp_response(resp),
        Err(e) => fehler_antwort_tcp(e.fehler_code(), &e.to_string()),
    }
}

/// Formatiert eine Response als TCP-Antwortzeile
fn format_tcp_response(resp: crate::commands::types::Response) -> String {
    use crate::commands::types::Response;
    match resp {
        Response::Ok => ok_antwort(&[]),
        Response::ServerInfo(info) => ok_antwort(&[
            ("name", &info.name),
            ("version", &info.version),
            ("clients", &info.aktuelle_clients.to_string()),
            ("maxclients", &info.max_clients.to_string()),
            ("uptime", &info.uptime_secs.to_string()),
        ]),
        Response::KanalListe(kanaele) => {
            if kanaele.is_empty() {
                return ok_antwort(&[]);
            }
            let eintraege: Vec<String> = kanaele
                .iter()
                .map(|k| {
                    format!(
                        "cid={}\\sname={}",
                        k.id,
                        crate::tcp::parser::encode_value(&k.name)
                    )
                })
                .collect();
            format!("ok {}\n", eintraege.join("|"))
        }
        Response::ClientListe(clients) => {
            if clients.is_empty() {
                return ok_antwort(&[]);
            }
            let eintraege: Vec<String> = clients
                .iter()
                .map(|c| {
                    format!(
                        "clid={} clname={}",
                        c.user_id,
                        crate::tcp::parser::encode_value(&c.username)
                    )
                })
                .collect();
            format!("ok {}\n", eintraege.join("|"))
        }
        other => match serde_json::to_string(&other) {
            Ok(json) => format!("ok data={}\n", crate::tcp::parser::encode_value(&json)),
            Err(_) => ok_antwort(&[]),
        },
    }
}

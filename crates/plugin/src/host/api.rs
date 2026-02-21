//! Host-API Funktionen die WASM-Plugins aufrufen koennen
//!
//! Diese Funktionen werden als WASM-Imports bereitgestellt und
//! ermöglichen Plugins die Interaktion mit dem Speakeasy-System.

use tracing::{debug, warn};

/// Log-Level fuer speakeasy_log
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl LogLevel {
    pub fn from_i32(v: i32) -> Self {
        match v {
            0 => Self::Trace,
            1 => Self::Debug,
            2 => Self::Info,
            3 => Self::Warn,
            4 => Self::Error,
            _ => Self::Info,
        }
    }
}

/// Ergebnis eines Host-API Aufrufs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiErgebnis {
    /// Aufruf erfolgreich
    Ok,
    /// Aufruf abgelehnt (fehlende Capability)
    ZugriffVerweigert,
    /// Ungueltige Parameter
    UngueltigeParameter,
    /// Interner Fehler
    Fehler(String),
}

impl ApiErgebnis {
    /// Konvertiert Ergebnis in i32 fuer WASM-Rueckgabe
    pub fn als_i32(&self) -> i32 {
        match self {
            Self::Ok => 0,
            Self::ZugriffVerweigert => -1,
            Self::UngueltigeParameter => -2,
            Self::Fehler(_) => -3,
        }
    }
}

/// Kontext fuer Host-API Aufrufe – enthaelt Plugin-Name und Capabilities
#[derive(Debug, Clone)]
pub struct ApiKontext {
    pub plugin_name: String,
    pub chat_read: bool,
    pub chat_write: bool,
    pub user_management: bool,
    pub server_config: bool,
    pub network: bool,
}

impl ApiKontext {
    pub fn neu(plugin_name: impl Into<String>) -> Self {
        Self {
            plugin_name: plugin_name.into(),
            chat_read: false,
            chat_write: false,
            user_management: false,
            server_config: false,
            network: false,
        }
    }
}

/// Verarbeitet einen speakeasy_log Aufruf vom Plugin
pub fn host_log(kontext: &ApiKontext, level: i32, nachricht: &str) {
    let lvl = LogLevel::from_i32(level);
    match lvl {
        LogLevel::Trace | LogLevel::Debug => {
            debug!(plugin = %kontext.plugin_name, "[Plugin] {}", nachricht)
        }
        LogLevel::Info => {
            tracing::info!(plugin = %kontext.plugin_name, "[Plugin] {}", nachricht)
        }
        LogLevel::Warn => {
            warn!(plugin = %kontext.plugin_name, "[Plugin] {}", nachricht)
        }
        LogLevel::Error => {
            tracing::error!(plugin = %kontext.plugin_name, "[Plugin] {}", nachricht)
        }
    }
}

/// Verarbeitet einen speakeasy_send_message Aufruf
pub fn host_send_message(kontext: &ApiKontext, _channel_id: &str, _nachricht: &str) -> ApiErgebnis {
    if !kontext.chat_write {
        warn!(
            plugin = %kontext.plugin_name,
            "Zugriff verweigert: chat_write nicht aktiviert"
        );
        return ApiErgebnis::ZugriffVerweigert;
    }
    // In einer echten Implementierung wuerde hier die Nachricht
    // ueber den Event-Bus gesendet werden
    debug!(
        plugin = %kontext.plugin_name,
        "send_message aufgerufen"
    );
    ApiErgebnis::Ok
}

/// Verarbeitet einen speakeasy_get_user_info Aufruf
pub fn host_get_user_info(kontext: &ApiKontext, _user_id: &str) -> ApiErgebnis {
    if !kontext.chat_read && !kontext.user_management {
        return ApiErgebnis::ZugriffVerweigert;
    }
    ApiErgebnis::Ok
}

/// Verarbeitet einen speakeasy_kick_user Aufruf
pub fn host_kick_user(kontext: &ApiKontext, _user_id: &str, _grund: &str) -> ApiErgebnis {
    if !kontext.user_management {
        warn!(
            plugin = %kontext.plugin_name,
            "Zugriff verweigert: user_management nicht aktiviert"
        );
        return ApiErgebnis::ZugriffVerweigert;
    }
    debug!(plugin = %kontext.plugin_name, "kick_user aufgerufen");
    ApiErgebnis::Ok
}

/// Verarbeitet einen speakeasy_move_user Aufruf
pub fn host_move_user(kontext: &ApiKontext, _user_id: &str, _channel_id: &str) -> ApiErgebnis {
    if !kontext.user_management {
        return ApiErgebnis::ZugriffVerweigert;
    }
    ApiErgebnis::Ok
}

/// Verarbeitet einen speakeasy_get_channel_users Aufruf
pub fn host_get_channel_users(kontext: &ApiKontext, _channel_id: &str) -> ApiErgebnis {
    if !kontext.chat_read && !kontext.user_management {
        return ApiErgebnis::ZugriffVerweigert;
    }
    ApiErgebnis::Ok
}

/// Verarbeitet einen speakeasy_read_config Aufruf
pub fn host_read_config(kontext: &ApiKontext, _schluessel: &str) -> ApiErgebnis {
    if !kontext.server_config {
        return ApiErgebnis::ZugriffVerweigert;
    }
    ApiErgebnis::Ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat_kontext() -> ApiKontext {
        ApiKontext {
            plugin_name: "test-plugin".into(),
            chat_read: true,
            chat_write: true,
            user_management: false,
            server_config: false,
            network: false,
        }
    }

    fn admin_kontext() -> ApiKontext {
        ApiKontext {
            plugin_name: "admin-plugin".into(),
            chat_read: true,
            chat_write: true,
            user_management: true,
            server_config: true,
            network: false,
        }
    }

    #[test]
    fn send_message_ohne_cap_verweigert() {
        let mut k = chat_kontext();
        k.chat_write = false;
        let r = host_send_message(&k, "channel-1", "Hallo");
        assert_eq!(r, ApiErgebnis::ZugriffVerweigert);
    }

    #[test]
    fn send_message_mit_cap_ok() {
        let k = chat_kontext();
        let r = host_send_message(&k, "channel-1", "Hallo");
        assert_eq!(r, ApiErgebnis::Ok);
    }

    #[test]
    fn kick_user_ohne_cap_verweigert() {
        let k = chat_kontext();
        let r = host_kick_user(&k, "user-1", "Spam");
        assert_eq!(r, ApiErgebnis::ZugriffVerweigert);
    }

    #[test]
    fn kick_user_mit_cap_ok() {
        let k = admin_kontext();
        let r = host_kick_user(&k, "user-1", "Spam");
        assert_eq!(r, ApiErgebnis::Ok);
    }

    #[test]
    fn read_config_ohne_cap_verweigert() {
        let k = chat_kontext();
        let r = host_read_config(&k, "max_users");
        assert_eq!(r, ApiErgebnis::ZugriffVerweigert);
    }

    #[test]
    fn read_config_mit_cap_ok() {
        let k = admin_kontext();
        let r = host_read_config(&k, "max_users");
        assert_eq!(r, ApiErgebnis::Ok);
    }

    #[test]
    fn api_ergebnis_als_i32() {
        assert_eq!(ApiErgebnis::Ok.als_i32(), 0);
        assert_eq!(ApiErgebnis::ZugriffVerweigert.als_i32(), -1);
        assert_eq!(ApiErgebnis::UngueltigeParameter.als_i32(), -2);
        assert_eq!(ApiErgebnis::Fehler("x".into()).als_i32(), -3);
    }

    #[test]
    fn log_level_from_i32() {
        assert!(matches!(LogLevel::from_i32(0), LogLevel::Trace));
        assert!(matches!(LogLevel::from_i32(3), LogLevel::Warn));
        assert!(matches!(LogLevel::from_i32(99), LogLevel::Info));
    }
}

//! wasmtime Runtime Setup und Engine-Konfiguration

use anyhow::Context;
use tracing::{debug, info};
use wasmtime::{Engine, Module, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};

use crate::error::{PluginError, Result};
use crate::host::sandbox::SandboxKonfiguration;

/// Globale wasmtime Engine (wiederverwendbar, thread-safe)
pub struct PluginEngine {
    engine: Engine,
}

impl PluginEngine {
    /// Erstellt eine neue Engine mit optimierten Einstellungen
    pub fn neu() -> Result<Self> {
        let mut config = wasmtime::Config::new();
        config.async_support(true);
        // Fuel-basierte CPU-Begrenzung aktivieren
        config.consume_fuel(true);

        let engine = Engine::new(&config)
            .map_err(|e| PluginError::Intern(format!("Engine-Erstellung fehlgeschlagen: {}", e)))?;

        info!("wasmtime Engine initialisiert");
        Ok(Self { engine })
    }

    /// Gibt Referenz auf die interne Engine zurueck
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Kompiliert WASM-Bytecode zu einem Modul
    pub fn kompilieren(&self, wasm_bytes: &[u8]) -> Result<Module> {
        debug!("Kompiliere WASM-Modul ({} Bytes)", wasm_bytes.len());
        Module::new(&self.engine, wasm_bytes)
            .map_err(|e| PluginError::WasmKompilierung(e.to_string()))
    }
}

/// Erstellt einen WASI-Kontext basierend auf der Sandbox-Konfiguration
pub fn wasi_kontext_erstellen(sandbox: &SandboxKonfiguration) -> anyhow::Result<WasiCtx> {
    let mut builder = WasiCtxBuilder::new();

    // Stdio immer erlauben (fuer Plugin-Logging)
    if sandbox.stdio {
        builder.inherit_stdio();
    }

    // Dateisystem nur bei expliziter Capability
    // (Kein predir hinzufuegen = kein FS-Zugriff)
    if sandbox.filesystem {
        debug!("Plugin hat Dateisystem-Capability – FS-Zugriff aktiviert");
    }

    Ok(builder.build())
}

/// Host-Daten fuer den Store: WASI-Kontext + Speicherlimiter
pub struct HostDaten {
    pub wasi: WasiCtx,
    pub(crate) limiter: SpeicherLimiter,
}

/// Erstellt einen Store fuer ein Plugin mit Sandbox-Grenzen
pub fn store_erstellen(
    engine: &Engine,
    sandbox: &SandboxKonfiguration,
    wasi: WasiCtx,
) -> anyhow::Result<Store<HostDaten>> {
    let host = HostDaten {
        wasi,
        limiter: SpeicherLimiter {
            max_bytes: sandbox.max_speicher_bytes,
        },
    };
    let mut store = Store::new(engine, host);

    // Speicherlimit setzen – Referenz auf Feld in Host-Daten
    store.limiter(|host| &mut host.limiter);

    // CPU-Fuel setzen (0 = unbegrenzt -> hoher Wert)
    let fuel = if sandbox.max_instruktionen == 0 {
        u64::MAX / 2
    } else {
        sandbox.max_instruktionen
    };
    store.set_fuel(fuel).context("Fuel setzen fehlgeschlagen")?;

    Ok(store)
}

/// Speicherlimiter fuer WASM-Instanzen
pub(crate) struct SpeicherLimiter {
    max_bytes: u64,
}

impl wasmtime::ResourceLimiter for SpeicherLimiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        if desired as u64 > self.max_bytes {
            tracing::warn!(
                "Plugin ueberschreitet Speicherlimit: {} > {}",
                desired,
                self.max_bytes
            );
            return Ok(false);
        }
        let _ = current;
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        _desired: usize,
        _maximum: Option<usize>,
    ) -> anyhow::Result<bool> {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasmtime::ResourceLimiter;

    #[test]
    fn engine_erstellen() {
        let engine = PluginEngine::neu();
        assert!(engine.is_ok(), "Engine muss erstellt werden koennen");
    }

    #[test]
    fn wasm_bytes_ungueltig() {
        let engine = PluginEngine::neu().unwrap();
        // Ungueltige WASM-Bytes
        let err = engine.kompilieren(b"das ist kein wasm").unwrap_err();
        assert!(matches!(err, PluginError::WasmKompilierung(_)));
    }

    #[test]
    fn wasi_kontext_minimal() {
        let sandbox = SandboxKonfiguration::minimal();
        let ctx = wasi_kontext_erstellen(&sandbox);
        assert!(ctx.is_ok());
    }

    #[test]
    fn speicher_limiter_pruefung() {
        let mut limiter = SpeicherLimiter { max_bytes: 1024 };
        // Innerhalb Limit
        assert!(limiter.memory_growing(0, 512, None).unwrap());
        // Ueber Limit
        assert!(!limiter.memory_growing(0, 2048, None).unwrap());
    }
}

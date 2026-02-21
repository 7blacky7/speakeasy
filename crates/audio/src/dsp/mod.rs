//! DSP-Module fuer Audio-Verarbeitung
//!
//! Alle Module implementieren das `AudioProcessor` Trait fuer
//! eine einheitliche Pipeline-Integration.

pub mod agc;
pub mod deesser;
pub mod echo_cancel;
pub mod noise_gate;
pub mod noise_suppression;
pub mod vad;

/// Gemeinsames Trait fuer alle Audio-Prozessoren
///
/// Alle DSP-Bausteine verarbeiten Samples in-place und sind
/// Send + Sync fuer Thread-sichere Pipeline-Nutzung.
pub trait AudioProcessor: Send + Sync {
    /// Verarbeitet einen Puffer von Samples in-place
    fn process(&mut self, samples: &mut [f32]);

    /// Setzt den internen Zustand zurueck (z.B. Filter-Historie)
    fn reset(&mut self);

    /// Gibt zurueck ob der Prozessor aktiv ist
    fn is_enabled(&self) -> bool;

    /// Aktiviert oder deaktiviert den Prozessor
    fn set_enabled(&mut self, enabled: bool);
}

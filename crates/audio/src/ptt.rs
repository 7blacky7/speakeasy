//! Push-to-Talk Steuerung
//!
//! Unterstuetzt drei Modi: Hold (Taste halten), Toggle (Taste umschalten),
//! VoiceActivation (automatisch via VAD).

/// Betriebsmodus fuer Push-to-Talk
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PttMode {
    /// Taste halten um zu senden – loslassen stoppt Uebertragung
    Hold,
    /// Einmal druecken zum Aktivieren, nochmals zum Deaktivieren
    Toggle,
    /// Automatische Aktivierung per Voice Activity Detection
    #[default]
    VoiceActivation,
}

/// Push-to-Talk Controller
pub struct PttController {
    mode: PttMode,
    /// Hold-Modus: Taste aktuell gedrueckt?
    key_held: bool,
    /// Toggle-Modus: Aktuell aktiv?
    toggle_active: bool,
    /// VAD-Modus: Sprache erkannt?
    vad_active: bool,
    /// Globales Mute (ueberschreibt alles)
    muted: bool,
}

impl PttController {
    pub fn new(mode: PttMode) -> Self {
        Self {
            mode,
            key_held: false,
            toggle_active: false,
            vad_active: false,
            muted: false,
        }
    }

    /// Taste gedrueckt (fuer Hold-Modus)
    pub fn key_down(&mut self) {
        self.key_held = true;
    }

    /// Taste losgelassen (fuer Hold-Modus)
    pub fn key_up(&mut self) {
        self.key_held = false;
    }

    /// Umschalten (fuer Toggle-Modus)
    pub fn toggle(&mut self) {
        self.toggle_active = !self.toggle_active;
    }

    /// VAD-Status setzen (fuer VoiceActivation-Modus)
    pub fn set_vad_active(&mut self, active: bool) {
        self.vad_active = active;
    }

    /// Modus wechseln – setzt alle Zustaende zurueck
    pub fn set_mode(&mut self, mode: PttMode) {
        self.mode = mode;
        self.key_held = false;
        self.toggle_active = false;
        self.vad_active = false;
    }

    /// Globales Mute setzen
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Gibt zurueck ob aktuell gesendet wird
    pub fn is_transmitting(&self) -> bool {
        if self.muted {
            return false;
        }
        match self.mode {
            PttMode::Hold => self.key_held,
            PttMode::Toggle => self.toggle_active,
            PttMode::VoiceActivation => self.vad_active,
        }
    }

    /// Gibt den aktuellen Modus zurueck
    pub fn mode(&self) -> PttMode {
        self.mode
    }

    /// Gibt zurueck ob global gemutet
    pub fn is_muted(&self) -> bool {
        self.muted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ptt_hold_sendet_nur_waehrend_taste_gedrueckt() {
        let mut ptt = PttController::new(PttMode::Hold);
        assert!(!ptt.is_transmitting());
        ptt.key_down();
        assert!(ptt.is_transmitting());
        ptt.key_up();
        assert!(!ptt.is_transmitting());
    }

    #[test]
    fn ptt_toggle_wechselt_zustand() {
        let mut ptt = PttController::new(PttMode::Toggle);
        assert!(!ptt.is_transmitting());
        ptt.toggle();
        assert!(ptt.is_transmitting());
        ptt.toggle();
        assert!(!ptt.is_transmitting());
    }

    #[test]
    fn ptt_vad_folgt_sprachaktivitaet() {
        let mut ptt = PttController::new(PttMode::VoiceActivation);
        assert!(!ptt.is_transmitting());
        ptt.set_vad_active(true);
        assert!(ptt.is_transmitting());
        ptt.set_vad_active(false);
        assert!(!ptt.is_transmitting());
    }

    #[test]
    fn ptt_mute_verhindert_sendung() {
        let mut ptt = PttController::new(PttMode::Hold);
        ptt.key_down();
        assert!(ptt.is_transmitting());
        ptt.set_muted(true);
        assert!(!ptt.is_transmitting(), "Mute sollte Sendung verhindern");
    }

    #[test]
    fn ptt_modus_wechsel_setzt_zustand_zurueck() {
        let mut ptt = PttController::new(PttMode::Hold);
        ptt.key_down();
        assert!(ptt.is_transmitting());
        ptt.set_mode(PttMode::Toggle);
        assert!(
            !ptt.is_transmitting(),
            "Nach Moduswechsel sollte nichts aktiv sein"
        );
        assert!(!ptt.key_held);
    }

    #[test]
    fn ptt_toggle_unabhaengig_von_key() {
        let mut ptt = PttController::new(PttMode::Toggle);
        ptt.key_down(); // Hat im Toggle-Modus keinen Effekt
        assert!(!ptt.is_transmitting());
        ptt.toggle();
        assert!(ptt.is_transmitting());
    }

    #[test]
    fn ptt_default_modus_vad() {
        let ptt = PttController::new(PttMode::default());
        assert_eq!(ptt.mode(), PttMode::VoiceActivation);
    }

    #[test]
    fn ptt_mute_toggle_kombination() {
        let mut ptt = PttController::new(PttMode::Toggle);
        ptt.toggle();
        ptt.set_muted(true);
        assert!(!ptt.is_transmitting());
        ptt.set_muted(false);
        assert!(ptt.is_transmitting());
    }
}

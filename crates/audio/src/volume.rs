//! Per-User Lautstaerke-Kontrolle
//!
//! Verwaltet Master-Lautstaerke, per-User Lautstaerke, Muting
//! und sanfte Lautstaerke-Uebergaenge (keine Klicks).

use speakeasy_core::types::UserId;
use std::collections::HashMap;

/// Lautstaerke-Kontroller fuer Playback-Mixing
pub struct VolumeController {
    /// Master-Lautstaerke (0.0..2.0, 1.0 = normal)
    master_volume: f32,
    /// Ziel-Master-Lautstaerke (fuer sanfte Uebergaenge)
    master_target: f32,
    /// Globales Mute
    master_muted: bool,
    /// Per-User Lautstaerke
    user_volumes: HashMap<UserId, f32>,
    /// Per-User Ziel-Lautstaerke
    user_targets: HashMap<UserId, f32>,
    /// Per-User Mute
    user_muted: HashMap<UserId, bool>,
    /// Glaettungskoeffizient fuer Lautstaerke-Uebergaenge
    smoothing: f32,
}

impl VolumeController {
    /// Erstellt einen neuen VolumeController
    pub fn new() -> Self {
        Self {
            master_volume: 1.0,
            master_target: 1.0,
            master_muted: false,
            user_volumes: HashMap::new(),
            user_targets: HashMap::new(),
            user_muted: HashMap::new(),
            smoothing: 0.995,
        }
    }

    /// Setzt die Master-Lautstaerke (sanfter Uebergang)
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_target = volume.clamp(0.0, 2.0);
    }

    /// Gibt die aktuelle Master-Lautstaerke zurueck
    pub fn master_volume(&self) -> f32 {
        self.master_volume
    }

    /// Master Mute/Unmute
    pub fn set_master_muted(&mut self, muted: bool) {
        self.master_muted = muted;
    }

    /// Gibt zurueck ob Master gemutet ist
    pub fn is_master_muted(&self) -> bool {
        self.master_muted
    }

    /// Setzt die Lautstaerke fuer einen bestimmten User (sanfter Uebergang)
    pub fn set_user_volume(&mut self, user: UserId, volume: f32) {
        let v = volume.clamp(0.0, 2.0);
        self.user_targets.insert(user, v);
        // Falls noch kein Eintrag: direkt setzen (kein Klick beim ersten Mal)
        self.user_volumes.entry(user).or_insert(v);
    }

    /// Gibt die aktuelle Lautstaerke eines Users zurueck (1.0 wenn unbekannt)
    pub fn user_volume(&self, user: UserId) -> f32 {
        *self.user_volumes.get(&user).unwrap_or(&1.0)
    }

    /// Mutet/Unmutet einen bestimmten User
    pub fn set_user_muted(&mut self, user: UserId, muted: bool) {
        self.user_muted.insert(user, muted);
    }

    /// Gibt zurueck ob ein User gemutet ist
    pub fn is_user_muted(&self, user: UserId) -> bool {
        *self.user_muted.get(&user).unwrap_or(&false)
    }

    /// Entfernt einen User (z.B. wenn er den Kanal verlaesst)
    pub fn remove_user(&mut self, user: UserId) {
        self.user_volumes.remove(&user);
        self.user_targets.remove(&user);
        self.user_muted.remove(&user);
    }

    /// Wendet Lautstaerke auf einen Sample-Buffer eines bestimmten Users an.
    /// Aktualisiert dabei die Lautstaerke-Glaettung.
    pub fn apply(&mut self, user: UserId, samples: &mut [f32]) {
        // Master-Lautstaerke glaetten
        self.master_volume =
            self.smoothing * self.master_volume + (1.0 - self.smoothing) * self.master_target;

        let effective_master = if self.master_muted {
            0.0
        } else {
            self.master_volume
        };

        // User-Lautstaerke glaetten
        let target = *self.user_targets.get(&user).unwrap_or(&1.0);
        let current = self.user_volumes.entry(user).or_insert(target);
        *current = self.smoothing * *current + (1.0 - self.smoothing) * target;
        let user_vol = *current;

        let user_muted = *self.user_muted.get(&user).unwrap_or(&false);
        let effective_user = if user_muted { 0.0 } else { user_vol };

        let gain = effective_master * effective_user;
        for s in samples.iter_mut() {
            *s *= gain;
        }
    }

    /// Mischt mehrere User-Streams zusammen (additive Mischung mit Normalisierung)
    pub fn mix_users(&mut self, streams: &mut HashMap<UserId, Vec<f32>>, output: &mut [f32]) {
        output.fill(0.0);

        for (&user, samples) in streams.iter_mut() {
            self.apply(user, samples);
            for (out, s) in output.iter_mut().zip(samples.iter()) {
                *out += s;
            }
        }

        // Soft Clip um Clipping bei vielen gleichzeitigen Sprechern zu vermeiden
        for s in output.iter_mut() {
            *s = soft_clip(*s);
        }
    }
}

impl Default for VolumeController {
    fn default() -> Self {
        Self::new()
    }
}

/// Sanfter Clipper (tanh-basiert)
fn soft_clip(x: f32) -> f32 {
    x.tanh()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(n: u64) -> UserId {
        use uuid::Uuid;
        UserId(Uuid::from_u128(n as u128))
    }

    #[test]
    fn volume_master_default_eins() {
        let vc = VolumeController::new();
        assert!((vc.master_volume() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn volume_master_mute() {
        let mut vc = VolumeController::new();
        let u = user(1);
        vc.set_master_muted(true);
        let mut samples = vec![1.0f32; 480];
        vc.apply(u, &mut samples);
        assert!(
            samples.iter().all(|&s| s == 0.0),
            "Master Mute sollte alles auf 0 setzen"
        );
    }

    #[test]
    fn volume_user_mute() {
        let mut vc = VolumeController::new();
        let u = user(2);
        vc.set_user_muted(u, true);
        let mut samples = vec![1.0f32; 480];
        vc.apply(u, &mut samples);
        assert!(
            samples.iter().all(|&s| s == 0.0),
            "User Mute sollte auf 0 setzen"
        );
    }

    #[test]
    fn volume_user_volume_skaliert() {
        let mut vc = VolumeController::new();
        let u = user(3);
        // Direkt setzen ohne Glaettung: smoothing auf 0 simulieren
        vc.smoothing = 0.0;
        vc.set_user_volume(u, 0.5);
        let mut samples = vec![1.0f32; 4];
        vc.apply(u, &mut samples);
        // Bei smoothing=0: sofort auf Zielwert
        for s in &samples {
            assert!(
                (*s - 0.5).abs() < 0.01,
                "Lautstaerke 0.5 erwartet, war {}",
                s
            );
        }
    }

    #[test]
    fn volume_user_entfernen() {
        let mut vc = VolumeController::new();
        let u = user(4);
        vc.set_user_volume(u, 0.3);
        vc.remove_user(u);
        assert!(!vc.user_volumes.contains_key(&u));
    }

    #[test]
    fn volume_clamp_max() {
        let mut vc = VolumeController::new();
        vc.set_master_volume(99.0); // Sollte auf 2.0 geclamped werden
        assert!((vc.master_target - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn volume_clamp_min() {
        let mut vc = VolumeController::new();
        vc.set_master_volume(-5.0);
        assert!((vc.master_target - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn volume_soft_clip_begrenzt() {
        // tanh naehert sich 1.0 an – bei sehr grossen Werten ist f32 == 1.0
        // Wichtig: Wert darf nie ueber 1.0 gehen
        assert!(soft_clip(100.0) <= 1.0);
        assert!(soft_clip(-100.0) >= -1.0);
        assert!((soft_clip(0.0)).abs() < f32::EPSILON);
        // Bei moderaten Werten ist tanh echt kleiner als 1.0
        assert!(soft_clip(2.0) < 1.0);
        assert!(soft_clip(-2.0) > -1.0);
    }

    #[test]
    fn volume_mix_users_addiert() {
        let mut vc = VolumeController::new();
        vc.smoothing = 0.0; // sofortige Reaktion
        let u1 = user(10);
        let u2 = user(11);
        let mut streams = HashMap::new();
        streams.insert(u1, vec![0.1f32; 4]);
        streams.insert(u2, vec![0.1f32; 4]);
        let mut output = vec![0.0f32; 4];
        vc.mix_users(&mut streams, &mut output);
        // Beide Streams addiert, dann soft_clip
        for s in &output {
            assert!(*s > 0.0, "Gemischtes Signal sollte positiv sein");
        }
    }

    #[test]
    fn volume_user_volume_abfrage_unbekannter_user() {
        let vc = VolumeController::new();
        assert!((vc.user_volume(user(999)) - 1.0).abs() < f32::EPSILON);
    }
}

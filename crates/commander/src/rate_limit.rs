//! Rate Limiter fuer den Speakeasy Commander
//!
//! Implementiert den Token-Bucket-Algorithmus fuer Rate Limiting
//! pro IP-Adresse und pro API-Token.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use parking_lot::Mutex;

/// Konfiguration fuer den Rate Limiter
#[derive(Debug, Clone)]
pub struct RateLimitKonfig {
    /// Maximale Anfragen pro Minute pro IP
    pub anfragen_pro_minute_ip: u32,
    /// Maximale Anfragen pro Minute pro Token
    pub anfragen_pro_minute_token: u32,
    /// Maximale "teure" Anfragen pro Minute (Ban, Permission-Aenderungen)
    pub teure_anfragen_pro_minute: u32,
}

impl Default for RateLimitKonfig {
    fn default() -> Self {
        Self {
            anfragen_pro_minute_ip: 100,
            anfragen_pro_minute_token: 200,
            teure_anfragen_pro_minute: 10,
        }
    }
}

/// Ein Token-Bucket fuer eine einzelne Entitaet (IP oder Token)
#[derive(Debug)]
struct TokenBucket {
    /// Aktuelle Token-Anzahl (als f64 fuer Bruchteil-Auffuellung)
    token: f64,
    /// Maximale Token-Anzahl (= Burst-Limit)
    max_token: f64,
    /// Auffuellrate in Token pro Sekunde
    fuellrate: f64,
    /// Letzter Zeitpunkt der Auffuellung
    letzter_auffuellung: Instant,
}

impl TokenBucket {
    fn neu(max_anfragen_pro_minute: u32) -> Self {
        let max = max_anfragen_pro_minute as f64;
        Self {
            token: max,
            max_token: max,
            fuellrate: max / 60.0,
            letzter_auffuellung: Instant::now(),
        }
    }

    /// Versucht ein Token zu verbrauchen. Gibt `true` zurueck wenn erlaubt.
    fn verbrauchen(&mut self) -> bool {
        self.auffuellen();
        if self.token >= 1.0 {
            self.token -= 1.0;
            true
        } else {
            false
        }
    }

    /// Berechnet wie viele Sekunden bis zum naechsten verfuegbaren Token
    fn retry_after_secs(&mut self) -> u64 {
        self.auffuellen();
        let fehlend = 1.0 - self.token;
        if fehlend <= 0.0 {
            return 0;
        }
        (fehlend / self.fuellrate).ceil() as u64
    }

    fn auffuellen(&mut self) {
        let jetzt = Instant::now();
        let vergangen = jetzt.duration_since(self.letzter_auffuellung).as_secs_f64();
        self.token = (self.token + vergangen * self.fuellrate).min(self.max_token);
        self.letzter_auffuellung = jetzt;
    }
}

/// Rate Limiter mit Token-Bucket-Algorithmus
///
/// Verwaltet separate Buckets fuer IP-Adressen und API-Tokens.
pub struct RateLimiter {
    konfig: RateLimitKonfig,
    ip_buckets: Mutex<HashMap<String, TokenBucket>>,
    token_buckets: Mutex<HashMap<String, TokenBucket>>,
    teure_ip_buckets: Mutex<HashMap<String, TokenBucket>>,
}

impl RateLimiter {
    pub fn neu(konfig: RateLimitKonfig) -> Arc<Self> {
        Arc::new(Self {
            konfig,
            ip_buckets: Mutex::new(HashMap::new()),
            token_buckets: Mutex::new(HashMap::new()),
            teure_ip_buckets: Mutex::new(HashMap::new()),
        })
    }

    /// Prueft und verbraucht ein Token fuer eine IP-Adresse.
    ///
    /// Gibt `Ok(())` zurueck wenn erlaubt, `Err(retry_after_secs)` sonst.
    pub fn pruefe_ip(&self, ip: &str) -> Result<(), u64> {
        let mut buckets = self.ip_buckets.lock();
        let bucket = buckets
            .entry(ip.to_string())
            .or_insert_with(|| TokenBucket::neu(self.konfig.anfragen_pro_minute_ip));
        if bucket.verbrauchen() {
            Ok(())
        } else {
            Err(bucket.retry_after_secs())
        }
    }

    /// Prueft und verbraucht ein Token fuer einen API-Token.
    pub fn pruefe_token(&self, token_id: &str) -> Result<(), u64> {
        let mut buckets = self.token_buckets.lock();
        let bucket = buckets
            .entry(token_id.to_string())
            .or_insert_with(|| TokenBucket::neu(self.konfig.anfragen_pro_minute_token));
        if bucket.verbrauchen() {
            Ok(())
        } else {
            Err(bucket.retry_after_secs())
        }
    }

    /// Prueft das Limit fuer "teure" Operationen (Ban, Permission-Aenderungen) per IP.
    pub fn pruefe_teure_operation(&self, ip: &str) -> Result<(), u64> {
        let mut buckets = self.teure_ip_buckets.lock();
        let bucket = buckets
            .entry(ip.to_string())
            .or_insert_with(|| TokenBucket::neu(self.konfig.teure_anfragen_pro_minute));
        if bucket.verbrauchen() {
            Ok(())
        } else {
            Err(bucket.retry_after_secs())
        }
    }

    /// Bereinigt Buckets die seit mehr als 5 Minuten inaktiv sind (Speicher-Management).
    pub fn cleanup(&self) {
        let schwellwert = Duration::from_secs(5 * 60);
        let jetzt = Instant::now();

        let mut ip = self.ip_buckets.lock();
        ip.retain(|_, b| jetzt.duration_since(b.letzter_auffuellung) < schwellwert);

        let mut tok = self.token_buckets.lock();
        tok.retain(|_, b| jetzt.duration_since(b.letzter_auffuellung) < schwellwert);

        let mut teuer = self.teure_ip_buckets.lock();
        teuer.retain(|_, b| jetzt.duration_since(b.letzter_auffuellung) < schwellwert);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_bucket_erlaubt_anfragen_bis_limit() {
        let mut bucket = TokenBucket::neu(5);
        // 5 Anfragen sollten durchgehen
        for _ in 0..5 {
            assert!(bucket.verbrauchen(), "Anfrage sollte erlaubt sein");
        }
        // 6. Anfrage sollte abgelehnt werden
        assert!(!bucket.verbrauchen(), "6. Anfrage sollte abgelehnt werden");
    }

    #[test]
    fn rate_limiter_ip_pruefung() {
        let limiter = RateLimiter::neu(RateLimitKonfig {
            anfragen_pro_minute_ip: 3,
            anfragen_pro_minute_token: 200,
            teure_anfragen_pro_minute: 10,
        });

        assert!(limiter.pruefe_ip("127.0.0.1").is_ok());
        assert!(limiter.pruefe_ip("127.0.0.1").is_ok());
        assert!(limiter.pruefe_ip("127.0.0.1").is_ok());
        assert!(limiter.pruefe_ip("127.0.0.1").is_err());
    }

    #[test]
    fn rate_limiter_verschiedene_ips_unabhaengig() {
        let limiter = RateLimiter::neu(RateLimitKonfig {
            anfragen_pro_minute_ip: 1,
            anfragen_pro_minute_token: 200,
            teure_anfragen_pro_minute: 10,
        });

        assert!(limiter.pruefe_ip("192.168.1.1").is_ok());
        assert!(limiter.pruefe_ip("192.168.1.2").is_ok()); // andere IP
        assert!(limiter.pruefe_ip("192.168.1.1").is_err()); // erste IP erschoepft
    }

    #[test]
    fn rate_limiter_token_pruefung() {
        let limiter = RateLimiter::neu(RateLimitKonfig {
            anfragen_pro_minute_ip: 100,
            anfragen_pro_minute_token: 2,
            teure_anfragen_pro_minute: 10,
        });

        assert!(limiter.pruefe_token("token-abc").is_ok());
        assert!(limiter.pruefe_token("token-abc").is_ok());
        let ergebnis = limiter.pruefe_token("token-abc");
        assert!(ergebnis.is_err());
        assert!(ergebnis.unwrap_err() > 0);
    }

    #[test]
    fn rate_limiter_teure_operation() {
        let limiter = RateLimiter::neu(RateLimitKonfig {
            anfragen_pro_minute_ip: 100,
            anfragen_pro_minute_token: 200,
            teure_anfragen_pro_minute: 2,
        });

        assert!(limiter.pruefe_teure_operation("10.0.0.1").is_ok());
        assert!(limiter.pruefe_teure_operation("10.0.0.1").is_ok());
        assert!(limiter.pruefe_teure_operation("10.0.0.1").is_err());
    }

    #[test]
    fn token_bucket_auffuellung_nach_zeit() {
        // Bucket mit 60 Anfragen/Minute = 1/Sekunde
        let mut bucket = TokenBucket::neu(60);
        // Alle Token verbrauchen
        for _ in 0..60 {
            bucket.verbrauchen();
        }
        // Zeit simulieren: letzter_auffuellung in der Vergangenheit setzen
        bucket.letzter_auffuellung = Instant::now() - Duration::from_secs(2);
        // Jetzt sollten ~2 neue Token verfuegbar sein
        assert!(
            bucket.verbrauchen(),
            "Nach 2 Sekunden sollte 1 Token verfuegbar sein"
        );
    }
}

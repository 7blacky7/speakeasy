// Verhindert ein zusaetzliches Konsolenfenster unter Windows im Release-Modus
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    speakeasy_client_lib::run();
}

use crate::settings;

#[tauri::command]
pub fn load_settings() -> settings::Settings {
    settings::load()
}

#[tauri::command]
pub fn save_settings(settings: settings::Settings) -> Result<(), String> {
    settings::save(&settings)
}

#[tauri::command]
pub fn set_loop_enabled(enabled: bool) -> Result<(), String> {
    let mut s = settings::load();
    s.loop_enabled = enabled;
    settings::save(&s)
}

#[tauri::command]
pub fn get_loop_enabled() -> bool {
    settings::load().loop_enabled
}

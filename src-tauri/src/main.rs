#![cfg_attr(windows, windows_subsystem = "windows")]

mod hardware;
mod lighting;
mod profiles;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, State, WindowEvent, Wry,
};

use hardware::Device;
use lighting::{ApplyResult, Assignment, Lighting};
use profiles::{Profile, Profiles};

struct TrayMenu {
    open: MenuItem<Wry>,
    exit: MenuItem<Wry>,
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn setup_tray(app: &AppHandle) -> tauri::Result<TrayMenu> {
    let open = MenuItem::with_id(app, "open", "Open VibeRGB", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", "Exit VibeRGB", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &exit])?;
    let icon = app.default_window_icon().expect("VibeRGB icon").clone();
    TrayIconBuilder::with_id("vibergb")
        .icon(icon)
        .tooltip("VibeRGB")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_window(app),
            "exit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(TrayMenu { open, exit })
}

#[tauri::command]
fn set_language(menu: State<'_, TrayMenu>, language: &str) -> Result<(), String> {
    let (open, exit) = if language == "ru" {
        ("Открыть VibeRGB", "Выйти из VibeRGB")
    } else {
        ("Open VibeRGB", "Exit VibeRGB")
    };
    menu.open
        .set_text(open)
        .and_then(|()| menu.exit.set_text(exit))
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn devices(lighting: State<'_, Lighting>, refresh: bool) -> Result<Vec<Device>, String> {
    lighting.devices(refresh).await
}

#[tauri::command]
async fn apply_lighting(
    lighting: State<'_, Lighting>,
    assignments: Vec<Assignment>,
) -> Result<ApplyResult, String> {
    lighting.apply(assignments).await
}

#[tauri::command]
fn list_profiles(profiles: State<'_, Profiles>) -> Result<Vec<Profile>, String> {
    profiles.list()
}

#[tauri::command]
fn save_profile(
    profiles: State<'_, Profiles>,
    name: String,
    assignments: Vec<Assignment>,
) -> Result<Vec<Profile>, String> {
    profiles.save(name, assignments)
}

#[tauri::command]
fn rename_profile(
    profiles: State<'_, Profiles>,
    id: String,
    name: String,
) -> Result<Vec<Profile>, String> {
    profiles.rename(&id, name)
}

#[tauri::command]
fn delete_profile(profiles: State<'_, Profiles>, id: String) -> Result<Vec<Profile>, String> {
    profiles.delete(&id)
}

#[tauri::command]
async fn apply_profile(
    lighting: State<'_, Lighting>,
    profiles: State<'_, Profiles>,
    id: String,
) -> Result<ApplyResult, String> {
    let profile = profiles
        .list()?
        .into_iter()
        .find(|profile| profile.id == id)
        .ok_or("Profile not found.")?;
    lighting.apply(profile.assignments).await
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_window(app)
        }))
        .setup(|app| {
            let config = app.path().app_config_dir()?.join("profiles.json");
            app.manage(setup_tray(app.handle())?);
            app.manage(Profiles::new(config));
            app.manage(Lighting::start(app.handle().clone()));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" && window.hide().is_ok() {
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            set_language,
            devices,
            apply_lighting,
            list_profiles,
            save_profile,
            rename_profile,
            delete_profile,
            apply_profile,
        ])
        .build(tauri::generate_context!())
        .expect("failed to start VibeRGB")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<Lighting>().shutdown();
            }
        });
}

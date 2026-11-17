#![cfg_attr(windows, windows_subsystem = "windows")]

mod hardware;
mod lighting;
mod profiles;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, State, WindowEvent, Wry,
};

use lighting::{ApplyResult, Discovery, Lighting};
use profiles::{ProfileLibrary, Profiles};

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
async fn devices(lighting: State<'_, Lighting>, refresh: bool) -> Result<Discovery, String> {
    lighting.devices(refresh).await
}

#[tauri::command]
async fn install_pawnio() -> Result<&'static str, String> {
    #[cfg(windows)]
    return tauri::async_runtime::spawn_blocking(hardware::pawnio::install)
        .await
        .map_err(|error| error.to_string())?;

    #[cfg(not(windows))]
    Err("PawnIO is only available on Windows.".into())
}

#[tauri::command]
async fn restart_for_rgb_memory(app: AppHandle) -> Result<(), String> {
    #[cfg(windows)]
    if tauri::async_runtime::spawn_blocking(hardware::pawnio::restart_as_admin)
        .await
        .map_err(|error| error.to_string())??
    {
        app.exit(0);
    }
    Ok(())
}

#[tauri::command]
async fn apply_lighting(
    lighting: State<'_, Lighting>,
    assignments: Vec<lighting::Assignment>,
) -> Result<ApplyResult, String> {
    lighting.apply(assignments).await
}

#[tauri::command]
fn list_profiles(profiles: State<'_, Profiles>) -> Result<ProfileLibrary, String> {
    profiles.list()
}

#[tauri::command]
async fn save_profile(
    profiles: State<'_, Profiles>,
    lighting: State<'_, Lighting>,
    name: String,
) -> Result<ProfileLibrary, String> {
    profiles.save(name, lighting.active().await?)
}

#[tauri::command]
async fn replace_profile(profiles: State<'_, Profiles>, lighting: State<'_, Lighting>, id: String) -> Result<ProfileLibrary, String> {
    profiles.replace(&id, lighting.active().await?)
}

#[tauri::command]
fn set_startup_profile(profiles: State<'_, Profiles>, id: Option<String>) -> Result<ProfileLibrary, String> {
    profiles.set_startup(id)
}

#[tauri::command]
fn rename_profile(
    profiles: State<'_, Profiles>,
    id: String,
    name: String,
) -> Result<ProfileLibrary, String> {
    profiles.rename(&id, name)
}

#[tauri::command]
fn delete_profile(profiles: State<'_, Profiles>, id: String) -> Result<ProfileLibrary, String> {
    profiles.delete(&id)
}

#[tauri::command]
async fn apply_profile(
    lighting: State<'_, Lighting>,
    profiles: State<'_, Profiles>,
    id: String,
) -> Result<ApplyResult, String> {
    lighting.apply(profiles.assignments(&id)?).await
}

fn main() {
    #[cfg(windows)]
    if !hardware::pawnio::wait_for_previous_instance() {
        return;
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if !args.iter().any(|arg| arg == "--autostart") {
                show_window(app);
            }
        }))
        .plugin(tauri_plugin_autostart::Builder::new().arg("--autostart").build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let config = app.path().app_config_dir()?.join("profiles.json");
            app.manage(setup_tray(app.handle())?);
            if !std::env::args_os().any(|arg| arg == "--autostart") {
                show_window(app.handle());
            }
            let profiles = Profiles::new(config);
            let startup = profiles.startup();
            app.manage(profiles);
            app.manage(Lighting::start(app.handle().clone(), startup));
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
            install_pawnio,
            restart_for_rgb_memory,
            apply_lighting,
            list_profiles,
            save_profile,
            replace_profile,
            set_startup_profile,
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

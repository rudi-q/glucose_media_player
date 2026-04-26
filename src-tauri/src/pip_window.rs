use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

#[derive(Clone, Copy, Debug)]
struct WindowState {
    size: PhysicalSize<u32>,
    position: PhysicalPosition<i32>,
    decorations_enabled: bool,
    resizable: bool,
    fullscreen: bool,
    maximized: bool,
    always_on_top: bool,
    normal_min_size: PhysicalSize<u32>,
    pip_size: PhysicalSize<u32>,
}

#[derive(Clone, Copy, Debug)]
struct WorkArea {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PipWindowConfig {
    width: u32,
    height: u32,
    min_width: u32,
    min_height: u32,
    max_width: u32,
    padding: i32,
    snap_threshold: i32,
    aspect_width: u32,
    aspect_height: u32,
    normal_min_width: u32,
    normal_min_height: u32,
}

#[derive(Deserialize)]
struct AppConstants {
    #[serde(rename = "pipWindow")]
    pip_window: PipWindowConfig,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
struct PipWindowLayout {
    width: u32,
    height: u32,
    x: i32,
    y: i32,
}

static WINDOW_STATE: Mutex<Option<WindowState>> = Mutex::new(None);

#[tauri::command]
pub(crate) fn enter_pip_mode(app_handle: AppHandle) -> Result<(), String> {
    let window = main_window(&app_handle)?;

    {
        let state = WINDOW_STATE.lock().unwrap_or_else(|e| e.into_inner());
        if state.is_some() {
            #[cfg(debug_assertions)]
            println!("Already in PiP mode, skipping");
            return Ok(());
        }
    }

    let config = get_pip_constants()?;
    let normal_size = window
        .outer_size()
        .map_err(|e| format!("Failed to get window size: {}", e))?;
    let normal_position = window
        .outer_position()
        .map_err(|e| format!("Failed to get window position: {}", e))?;
    let decorations_enabled = window
        .is_decorated()
        .map_err(|e| format!("Failed to get decoration state: {}", e))?;
    let resizable = window
        .is_resizable()
        .map_err(|e| format!("Failed to get resizable state: {}", e))?;
    let fullscreen = window
        .is_fullscreen()
        .map_err(|e| format!("Failed to get fullscreen state: {}", e))?;
    let maximized = window
        .is_maximized()
        .map_err(|e| format!("Failed to get maximized state: {}", e))?;
    let always_on_top = window
        .is_always_on_top()
        .map_err(|e| format!("Failed to get always-on-top state: {}", e))?;

    if fullscreen {
        window
            .set_fullscreen(false)
            .map_err(|e| format!("Failed to leave fullscreen before PiP: {}", e))?;
    }
    if maximized {
        window
            .unmaximize()
            .map_err(|e| format!("Failed to unmaximize before PiP: {}", e))?;
    }

    let work_area = work_area_for_window(&window);
    let saved_layout = load_saved_pip_layout()?;
    let fallback_size = PhysicalSize::new(config.width, config.height);
    let pip_size = saved_layout
        .map(|layout| PhysicalSize::new(layout.width, layout.height))
        .unwrap_or(fallback_size);
    let pip_size = normalize_pip_size(pip_size.width, pip_size.height, None, &config, &work_area);
    let pip_position = saved_layout
        .map(|layout| PhysicalPosition::new(layout.x, layout.y))
        .unwrap_or_else(|| default_pip_position(pip_size, &work_area, &config));
    let pip_position = snap_and_clamp_position(pip_position, pip_size, &work_area, &config);

    let normal_min_size = PhysicalSize::new(config.normal_min_width, config.normal_min_height);
    {
        let mut state = WINDOW_STATE.lock().unwrap_or_else(|e| e.into_inner());
        if state.is_some() {
            return Ok(());
        }
        *state = Some(WindowState {
            size: normal_size,
            position: normal_position,
            decorations_enabled,
            resizable,
            fullscreen,
            maximized,
            always_on_top,
            normal_min_size,
            pip_size,
        });
    }

    window
        .set_decorations(false)
        .map_err(|e| format!("Failed to enable frameless PiP: {}", e))?;
    window
        .set_resizable(true)
        .map_err(|e| format!("Failed to enable PiP resizing: {}", e))?;
    window
        .set_min_size(Some(PhysicalSize::new(config.min_width, config.min_height)))
        .map_err(|e| format!("Failed to set PiP minimum size: {}", e))?;
    window
        .set_max_size(None::<PhysicalSize<u32>>)
        .map_err(|e| format!("Failed to clear PiP maximum size: {}", e))?;
    window
        .set_size(pip_size)
        .map_err(|e| format!("Failed to set PiP size: {}", e))?;
    window
        .set_position(pip_position)
        .map_err(|e| format!("Failed to set PiP position: {}", e))?;
    window
        .set_always_on_top(true)
        .map_err(|e| format!("Failed to set PiP always-on-top: {}", e))?;

    #[cfg(debug_assertions)]
    println!("Entered PiP mode at {:?} {:?}", pip_position, pip_size);

    Ok(())
}

#[tauri::command]
pub(crate) fn exit_pip_mode(app_handle: AppHandle) -> Result<(), String> {
    let window = main_window(&app_handle)?;
    let saved_state = {
        let mut state = WINDOW_STATE.lock().unwrap_or_else(|e| e.into_inner());
        state
            .take()
            .ok_or("Cannot exit PiP mode: no saved window state found")?
    };

    if let Err(err) = persist_current_pip_layout(&window, Some(saved_state.pip_size)) {
        #[cfg(debug_assertions)]
        eprintln!("Failed to persist PiP layout before exit: {}", err);
    }

    let restore_result = (|| -> Result<(), String> {
        window
            .set_always_on_top(saved_state.always_on_top)
            .map_err(|e| format!("Failed to restore always-on-top state: {}", e))?;
        window
            .set_min_size(Some(saved_state.normal_min_size))
            .map_err(|e| format!("Failed to restore minimum window size: {}", e))?;
        window
            .set_max_size(None::<PhysicalSize<u32>>)
            .map_err(|e| format!("Failed to clear maximum window size: {}", e))?;
        window
            .set_decorations(saved_state.decorations_enabled)
            .map_err(|e| format!("Failed to restore decoration state: {}", e))?;
        window
            .set_resizable(saved_state.resizable)
            .map_err(|e| format!("Failed to restore resizable state: {}", e))?;
        window
            .set_size(saved_state.size)
            .map_err(|e| format!("Failed to restore window size: {}", e))?;
        window
            .set_position(saved_state.position)
            .map_err(|e| format!("Failed to restore window position: {}", e))?;
        if saved_state.maximized {
            window
                .maximize()
                .map_err(|e| format!("Failed to restore maximized state: {}", e))?;
        }
        if saved_state.fullscreen {
            window
                .set_fullscreen(true)
                .map_err(|e| format!("Failed to restore fullscreen state: {}", e))?;
        }
        Ok(())
    })();

    if let Err(err) = restore_result {
        let mut state = WINDOW_STATE.lock().unwrap_or_else(|e| e.into_inner());
        *state = Some(saved_state);
        return Err(err);
    }

    #[cfg(debug_assertions)]
    println!("Exited PiP mode");

    Ok(())
}

#[tauri::command]
pub(crate) fn settle_pip_window(app_handle: AppHandle) -> Result<(), String> {
    let window = main_window(&app_handle)?;
    if !is_pip_active() {
        return Ok(());
    }

    let config = get_pip_constants()?;
    let work_area = work_area_for_window(&window);
    let current_size = window
        .outer_size()
        .map_err(|e| format!("Failed to read PiP size: {}", e))?;
    let current_position = window
        .outer_position()
        .map_err(|e| format!("Failed to read PiP position: {}", e))?;
    let previous_size = current_pip_size().unwrap_or(current_size);
    let settled_size = normalize_pip_size(
        current_size.width,
        current_size.height,
        Some(previous_size),
        &config,
        &work_area,
    );
    let settled_position =
        snap_and_clamp_position(current_position, settled_size, &work_area, &config);

    if current_size != settled_size {
        window
            .set_size(settled_size)
            .map_err(|e| format!("Failed to settle PiP size: {}", e))?;
    }
    if current_position != settled_position {
        window
            .set_position(settled_position)
            .map_err(|e| format!("Failed to settle PiP position: {}", e))?;
    }

    save_pip_layout(PipWindowLayout {
        width: settled_size.width,
        height: settled_size.height,
        x: settled_position.x,
        y: settled_position.y,
    })?;
    update_pip_size(settled_size);

    Ok(())
}

#[tauri::command]
pub(crate) fn save_pip_window_layout(app_handle: AppHandle) -> Result<(), String> {
    let window = main_window(&app_handle)?;
    if is_pip_active() {
        persist_current_pip_layout(&window, current_pip_size())?;
    }
    Ok(())
}

fn main_window(app_handle: &AppHandle) -> Result<WebviewWindow, String> {
    app_handle
        .get_webview_window("main")
        .ok_or_else(|| "Failed to get main window".to_string())
}

fn get_pip_constants() -> Result<PipWindowConfig, String> {
    static CACHE: OnceLock<PipWindowConfig> = OnceLock::new();
    if let Some(config) = CACHE.get() {
        return Ok(*config);
    }
    const CONSTANTS_JSON: &str = include_str!("../../constants.json");
    let constants: AppConstants = serde_json::from_str(CONSTANTS_JSON)
        .map_err(|e| format!("Failed to parse constants.json: {}", e))?;
    Ok(*CACHE.get_or_init(|| constants.pip_window))
}

fn is_pip_active() -> bool {
    WINDOW_STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some()
}

fn current_pip_size() -> Option<PhysicalSize<u32>> {
    WINDOW_STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .map(|state| state.pip_size)
}

fn update_pip_size(size: PhysicalSize<u32>) {
    if let Some(state) = WINDOW_STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_mut()
    {
        state.pip_size = size;
    }
}

fn persist_current_pip_layout(
    window: &WebviewWindow,
    previous_size: Option<PhysicalSize<u32>>,
) -> Result<(), String> {
    let config = get_pip_constants()?;
    let work_area = work_area_for_window(window);
    let size = window
        .outer_size()
        .map_err(|e| format!("Failed to read PiP size: {}", e))?;
    let position = window
        .outer_position()
        .map_err(|e| format!("Failed to read PiP position: {}", e))?;
    let normalized_size =
        normalize_pip_size(size.width, size.height, previous_size, &config, &work_area);
    let normalized_position =
        snap_and_clamp_position(position, normalized_size, &work_area, &config);

    save_pip_layout(PipWindowLayout {
        width: normalized_size.width,
        height: normalized_size.height,
        x: normalized_position.x,
        y: normalized_position.y,
    })?;
    update_pip_size(normalized_size);

    Ok(())
}

fn normalize_pip_size(
    width: u32,
    height: u32,
    previous_size: Option<PhysicalSize<u32>>,
    config: &PipWindowConfig,
    work_area: &WorkArea,
) -> PhysicalSize<u32> {
    let ratio = aspect_ratio(config);
    let min_width = config
        .min_width
        .max(((config.min_height as f64) * ratio).ceil() as u32);
    let mut max_width = config.max_width.max(min_width);
    let available_width = work_area
        .width
        .saturating_sub((config.padding.max(0) as u32).saturating_mul(2));
    let available_height = work_area
        .height
        .saturating_sub((config.padding.max(0) as u32).saturating_mul(2));
    if available_width > 0 {
        max_width = max_width.min(available_width.max(min_width));
    }
    if available_height > 0 {
        let height_limited_width = ((available_height as f64) * ratio).floor() as u32;
        max_width = max_width.min(height_limited_width.max(min_width));
    }

    let preferred_width = if let Some(previous) = previous_size {
        let width_delta = relative_delta(width, previous.width);
        let height_delta = relative_delta(height, previous.height);
        if height_delta > width_delta {
            ((height as f64) * ratio).round() as u32
        } else {
            width
        }
    } else {
        width.max(((height as f64) * ratio).round() as u32)
    };

    let settled_width = preferred_width.clamp(min_width, max_width);
    let settled_height = ((settled_width as f64) / ratio).round().max(1.0) as u32;

    PhysicalSize::new(settled_width, settled_height)
}

fn relative_delta(current: u32, previous: u32) -> f64 {
    if previous == 0 {
        return 0.0;
    }
    ((current as f64) - (previous as f64)).abs() / (previous as f64)
}

fn aspect_ratio(config: &PipWindowConfig) -> f64 {
    let width = config.aspect_width.max(1) as f64;
    let height = config.aspect_height.max(1) as f64;
    width / height
}

fn work_area_for_window(window: &WebviewWindow) -> WorkArea {
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());

    if let Some(m) = monitor {
        let work_area = m.work_area();
        return WorkArea {
            x: work_area.position.x,
            y: work_area.position.y,
            width: work_area.size.width,
            height: work_area.size.height,
        };
    }

    // Both monitor queries failed — use a conservative screen-aligned fallback so
    // default_pip_position and snap_and_clamp_position clamp against the screen origin,
    // not the window's own bounding box.
    #[cfg(debug_assertions)]
    eprintln!("work_area_for_window: could not determine monitor; using fallback work area");
    WorkArea {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
    }
}

fn default_pip_position(
    size: PhysicalSize<u32>,
    work_area: &WorkArea,
    config: &PipWindowConfig,
) -> PhysicalPosition<i32> {
    let padding = config.padding.max(0);
    let x = work_area.x + work_area.width as i32 - size.width as i32 - padding;
    let y = work_area.y + work_area.height as i32 - size.height as i32 - padding;
    PhysicalPosition::new(x, y)
}

fn snap_and_clamp_position(
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    work_area: &WorkArea,
    config: &PipWindowConfig,
) -> PhysicalPosition<i32> {
    let padding = config.padding.max(0);
    let left = work_area.x + padding;
    let top = work_area.y + padding;
    let right = work_area.x + work_area.width as i32 - size.width as i32 - padding;
    let bottom = work_area.y + work_area.height as i32 - size.height as i32 - padding;
    let snap = config.snap_threshold.max(0);
    let mut x = position.x;
    let mut y = position.y;

    if (x - left).abs() <= snap {
        x = left;
    } else if (x - right).abs() <= snap {
        x = right;
    }

    if (y - top).abs() <= snap {
        y = top;
    } else if (y - bottom).abs() <= snap {
        y = bottom;
    }

    PhysicalPosition::new(clamp_i32(x, left, right), clamp_i32(y, top, bottom))
}

fn clamp_i32(value: i32, min: i32, max: i32) -> i32 {
    if max < min {
        return min;
    }
    value.clamp(min, max)
}

fn user_config_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    Ok(home.join(".glucose").join("config.json"))
}

fn load_saved_pip_layout() -> Result<Option<PipWindowLayout>, String> {
    let config_file = user_config_path()?;
    let _guard = crate::CONFIG_MUTEX
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    if !config_file.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(&config_file).map_err(|e| format!("Failed to read config: {}", e))?;
    let config: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    Ok(config
        .get("pip_window")
        .and_then(|value| serde_json::from_value(value.clone()).ok()))
}

fn save_pip_layout(layout: PipWindowLayout) -> Result<(), String> {
    let config_file = user_config_path()?;
    let config_dir = config_file
        .parent()
        .ok_or_else(|| "Could not get config directory".to_string())?;

    fs::create_dir_all(config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;

    let _guard = crate::CONFIG_MUTEX
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let mut config: Value = if config_file.exists() {
        let content = fs::read_to_string(&config_file)
            .map_err(|e| format!("Failed to read config: {}", e))?;
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))?
    } else {
        serde_json::json!({})
    };

    let config_object = config
        .as_object_mut()
        .ok_or_else(|| "Config root must be a JSON object".to_string())?;
    config_object.insert("pip_window".to_string(), serde_json::json!(layout));

    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    let temp_file = config_file.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&temp_file)
            .map_err(|e| format!("Failed to create temp config: {}", e))?;
        file.write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write temp config: {}", e))?;
        file.sync_all()
            .map_err(|e| format!("Failed to sync temp config: {}", e))?;
    }
    if let Err(e) = fs::rename(&temp_file, &config_file) {
        let _ = fs::remove_file(&temp_file);
        return Err(format!("Failed to replace config: {}", e));
    }

    Ok(())
}

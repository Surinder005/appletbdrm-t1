use crate::config::Config;
use crate::TIMEOUT_MS;
use anyhow::{anyhow, Result};
use input::event::{
    switch::{Switch, SwitchEvent, SwitchState},
    Event,
};
use std::{
    cmp::min,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

const MAX_DISPLAY_BRIGHTNESS: u32 = 509;
const MAX_TOUCH_BAR_BRIGHTNESS: u32 = 255;
const BRIGHTNESS_DIM_TIMEOUT: i32 = TIMEOUT_MS * 3; // should be a multiple of TIMEOUT_MS
const BRIGHTNESS_OFF_TIMEOUT: i32 = TIMEOUT_MS * 6; // should be a multiple of TIMEOUT_MS
const DIMMED_BRIGHTNESS: u32 = 1;

fn read_attr(path: &Path, attr: &str) -> u32 {
    fs::read_to_string(path.join(attr))
        .unwrap_or_else(|_| panic!("Failed to read {attr}"))
        .trim()
        .parse::<u32>()
        .unwrap_or_else(|_| panic!("Failed to parse {attr}"))
}

fn find_backlight() -> Result<PathBuf> {
    for entry in fs::read_dir("/sys/class/backlight/")? {
        let entry = entry?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        if ["display-pipe", "228600000.dsi.0", "appletb_backlight"]
            .iter()
            .any(|s| name.contains(s))
        {
            return Ok(entry.path());
        }
    }
    Err(anyhow!("No Touch Bar backlight device found"))
}

fn find_display_backlight() -> Result<PathBuf> {
    for entry in fs::read_dir("/sys/class/backlight/")? {
        let entry = entry?;
        if [
            "apple-panel-bl",
            "gmux_backlight",
            "intel_backlight",
            "acpi_video0",
        ]
        .iter()
        .any(|s| entry.file_name().to_string_lossy().contains(s))
        {
            return Ok(entry.path());
        }
    }
    Err(anyhow!("No Built-in Retina Display backlight device found"))
}

fn set_backlight(mut file: &File, value: u32) {
    file.write_all(format!("{}\n", value).as_bytes()).unwrap();
}

pub struct BacklightManager {
    last_active: Instant,
    max_bl: u32,
    current_bl: u32,
    lid_state: SwitchState,
    bl_file: Option<File>,
    display_bl_path: Option<PathBuf>,
    has_dedicated_backlight: bool,
}

impl BacklightManager {
    pub fn new() -> BacklightManager {
        let bl_path = find_backlight();
        let display_bl_path = find_display_backlight().ok();

        match bl_path {
            Ok(ref path) => {
                // Dedicated Touch Bar backlight found (T2 / Apple Silicon)
                let bl_file = OpenOptions::new()
                    .write(true)
                    .open(path.join("brightness"))
                    .ok();
                BacklightManager {
                    bl_file,
                    lid_state: SwitchState::Off,
                    max_bl: read_attr(path, "max_brightness"),
                    current_bl: read_attr(path, "brightness"),
                    last_active: Instant::now(),
                    display_bl_path,
                    has_dedicated_backlight: true,
                }
            }
            Err(_) => {
                // No dedicated Touch Bar backlight (T1 iBridge)
                // Touch Bar brightness is managed by the DRM driver itself
                println!("No dedicated Touch Bar backlight found, using always-on mode (T1 iBridge)");
                BacklightManager {
                    bl_file: None,
                    lid_state: SwitchState::Off,
                    max_bl: MAX_TOUCH_BAR_BRIGHTNESS,
                    current_bl: MAX_TOUCH_BAR_BRIGHTNESS,
                    last_active: Instant::now(),
                    display_bl_path,
                    has_dedicated_backlight: false,
                }
            }
        }
    }
    fn display_to_touchbar(display: u32, active_brightness: u32) -> u32 {
        let normalized = display as f64 / MAX_DISPLAY_BRIGHTNESS as f64;
        // Add one so that the touch bar does not turn off
        let adjusted = (normalized.powf(0.5) * active_brightness as f64) as u32 + 1;
        adjusted.min(MAX_TOUCH_BAR_BRIGHTNESS) // Clamp the value to the maximum allowed brightness
    }
    pub fn process_event(&mut self, event: &Event) {
        match event {
            Event::Keyboard(_) | Event::Pointer(_) | Event::Gesture(_) | Event::Touch(_) => {
                self.last_active = Instant::now();
            }
            Event::Switch(SwitchEvent::Toggle(toggle)) => {
                if let Some(Switch::Lid) = toggle.switch() {
                    self.lid_state = toggle.switch_state();
                    println!("Lid Switch event: {:?}", self.lid_state);
                    if toggle.switch_state() == SwitchState::Off {
                        self.last_active = Instant::now();
                    }
                }
            }
            _ => {}
        }
    }
    pub fn update_backlight(&mut self, cfg: &Config) {
        if !self.has_dedicated_backlight {
            // No hardware backlight control available; keep reporting full brightness
            // so touch events remain active
            self.current_bl = MAX_TOUCH_BAR_BRIGHTNESS;
            return;
        }

        let since_last_active = (Instant::now() - self.last_active).as_millis() as u64;
        let new_bl = min(
            self.max_bl,
            if self.lid_state == SwitchState::On {
                0
            } else if since_last_active < BRIGHTNESS_DIM_TIMEOUT as u64 {
                if cfg.adaptive_brightness {
                    if let Some(ref display_bl_path) = self.display_bl_path {
                        BacklightManager::display_to_touchbar(
                            read_attr(display_bl_path, "brightness"),
                            cfg.active_brightness,
                        )
                    } else {
                        cfg.active_brightness
                    }
                } else {
                    cfg.active_brightness
                }
            } else if since_last_active < BRIGHTNESS_OFF_TIMEOUT as u64 {
                DIMMED_BRIGHTNESS
            } else {
                0
            },
        );
        if self.current_bl != new_bl {
            self.current_bl = new_bl;
            if let Some(ref mut file) = self.bl_file {
                set_backlight(file, self.current_bl);
            }
        }
    }
    pub fn current_bl(&self) -> u32 {
        self.current_bl
    }
}

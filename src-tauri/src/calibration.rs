use std::{
    collections::BTreeMap,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

use crate::hardware::Rgb;

/// Per-device white balance. Each channel is scaled to `gain`% before it is
/// sent, so LEDs with a dominant red or a weak blue show the intended hue.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Calibration {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Default for Calibration {
    fn default() -> Self {
        Self { red: 100, green: 100, blue: 100 }
    }
}

impl Calibration {
    /// Starting point for a device that has no saved calibration.
    pub fn preset(device_id: &str) -> Self {
        if device_id.starts_with("dimm-ene-") {
            // XPG / ADATA ENE modules: the blue die is several times weaker than red
            // and green, so white turns yellow, purple turns pink and blue or cyan
            // read as pale green. Orange (no blue) is right, so red and green stay
            // balanced with each other and both come well down to meet blue.
            Self { red: 40, green: 35, blue: 100 }
        } else {
            Self::default()
        }
    }

    pub fn apply(self, color: Rgb) -> Rgb {
        let gains = [self.red, self.green, self.blue];
        std::array::from_fn(|channel| {
            ((u16::from(color[channel]) * u16::from(gains[channel].min(100)) + 50) / 100) as u8
        })
    }

    fn clamped(self) -> Self {
        Self { red: self.red.min(100), green: self.green.min(100), blue: self.blue.min(100) }
    }
}

pub fn lookup(saved: &BTreeMap<String, Calibration>, device_id: &str) -> Calibration {
    saved.get(device_id).copied().unwrap_or_else(|| Calibration::preset(device_id))
}

pub struct Calibrations(Mutex<PathBuf>);

impl Calibrations {
    pub fn new(path: PathBuf) -> Self {
        Self(Mutex::new(path))
    }

    pub fn load(&self) -> BTreeMap<String, Calibration> {
        self.0.lock().map(|path| read(&path)).unwrap_or_default()
    }

    pub fn save(&self, device_id: &str, calibration: Calibration) -> Result<Calibration, String> {
        let path = self.0.lock().map_err(|error| error.to_string())?;
        let mut saved = read(&path);
        let calibration = calibration.clamped();
        saved.insert(device_id.to_owned(), calibration);
        let directory = path.parent().ok_or("The settings directory is unavailable.")?;
        fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let mut temporary = tempfile::NamedTempFile::new_in(directory).map_err(|error| error.to_string())?;
        serde_json::to_writer_pretty(temporary.as_file_mut(), &saved).map_err(|error| error.to_string())?;
        temporary.persist(path.as_path()).map_err(|error| error.to_string())?;
        Ok(calibration)
    }
}

fn read(path: &Path) -> BTreeMap<String, Calibration> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(error) if error.kind() == ErrorKind::NotFound => BTreeMap::new(),
        Err(_) => BTreeMap::new(),
    }
}

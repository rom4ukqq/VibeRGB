#[cfg(windows)]
mod asrock;
#[cfg(windows)]
mod msi;
#[cfg(windows)]
pub(crate) mod pawnio;
#[cfg(windows)]
mod ram;
#[cfg(windows)]
mod smbios;

use std::sync::{Arc, Mutex};

use serde::Serialize;

pub type Rgb = [u8; 3];

pub fn dim(color: Rgb, brightness: u8) -> Rgb {
    color.map(|channel| ((u16::from(channel) * u16::from(brightness.min(100)) + 50) / 100) as u8)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    pub id: &'static str,
    pub name: &'static str,
    pub color: bool,
    pub brightness: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub effects: Vec<Effect>,
    pub direct: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    pub detail: String,
    pub zones: Vec<Zone>,
    pub needs_pawnio_install: bool,
    pub needs_pawnio_elevation: bool,
}

pub trait Controller: Send {
    fn device(&self) -> &Device;
    fn apply(&mut self, zone: &str, mode: &str, color: Rgb, brightness: u8) -> Result<(), String>;
    fn begin_animation(&mut self, zone: &str, color: Rgb) -> Result<(), String> {
        self.apply(zone, "solid", color, 100)
    }
    fn frame(&mut self, zone: &str, color: Rgb) -> Result<(), String>;
    fn frames(&mut self, updates: &[(&str, Rgb)]) -> Result<Vec<(String, String)>, String> {
        let mut failures = Vec::new();
        for &(zone, color) in updates {
            if let Err(error) = self.frame(zone, color) {
                failures.push((zone.to_owned(), error));
            }
        }
        Ok(failures)
    }
}

pub type SharedController = Arc<Mutex<Box<dyn Controller>>>;

/// A controller together with a copy of its device description, so the
/// device list can be read while an output thread is writing a frame.
pub struct Output {
    pub device: Device,
    pub controller: SharedController,
}

impl Output {
    fn new(controller: Box<dyn Controller>) -> Self {
        Self {
            device: controller.device().clone(),
            controller: Arc::new(Mutex::new(controller)),
        }
    }
}

#[derive(Default)]
pub struct Hardware {
    pub controllers: Vec<Output>,
    pub unavailable: Vec<Device>,
}

impl Hardware {
    pub fn discover() -> Self {
        #[cfg(windows)]
        {
            let firmware = smbios::Firmware::read();
            let mut hardware = Self {
                unavailable: firmware.memory(),
                ..Self::default()
            };
            if let Some((model, id)) = asrock::identify(&firmware) {
                let name = format!("ASRock {model}");
                hardware.add(id, &name, asrock::Board::open(id, &name));
            }
            hardware.add("msi-mystic-light", "MSI Mystic Light GPU", msi::Gpu::open());
            // Every supported board is an AMD B550 board with the same PIIX4 SMBus.
            if asrock::identify(&firmware).is_some()
                && hardware.unavailable.iter().any(is_adata_memory)
            {
                let count = hardware
                    .unavailable
                    .iter()
                    .filter(|device| is_adata_memory(device))
                    .count();
                match ram::Dimm::discover(count) {
                    Ok((modules, warning)) => {
                        if modules.len() == count {
                            hardware
                                .unavailable
                                .retain(|device| !is_adata_memory(device));
                        } else {
                            for device in hardware
                                .unavailable
                                .iter_mut()
                                .filter(|device| is_adata_memory(device))
                            {
                                device.detail = format!("Found {} of {count} ENE RGB controllers. ENE modules are listed separately.", modules.len());
                                if let Some(reason) = &warning {
                                    device.detail.push_str(&format!(" {reason}"));
                                }
                            }
                        }
                        hardware.controllers.extend(
                            modules
                                .into_iter()
                                .map(|module| Output::new(Box::new(module))),
                        );
                    }
                    Err(error) => {
                        let needs_pawnio_install = pawnio::is_missing();
                        for device in hardware
                            .unavailable
                            .iter_mut()
                            .filter(|device| is_adata_memory(device))
                        {
                            device.detail = error.clone();
                            device.needs_pawnio_install = needs_pawnio_install;
                            device.needs_pawnio_elevation = error == ram::NEEDS_ADMIN;
                        }
                    }
                }
            }
            hardware
        }
        #[cfg(not(windows))]
        Self::default()
    }

    #[cfg(windows)]
    fn add(&mut self, id: &str, name: &str, result: Result<impl Controller + 'static, String>) {
        match result {
            Ok(controller) => self.controllers.push(Output::new(Box::new(controller))),
            Err(detail) => self.unavailable.push(Device {
                id: id.into(),
                name: name.into(),
                detail,
                zones: Vec::new(),
                needs_pawnio_install: false,
                needs_pawnio_elevation: false,
            }),
        }
    }

    pub fn devices(&self) -> Vec<Device> {
        self.controllers
            .iter()
            .map(|output| output.device.clone())
            .chain(self.unavailable.iter().cloned())
            .collect()
    }

    pub fn output(&self, id: &str) -> Option<&Output> {
        self.controllers.iter().find(|output| output.device.id == id)
    }
}

#[cfg(windows)]
fn is_adata_memory(device: &Device) -> bool {
    device.id.starts_with("dimm-") && device.name.to_ascii_uppercase().contains("A-DATA")
}

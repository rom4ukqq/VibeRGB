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

#[derive(Default)]
pub struct Hardware {
    pub controllers: Vec<Box<dyn Controller>>,
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
            hardware.add(
                asrock::BOARD_ID,
                asrock::BOARD_NAME,
                asrock::Board::open(&firmware),
            );
            hardware.add(msi::GPU_ID, msi::GPU_NAME, msi::Gpu::open());
            if firmware.board_matches("B550M Pro4")
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
                                .map(|module| Box::new(module) as Box<dyn Controller>),
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
            Ok(controller) => self.controllers.push(Box::new(controller)),
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
            .map(|controller| controller.device().clone())
            .chain(self.unavailable.iter().cloned())
            .collect()
    }

    pub fn controller(&mut self, id: &str) -> Option<&mut (dyn Controller + '_)> {
        for controller in &mut self.controllers {
            if controller.device().id == id {
                return Some(controller.as_mut());
            }
        }
        None
    }
}

#[cfg(windows)]
fn is_adata_memory(device: &Device) -> bool {
    device.id.starts_with("dimm-") && device.name.to_ascii_uppercase().contains("A-DATA")
}

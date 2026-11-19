use std::{
    ffi::c_void,
    mem::{size_of, transmute},
    ptr::null_mut,
    thread,
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{FreeLibrary, HMODULE},
    System::LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32},
};

use super::{dim, Controller, Device, Effect, Rgb, Zone};

const NVIDIA_VENDOR: u32 = 0x10de;
const MSI_VENDOR: u32 = 0x1462;
// Device ID, subsystem ID, VibeRGB ID, name. From OpenRGB's MSI GPU v2 (ITE9) list;
// all use the same I²C protocol at 0x68.
const GPU_MODELS: &[(u16, u16, &str, &str)] = &[
    (0x2882, 0x5160, "msi-4060-gaming-x-5160", "MSI GeForce RTX 4060 Gaming X"),
    (0x2882, 0x5164, "msi-4060-gaming-x-nv-edition-5164", "MSI GeForce RTX 4060 Gaming X NV Edition"),
    (0x2803, 0x5152, "msi-4060-ti-gaming-x-5152", "MSI GeForce RTX 4060 Ti Gaming X"),
    (0x2805, 0x5172, "msi-4060-ti-16gb-gaming-x-5172", "MSI GeForce RTX 4060 Ti 16GB Gaming X"),
    (0x2805, 0x5171, "msi-4060-ti-16gb-gaming-x-slim-white-5171", "MSI GeForce RTX 4060 Ti 16GB Gaming X Slim White"),
    (0x2786, 0x5136, "msi-4070-gaming-x-trio-5136", "MSI GeForce RTX 4070 Gaming X Trio"),
    (0x2786, 0x513f, "msi-4070-gaming-x-slim-513f", "MSI GeForce RTX 4070 Gaming X Slim"),
    (0x2786, 0xe131, "msi-4070-gaming-x-slim-white-e131", "MSI GeForce RTX 4070 Gaming X Slim White"),
    (0x2783, 0x513f, "msi-4070-super-gaming-x-slim-513f", "MSI GeForce RTX 4070 SUPER Gaming X Slim"),
    (0x2783, 0xe131, "msi-4070-super-white", "MSI RTX 4070 SUPER Gaming X Slim White"),
    (0x2783, 0xe134, "msi-4070-super-mlg", "MSI RTX 4070 SUPER Gaming X Slim MLG"),
    (0x2782, 0x5132, "msi-4070-ti-gaming-x-trio-5132", "MSI GeForce RTX 4070 Ti Gaming X Trio"),
    (0x2782, 0x5139, "msi-4070-ti-gaming-x-trio-white-5139", "MSI GeForce RTX 4070 Ti Gaming X Trio White"),
    (0x2782, 0xe132, "msi-4070-ti-gaming-x-slim-white-e132", "MSI GeForce RTX 4070 Ti Gaming X Slim White"),
    (0x2782, 0x5133, "msi-4070-ti-suprim-x-trio-5133", "MSI GeForce RTX 4070 Ti Suprim X Trio"),
    (0x2705, 0x5139, "msi-4070-ti-super-gaming-x-trio-white-5139", "MSI GeForce RTX 4070 Ti SUPER Gaming X Trio White"),
    (0x2705, 0xe130, "msi-4070-ti-super-gaming-slim-e130", "MSI GeForce RTX 4070 Ti SUPER Gaming Slim"),
    (0x2705, 0xe13d, "msi-4070-ti-super-gaming-x-slim-e13d", "MSI GeForce RTX 4070 Ti SUPER Gaming X Slim"),
    (0x2705, 0xe13e, "msi-4070-ti-super-gaming-x-slim-white-e13e", "MSI GeForce RTX 4070 Ti SUPER Gaming X Slim White"),
    (0x2705, 0xe132, "msi-4070-ti-super-gaming-x-slim-white-rev-2-e132", "MSI GeForce RTX 4070 Ti SUPER Gaming X Slim White (rev. 2)"),
    (0x2704, 0x511a, "msi-4080-gaming-x-slim-white-511a", "MSI GeForce RTX 4080 Gaming X Slim White"),
    (0x2704, 0x5111, "msi-4080-gaming-x-trio-5111", "MSI GeForce RTX 4080 Gaming X Trio"),
    (0x2704, 0x5115, "msi-4080-gaming-x-trio-white-5115", "MSI GeForce RTX 4080 Gaming X Trio White"),
    (0x2704, 0x5110, "msi-4080-suprim-x-5110", "MSI GeForce RTX 4080 Suprim X"),
    (0x2702, 0x5117, "msi-4080-super-gaming-x-slim-5117", "MSI GeForce RTX 4080 SUPER Gaming X Slim"),
    (0x2702, 0x511a, "msi-4080-super-gaming-x-slim-white-511a", "MSI GeForce RTX 4080 SUPER Gaming X Slim White"),
    (0x2702, 0x5110, "msi-4080-super-suprim-x-5110", "MSI GeForce RTX 4080 SUPER Suprim X"),
    (0x2702, 0x5111, "msi-4080-super-gaming-x-trio-5111", "MSI GeForce RTX 4080 SUPER Gaming X Trio"),
    (0x2684, 0x5103, "msi-4090-gaming-x-trio-5103", "MSI GeForce RTX 4090 Gaming X Trio"),
    (0x2684, 0x510b, "msi-4090-gaming-x-slim-510b", "MSI GeForce RTX 4090 Gaming X Slim"),
    (0x2684, 0x5104, "msi-4090-suprim-liquid-x-5104", "MSI GeForce RTX 4090 Suprim Liquid X"),
    (0x2684, 0x5102, "msi-4090-suprim-x-5102", "MSI GeForce RTX 4090 Suprim X"),
    (0x2684, 0x5105, "msi-4090-suprim-x-classic-5105", "MSI GeForce RTX 4090 Suprim X Classic"),
    (0x2d04, 0x5354, "msi-5060-ti-gaming-oc-5354", "MSI GeForce RTX 5060 Ti Gaming OC"),
    (0x2d05, 0x5372, "msi-5060-gaming-trio-oc-5372", "MSI GeForce RTX 5060 Gaming Trio OC"),
    (0x2d04, 0x5353, "msi-5060-ti-16g-gaming-trio-oc-5353", "MSI GeForce RTX 5060 Ti 16G Gaming Trio OC"),
    (0x2f04, 0x5322, "msi-5070-gaming-trio-5322", "MSI GeForce RTX 5070 Gaming Trio"),
    (0x2c05, 0x5315, "msi-5070-ti-gaming-trio-oc-plus-5315", "MSI GeForce RTX 5070 Ti Gaming Trio OC Plus"),
    (0x2c02, 0x5315, "msi-5080-gaming-trio-oc-5315", "MSI GeForce RTX 5080 Gaming Trio OC"),
    (0x2c02, 0x5314, "msi-5080-vanguard-oc-soc-5314", "MSI GeForce RTX 5080 Vanguard OC SOC"),
    (0x2c02, 0x5313, "msi-5080-suprim-soc-5313", "MSI GeForce RTX 5080 Suprim SOC"),
    (0x2c02, 0x5312, "msi-5080-suprim-liquid-soc-5312", "MSI GeForce RTX 5080 Suprim Liquid SOC"),
    (0x2b85, 0x5303, "msi-5090-gaming-trio-oc-5303", "MSI GeForce RTX 5090 Gaming Trio OC"),
    (0x2b85, 0x5302, "msi-5090-vanguard-soc-5302", "MSI GeForce RTX 5090 Vanguard SOC"),
    (0x2b85, 0x5301, "msi-5090-suprim-soc-5301", "MSI GeForce RTX 5090 Suprim SOC"),
    (0x2b85, 0x5300, "msi-5090-suprim-liquid-soc-5300", "MSI GeForce RTX 5090 Suprim Liquid SOC"),
];
const MODE_REGISTER: u8 = 0x22;
const MODE_IDLE: u8 = 0x1c;
const MODE_STATIC: u8 = 0x13;
const EFFECTS: [Effect; 2] = [
    Effect {
        id: "breathing",
        name: "Breathing",
        color: true,
        brightness: true,
    },
    Effect {
        id: "rainbow",
        name: "Rainbow",
        color: false,
        brightness: true,
    },
];

type GpuHandle = *mut c_void;
type Query = unsafe extern "C" fn(u32) -> *const c_void;
type Initialize = unsafe extern "C" fn() -> i32;
type Enumerate = unsafe extern "C" fn(*mut GpuHandle, *mut i32) -> i32;
type PciIds = unsafe extern "C" fn(GpuHandle, *mut u32, *mut u32, *mut u32, *mut u32) -> i32;
type I2cOperation = unsafe extern "C" fn(GpuHandle, *mut I2cInfo, *mut u32) -> i32;

#[repr(C)]
struct I2cInfo {
    version: u32,
    display_mask: u32,
    is_ddc_port: u8,
    device_address: u8,
    register: *mut u8,
    register_length: u32,
    data: *mut u8,
    length: u32,
    legacy_speed: u32,
    speed: u32,
    port: u8,
    port_is_set: u32,
}

struct Library(HMODULE);

impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: This reference was acquired with LoadLibraryExW.
        unsafe { FreeLibrary(self.0) };
    }
}

struct Nvapi {
    _library: Library,
    unload: Initialize,
    enumerate: Enumerate,
    pci_ids: PciIds,
    read: I2cOperation,
    write: I2cOperation,
}

impl Nvapi {
    fn load() -> Result<Self, String> {
        let filename: Vec<u16> = "nvapi64.dll\0".encode_utf16().collect();
        // SAFETY: Search is confined to System32. The function IDs and C signatures are NVAPI ABI.
        unsafe {
            let handle =
                LoadLibraryExW(filename.as_ptr(), null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32);
            if handle.is_null() {
                return Err("NVAPI is unavailable. Check the NVIDIA driver.".into());
            }
            let library = Library(handle);
            let query: Query = transmute::<unsafe extern "system" fn() -> isize, Query>(
                GetProcAddress(handle, c"nvapi_QueryInterface".as_ptr().cast())
                    .ok_or("NVAPI entry point unavailable.")?,
            );
            let symbol = |id| {
                let pointer = query(id);
                if pointer.is_null() {
                    Err(format!("NVAPI function {id:#010x} is unavailable."))
                } else {
                    Ok(pointer)
                }
            };
            let initialize = transmute::<_, Initialize>(symbol(0x0150e828)?);
            let unload = transmute::<_, Initialize>(symbol(0xd22bdd7e)?);
            let enumerate = transmute::<_, Enumerate>(symbol(0xe5ac921f)?);
            let pci_ids = transmute::<_, PciIds>(symbol(0x2ddfb66e)?);
            let read = transmute::<_, I2cOperation>(symbol(0x4d7b0709)?);
            let write = transmute::<_, I2cOperation>(symbol(0x283ac65a)?);
            check(initialize(), "initialization")?;
            Ok(Self {
                _library: library,
                unload,
                enumerate,
                pci_ids,
                read,
                write,
            })
        }
    }

    fn find_gpu(&self) -> Result<(GpuHandle, &'static str, &'static str), String> {
        let mut handles = [null_mut(); 64];
        let mut length = 0;
        // SAFETY: NVAPI accepts a 64-entry GPU handle array and four writable ID fields.
        unsafe {
            check(
                (self.enumerate)(handles.as_mut_ptr(), &mut length),
                "GPU list",
            )?
        };
        let handles = handles
            .get(..length as usize)
            .ok_or("NVAPI returned an invalid GPU list.")?;
        let mut matches = handles.iter().copied().filter_map(|handle| {
            let (mut device, mut subsystem, mut revision, mut extended) = (0, 0, 0, 0);
            let result = unsafe {
                (self.pci_ids)(
                    handle,
                    &mut device,
                    &mut subsystem,
                    &mut revision,
                    &mut extended,
                )
            };
            // NVAPI packs the vendor in the low word and the device in the high word.
            if result != 0 || device & 0xffff != NVIDIA_VENDOR || subsystem & 0xffff != MSI_VENDOR {
                return None;
            }
            let (device, subsystem) = ((device >> 16) as u16, (subsystem >> 16) as u16);
            GPU_MODELS.iter().find(|model| model.0 == device && model.1 == subsystem)
                .map(|&(_, _, id, name)| (handle, id, name))
        });
        let handle = matches
            .next()
            .ok_or("No supported MSI Mystic Light GPU found.")?;
        if matches.next().is_some() {
            return Err("Two matching GPUs were found. PCI slot selection is unavailable.".into());
        }
        Ok(handle)
    }

    fn transfer(
        &self,
        gpu: GpuHandle,
        mut register: u8,
        bytes: &mut [u8],
        write: bool,
    ) -> Result<(), String> {
        let mut request = I2cInfo {
            version: (3 << 16) | size_of::<I2cInfo>() as u32,
            display_mask: 0,
            is_ddc_port: 0,
            device_address: 0x68 << 1,
            register: &mut register,
            register_length: 1,
            data: bytes.as_mut_ptr(),
            length: bytes.len() as u32,
            legacy_speed: 0xffff,
            speed: 0,
            port: 1,
            port_is_set: 1,
        };
        let operation = if write { self.write } else { self.read };
        // SAFETY: The handle came from this NVAPI instance. Buffers live for the synchronous call.
        let result = unsafe { operation(gpu, &mut request, &mut 0) };
        check(result, &format!("I²C register {register:#04x}"))?;
        Ok(())
    }
}

impl Drop for Nvapi {
    fn drop(&mut self) {
        // SAFETY: Initialization succeeded and the DLL is still loaded.
        unsafe { (self.unload)() };
    }
}

pub struct Gpu {
    nvapi: Nvapi,
    handle: GpuHandle,
    device: Device,
}

// NVAPI handles belong to the process. Only one thread uses this instance at a time.
unsafe impl Send for Gpu {}

impl Gpu {
    pub fn open() -> Result<Self, String> {
        let nvapi = Nvapi::load()?;
        let (handle, id, name) = nvapi.find_gpu()?;
        nvapi.transfer(handle, MODE_REGISTER, &mut [0], false)?;
        let device = Device {
            id: id.into(),
            name: name.into(),
            detail: "MSI Mystic Light · NVIDIA NVAPI".into(),
            needs_pawnio_install: false,
            needs_pawnio_elevation: false,
            zones: vec![Zone {
                id: "gpu".into(),
                name: "GPU lighting".into(),
                effects: EFFECTS.to_vec(),
                direct: true,
            }],
        };
        Ok(Self {
            nvapi,
            handle,
            device,
        })
    }

    fn write(&self, register: u8, values: &mut [u8]) -> Result<(), String> {
        self.write_with_delay(register, values, Duration::from_millis(20))
    }

    fn write_with_delay(&self, register: u8, values: &mut [u8], delay: Duration) -> Result<(), String> {
        self.nvapi.transfer(self.handle, register, values, true)?;
        thread::sleep(delay);
        Ok(())
    }

    fn color(&self, [red, green, blue]: Rgb) -> Result<(), String> {
        self.write(0x30, &mut [red])?;
        self.write(0x31, &mut [green])?;
        self.write(0x32, &mut [blue])
    }

    fn prepare_mode(&self) -> Result<(), String> {
        self.write(0x2e, &mut [0])?;
        self.write(MODE_REGISTER, &mut [MODE_IDLE])
    }
}

impl Controller for Gpu {
    fn device(&self) -> &Device {
        &self.device
    }

    fn apply(&mut self, _zone: &str, mode: &str, color: Rgb, brightness: u8) -> Result<(), String> {
        let mode = if brightness == 0 { "off" } else { mode };
        self.prepare_mode()?;
        let value = match mode {
            "off" => 0x01,
            "solid" => {
                self.color(dim(color, brightness))?;
                MODE_STATIC
            }
            "rainbow" => {
                self.write(0x46, &mut [0])?;
                self.write(0x38, &mut [2])?;
                0x08
            }
            "breathing" => {
                let [red, green, blue] = color;
                self.write(0x27, &mut [blue, green, red])?;
                self.write(0x28, &mut [0, 0, 0])?;
                self.write(0x38, &mut [2])?;
                0x04
            }
            _ => return Err("Mode unavailable on the MSI GPU.".into()),
        };
        if mode != "off" {
            let level = if mode == "solid" {
                100
            } else {
                brightness.div_ceil(20) * 20
            };
            self.write(0x36, &mut [level])?;
        }
        self.write(MODE_REGISTER, &mut [value])
    }

    fn frame(&mut self, _zone: &str, color: Rgb) -> Result<(), String> {
        // A new color only latches through the same idle → color → static sequence
        // as a manual apply, and the LEDs go dark while idle. Keep that sequence but
        // drop the 20 ms settle time inside it, so the dark window is a few
        // milliseconds instead of ~100 ms per frame.
        let [red, green, blue] = color;
        let short = Duration::from_millis(2);
        self.write_with_delay(0x2e, &mut [0], short)?;
        self.write_with_delay(MODE_REGISTER, &mut [MODE_IDLE], short)?;
        self.write_with_delay(0x30, &mut [red], short)?;
        self.write_with_delay(0x31, &mut [green], short)?;
        self.write_with_delay(0x32, &mut [blue], short)?;
        self.write(MODE_REGISTER, &mut [MODE_STATIC])
    }
}

fn check(status: i32, operation: &str) -> Result<(), String> {
    if status == 0 {
        Ok(())
    } else {
        Err(format!("NVAPI ({operation}): code {status}."))
    }
}

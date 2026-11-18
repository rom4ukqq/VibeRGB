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

pub const GPU_ID: &str = "msi-4070-super-mlg";
pub const GPU_NAME: &str = "MSI RTX 4070 SUPER Gaming X Slim MLG";
const NVIDIA_DEVICE: u32 = 0x2783_10de;
const MSI_SUBSYSTEM: u32 = 0xe134_1462;
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

    fn find_gpu(&self) -> Result<GpuHandle, String> {
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
        let mut matches = handles.iter().copied().filter(|handle| {
            let (mut device, mut subsystem, mut revision, mut extended) = (0, 0, 0, 0);
            let result = unsafe {
                (self.pci_ids)(
                    *handle,
                    &mut device,
                    &mut subsystem,
                    &mut revision,
                    &mut extended,
                )
            };
            result == 0 && device == NVIDIA_DEVICE && subsystem == MSI_SUBSYSTEM
        });
        let handle = matches
            .next()
            .ok_or("GPU with PCI ID 10DE:2783 / 1462:E134 not found.")?;
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
        let handle = nvapi.find_gpu()?;
        nvapi.transfer(handle, MODE_REGISTER, &mut [0], false)?;
        let device = Device {
            id: GPU_ID.into(),
            name: GPU_NAME.into(),
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
        self.nvapi.transfer(self.handle, register, values, true)?;
        thread::sleep(Duration::from_millis(20));
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
        self.prepare_mode()?;
        self.color(color)?;
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

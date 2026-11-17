use std::{mem::transmute, os::windows::ffi::OsStrExt, ptr::null_mut, sync::Arc};

use windows_sys::Win32::{
    Foundation::{CloseHandle, FreeLibrary, HANDLE, HMODULE, WAIT_ABANDONED, WAIT_OBJECT_0},
    System::{
        LibraryLoader::{
            GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        },
        Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
    },
};

use super::{dim, pawnio, Controller, Device, Rgb, Zone};

type Open = unsafe extern "system" fn(*mut HANDLE) -> i32;
type Load = unsafe extern "system" fn(HANDLE, *const u8, usize) -> i32;
type Execute = unsafe extern "system" fn(
    HANDLE,
    *const i8,
    *const u64,
    usize,
    *mut u64,
    usize,
    *mut usize,
) -> i32;
type Close = unsafe extern "system" fn(HANDLE) -> i32;

const BYTE_DATA: u64 = 2;
const WORD_DATA: u64 = 3;
const APPLY: u16 = 0x80a0;
const SMBUS_MODULE: &[u8] = include_bytes!("../../resources/pawnio/SmbusPIIX4.bin");

struct Library(HMODULE);

impl Drop for Library {
    fn drop(&mut self) {
        unsafe { FreeLibrary(self.0) };
    }
}

struct Bus {
    _library: Library,
    handle: HANDLE,
    mutex: HANDLE,
    execute: Execute,
    close: Close,
}

// Calls on a shared handle are serialized by the SMBus mutex across entire transactions.
// The library and Windows handles stay open until the last controller is dropped.
unsafe impl Send for Bus {}
unsafe impl Sync for Bus {}

impl Bus {
    fn open() -> Result<Self, String> {
        let dll = pawnio::library_path()?;
        if !dll.try_exists().map_err(|error| error.to_string())? {
            return Err("RGB memory control on this board requires PawnIO.".into());
        }
        let filename: Vec<_> = dll.as_os_str().encode_wide().chain([0]).collect();
        // SAFETY: The DLL is loaded from its installation directory and uses the PawnIOLib ABI.
        unsafe {
            let handle = LoadLibraryExW(
                filename.as_ptr(),
                null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            );
            if handle.is_null() {
                return Err("Could not load PawnIOLib.dll.".into());
            }
            let library = Library(handle);
            let symbol = |name: &std::ffi::CStr| {
                GetProcAddress(library.0, name.as_ptr().cast())
                    .ok_or("The installed version of PawnIO is missing the required interface.")
            };
            let open = transmute::<_, Open>(symbol(c"pawnio_open")?);
            let load = transmute::<_, Load>(symbol(c"pawnio_load")?);
            let execute = transmute::<_, Execute>(symbol(c"pawnio_execute")?);
            let close = transmute::<_, Close>(symbol(c"pawnio_close")?);
            let mut handle = null_mut();
            status(open(&mut handle), "PawnIO open")?;
            let mutex_name: Vec<_> = r"Global\Access_SMBUS.HTP.Method"
                .encode_utf16()
                .chain([0])
                .collect();
            let mutex = CreateMutexW(std::ptr::null(), 0, mutex_name.as_ptr());
            if mutex.is_null() {
                close(handle);
                return Err("SMBus: could not open the system mutex.".into());
            }
            let bus = Self {
                _library: library,
                handle,
                mutex,
                execute,
                close,
            };
            let guard = bus.lock()?;
            status(
                load(handle, module.as_ptr(), module.len()),
                "SmbusPIIX4 load",
            )?;
            bus.call(c"ioctl_piix4_port_sel", &[0], &mut [0])?;
            drop(guard);
            Ok(bus)
        }
    }

    fn lock(&self) -> Result<BusLock<'_>, String> {
        match unsafe { WaitForSingleObject(self.mutex, 500) } {
            WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(BusLock(self)),
            _ => Err("The SMBus is busy.".into()),
        }
    }

    fn call(&self, name: &std::ffi::CStr, input: &[u64], output: &mut [u64]) -> Result<(), String> {
        let mut returned = 0;
        // SAFETY: Buffers remain valid during the synchronous call. Sizes are in u64 entries.
        let result = unsafe {
            (self.execute)(
                self.handle,
                name.as_ptr(),
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
                output.len(),
                &mut returned,
            )
        };
        status(result, "SMBus")
    }

    fn read_byte(&self, address: u8, command: u8) -> Result<u8, String> {
        let mut output = [0; 5];
        self.call(
            c"ioctl_smbus_xfer",
            &[address.into(), 1, command.into(), BYTE_DATA],
            &mut output,
        )?;
        Ok(output[0] as u8)
    }

    fn write(&self, address: u8, command: u8, size: u64, data: u16) -> Result<(), String> {
        self.call(
            c"ioctl_smbus_xfer",
            &[address.into(), 0, command.into(), size, data.into()],
            &mut [],
        )
    }

    fn read_register(&self, address: u8, register: u16) -> Result<u8, String> {
        self.write(address, 0, WORD_DATA, register.swap_bytes())?;
        self.read_byte(address, 0x81)
    }

    fn write_register(&self, address: u8, register: u16, value: u8) -> Result<(), String> {
        self.write(address, 0, WORD_DATA, register.swap_bytes())?;
        self.write(address, 0x01, BYTE_DATA, value.into())
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        unsafe {
            (self.close)(self.handle);
            CloseHandle(self.mutex);
        }
    }
}

struct BusLock<'a>(&'a Bus);
impl Drop for BusLock<'_> {
    fn drop(&mut self) {
        unsafe { ReleaseMutex(self.0.mutex) };
    }
}

fn status(code: i32, operation: &str) -> Result<(), String> {
    if code >= 0 {
        Ok(())
    } else {
        Err(format!("{operation}: {:#010x}.", code as u32))
    }
}

pub struct Dimm {
    bus: Arc<Bus>,
    device: Device,
    address: u8,
    direct_register: u16,
    leds: u8,
}

impl Dimm {
    pub fn discover() -> Result<Vec<Self>, String> {
        let bus = Arc::new(Bus::open()?);
        let _guard = bus.lock()?;
        let devices = (0x70..=0x76)
            .filter_map(|address| Self::identify(&bus, address))
            .collect::<Vec<_>>();
        if devices.is_empty() {
            return Err(
                "PawnIO is available. No supported ENE DDR4 RGB controller was found.".into(),
            );
        }
        Ok(devices)
    }

    fn identify(bus: &Arc<Bus>, address: u8) -> Option<Self> {
        // Check the read-only ENE signature before addressing its internal registers.
        for value in 0..16 {
            if bus.read_byte(address, 0xa0 + value).ok()? != value {
                return None;
            }
        }
        let signature = (0..16)
            .map(|offset| bus.read_register(address, 0x1000 + offset))
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        let signature = signature.split(|byte| *byte == 0).next()?;
        let (direct_register, max_leds) = match signature {
            b"AUDA0-E6K5-0101" => (0x8100, 10),
            b"DIMM_LED-0102" | b"DIMM_LED-0103" => (0x8000, 5),
            _ => return None,
        };
        let leds = bus.read_register(address, 0x1c02).ok()?;
        if !(1..=max_leds).contains(&leds) {
            return None;
        }
        let device = Device {
            id: format!("dimm-ene-{address:02x}"),
            name: format!("ENE memory · {address:02X}"),
            detail: "AMD SMBus · PawnIO".into(),
            needs_pawnio_install: false,
            zones: vec![Zone {
                id: "dimm".into(),
                name: "Module lighting".into(),
                effects: vec![],
                direct: true,
            }],
        };
        Some(Self {
            bus: Arc::clone(bus),
            device,
            address,
            direct_register,
            leds,
        })
    }

    fn pixels(&self, [red, green, blue]: Rgb) -> Result<(), String> {
        for led in 0..u16::from(self.leds) {
            let register = self.direct_register + led * 3;
            for (offset, value) in [(0, red), (1, blue), (2, green)] {
                self.bus
                    .write_register(self.address, register + offset, value)?;
            }
        }
        Ok(())
    }
}

impl Controller for Dimm {
    fn device(&self) -> &Device {
        &self.device
    }

    fn apply(&mut self, _zone: &str, mode: &str, color: Rgb, brightness: u8) -> Result<(), String> {
        let color = match mode {
            "off" => [0; 3],
            "solid" => dim(color, brightness),
            _ => return Err("Mode unavailable on this memory module.".into()),
        };
        let _guard = self.bus.lock()?;
        self.bus.write_register(self.address, 0x8020, 1)?;
        self.bus.write_register(self.address, APPLY, 1)?;
        self.pixels(color)
    }

    fn frame(&mut self, _zone: &str, color: Rgb) -> Result<(), String> {
        let _guard = self.bus.lock()?;
        self.pixels(color)
    }
}

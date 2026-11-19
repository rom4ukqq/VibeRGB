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
const DEFAULT_ENE_ADDRESS: u8 = 0x77;
const SLOT_SELECT: u16 = 0x80f8;
const I2C_ADDRESS: u16 = 0x80f9;
const NO_SUCH_DEVICE: u32 = 0x8007_01b1;
pub(super) const NEEDS_ADMIN: &str = "Run VibeRGB as administrator to control RGB memory.";
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
            let result = open(&mut handle);
            if matches!(result as u32, 0x80070005 | 0xc0000022 | 0xd0000022) {
                return Err(NEEDS_ADMIN.into());
            }
            status(result, "PawnIO open")?;
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
                load(handle, SMBUS_MODULE.as_ptr(), SMBUS_MODULE.len()),
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
        status(self.call_status(name, input, output), "SMBus")
    }

    fn call_status(&self, name: &std::ffi::CStr, input: &[u64], output: &mut [u64]) -> i32 {
        let mut returned = 0;
        // SAFETY: Buffers remain valid during the synchronous call. Sizes are in u64 entries.
        unsafe {
            (self.execute)(
                self.handle,
                name.as_ptr(),
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
                output.len(),
                &mut returned,
            )
        }
    }

    fn address_available(&self, address: u8) -> Result<bool, String> {
        // SMBus Receive Byte checks whether an address responds without writing data.
        let result = self.call_status(c"ioctl_smbus_xfer", &[address.into(), 1, 0, 1], &mut [0]);
        if result as u32 == NO_SUCH_DEVICE {
            Ok(true)
        } else {
            status(result, "SMBus")?;
            Ok(false)
        }
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
    /// Last color written in direct mode, used to skip unchanged channels.
    shown: Option<Rgb>,
}

impl Dimm {
    pub fn discover(expected: usize) -> Result<(Vec<Self>, Option<String>), String> {
        let bus = Arc::new(Bus::open()?);
        let _guard = bus.lock()?;
        let mut devices = Vec::new();
        let mut issues = Vec::new();
        for address in 0x70..DEFAULT_ENE_ADDRESS {
            match Self::identify(&bus, address) {
                Ok(Some(device)) => devices.push(device),
                Ok(None) => {}
                Err(error) => issues.push(format!("SMBus {address:#04x} — {error}")),
            }
        }
        if let Err(error) = Self::resolve_default_address(&bus, expected, &mut devices) {
            issues.push(error);
        }
        let warning = (!issues.is_empty()).then(|| issues.join(" "));
        if devices.is_empty() {
            if let Some(reason) = warning {
                return Err(reason);
            }
            return Err(
                "PawnIO is available. No supported ENE DDR4 RGB controller was found.".into(),
            );
        }
        Ok((devices, warning))
    }

    fn resolve_default_address(
        bus: &Arc<Bus>,
        expected: usize,
        devices: &mut Vec<Self>,
    ) -> Result<(), String> {
        // ENE DDR4 DIMMs can share 0x77 until the slot selector and volatile address are set.
        // Only remap a controller after matching an ENE firmware and LED count at 0x77.
        for slot in 0..8 {
            if devices.len() >= expected || Self::identify(bus, DEFAULT_ENE_ADDRESS)?.is_none() {
                break;
            }
            let mut free_address = None;
            for address in 0x70..DEFAULT_ENE_ADDRESS {
                if bus.address_available(address)? {
                    free_address = Some(address);
                    break;
                }
            }
            let address = free_address.ok_or("No free SMBus address is available for the ENE module.")?;
            bus.write_register(DEFAULT_ENE_ADDRESS, SLOT_SELECT, slot)?;
            let write = bus.write_register(DEFAULT_ENE_ADDRESS, I2C_ADDRESS, address << 1);
            // A controller can switch addresses before the original transaction finishes.
            match Self::identify(bus, address) {
                Ok(Some(module)) => devices.push(module),
                other => {
                    write?;
                    other?;
                }
            }
        }
        Ok(())
    }

    fn identify(bus: &Arc<Bus>, address: u8) -> Result<Option<Self>, String> {
        // Check the read-only ENE signature before addressing its internal registers.
        let mut first = [0; 5];
        let result = bus.call_status(c"ioctl_smbus_xfer", &[address.into(), 1, 0xa0, BYTE_DATA], &mut first);
        if result as u32 == NO_SUCH_DEVICE {
            return Ok(None);
        }
        status(result, "SMBus")?;
        if first[0] as u8 != 0 {
            return Ok(None);
        }
        for value in 1..16 {
            if bus.read_byte(address, 0xa0 + value)? != value {
                return Ok(None);
            }
        }
        let signature = (0..16)
            .map(|offset| bus.read_register(address, 0x1000 + offset))
            .collect::<Result<Vec<_>, _>>()?;
        let signature = signature.split(|byte| *byte == 0).next().unwrap_or_default().trim_ascii_end();
        let (direct_register, max_leds) = match signature {
            b"AUDA0-E6K5-0101" => (0x8100, 10),
            b"DIMM_LED-0102" | b"DIMM_LED-0103" => (0x8000, 5),
            _ => return Ok(None),
        };
        let leds = bus.read_register(address, 0x1c02)?;
        if !(1..=max_leds).contains(&leds) {
            return Ok(None);
        }
        let device = Device {
            id: format!("dimm-ene-{address:02x}"),
            name: format!("ENE memory · {address:02X}"),
            detail: "AMD SMBus · PawnIO".into(),
            needs_pawnio_install: false,
            needs_pawnio_elevation: false,
            zones: vec![Zone {
                id: "dimm".into(),
                name: "Module lighting".into(),
                effects: vec![],
                direct: true,
            }],
        };
        Ok(Some(Self {
            bus: Arc::clone(bus),
            device,
            address,
            direct_register,
            leds,
            shown: None,
        }))
    }

    fn pixels(&mut self, color: Rgb) -> Result<(), String> {
        let [red, green, blue] = color;
        let previous = self.shown.take();
        // Every LED shows the same color, so a channel that didn't change can be
        // skipped on all of them. Each register write costs two SMBus transactions.
        let changed = |index: usize| previous.map_or(true, |old| old[index] != color[index]);
        for led in 0..u16::from(self.leds) {
            let register = self.direct_register + led * 3;
            for (offset, value, index) in [(0, red, 0), (1, blue, 2), (2, green, 1)] {
                if changed(index) {
                    self.bus.write_register(self.address, register + offset, value)?;
                }
            }
        }
        self.shown = Some(color);
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
        let bus = Arc::clone(&self.bus);
        let _guard = bus.lock()?;
        bus.write_register(self.address, 0x8020, 1)?;
        bus.write_register(self.address, APPLY, 1)?;
        self.shown = None;
        self.pixels(color)
    }

    fn frame(&mut self, _zone: &str, color: Rgb) -> Result<(), String> {
        let bus = Arc::clone(&self.bus);
        let _guard = bus.lock()?;
        self.pixels(color)
    }
}

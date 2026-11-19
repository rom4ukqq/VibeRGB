use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io,
    mem::size_of,
    os::windows::{ffi::OsStringExt, fs::OpenOptionsExt, io::AsRawHandle},
    ptr::{null, null_mut},
};

use windows_sys::Win32::{
    Devices::{
        DeviceAndDriverInstallation::{
            SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
            SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO,
            SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
        },
        HumanInterfaceDevice::HidD_GetHidGuid,
    },
    Foundation::{CloseHandle, ERROR_IO_PENDING, ERROR_NO_MORE_ITEMS, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{
        ReadFile, WriteFile, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE,
    },
    System::{
        Threading::CreateEventW,
        IO::{CancelIoEx, GetOverlappedResult, GetOverlappedResultEx, OVERLAPPED},
    },
};

use super::{dim, smbios::Firmware, Controller, Device, Effect, Rgb, Zone};

const BOARDS: &[(&str, &str)] = &[
    ("B550M Pro4", "asrock-b550m-pro4"),
    ("B550 Pro4", "asrock-b550-pro4"),
    ("B550 Steel Legend", "asrock-b550-steel-legend"),
    ("B550 Phantom Gaming-ITX/ax", "asrock-b550-phantom-gaming-itx-ax"),
];

/// Returns the board model, a stable device ID and whether the board is one of
/// the tested models. Polychrome USB itself is detected by its HID ID on any board.
pub fn identify(firmware: &Firmware) -> (String, String, bool) {
    if let Some((model, id)) = BOARDS.iter().copied().find(|(model, _)| firmware.board_matches(model)) {
        return (model.into(), id.into(), true);
    }
    let model = firmware.board_name().unwrap_or_else(|| "Polychrome".into());
    let slug: String = model
        .to_ascii_lowercase()
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() { character } else { '-' })
        .collect();
    let slug = slug.split('-').filter(|part| !part.is_empty()).collect::<Vec<_>>().join("-");
    (model, format!("asrock-{slug}"), false)
}

const MODE_STATIC: u8 = 0x01;
const MODE_DIRECT: u8 = 0x0f;
const SET_ZONE: u8 = 0x10;
const WRITE_HEADER: u8 = 0x15;
const RGSWAP_CONFIG: u8 = 0x03;
const DEFAULT_SPEED: u8 = 0xe0;
const ADDRESSABLE_MAX_LEDS: usize = 100;
const ZONES: [&str; 8] = [
    "RGB LED 1",
    "RGB LED 2",
    "ARGB 1",
    "ARGB 2",
    "PCH",
    "I/O cover",
    "Motherboard",
    "ARGB 3 / Audio",
];
const EFFECTS: [Effect; 3] = [
    Effect {
        id: "breathing",
        name: "Breathing",
        color: true,
        brightness: true,
    },
    Effect {
        id: "spectrum",
        name: "Color cycle",
        color: false,
        brightness: false,
    },
    Effect {
        id: "rainbow",
        name: "Rainbow",
        color: false,
        brightness: false,
    },
];

pub struct Board {
    hid: File,
    device: Device,
    red_green_order: u8,
    channels: Vec<usize>,
    counts: [u8; 8],
    colors: [Rgb; 8],
    /// Channels currently in Direct mode, one bit per channel.
    direct: u8,
}

impl Board {
    pub fn open(id: &str, name: &str) -> Result<Self, String> {
        let hid = open_hid().map_err(|error| format!("Polychrome USB: {error}"))?;
        let availability = read_configuration(&hid, 1)?[0];
        let counts = read_configuration(&hid, 2)?;
        let red_green_order = read_configuration(&hid, 3)?[0];
        let channels: Vec<usize> = (0..ZONES.len())
            .filter(|&channel| availability & (1 << channel) != 0 && counts[channel] != 0x1e)
            .collect();
        let zones = ZONES
            .iter()
            .enumerate()
            .filter(|(channel, _)| channels.contains(channel))
            .map(|(channel, name)| {
                Zone {
                    id: channel.to_string(),
                    name: (*name).into(),
                    effects: EFFECTS.to_vec(),
                    // Every Polychrome channel accepts Direct mode (OpenRGB exposes it on all).
                    direct: true,
                }
            })
            .collect::<Vec<_>>();
        if zones.is_empty() {
            return Err("The Polychrome controller reported no available zones.".into());
        }
        let device = Device {
            id: id.into(),
            name: name.into(),
            detail: "Polychrome USB".into(),
            needs_pawnio_install: false,
            needs_pawnio_elevation: false,
            zones,
        };
        Ok(Self {
            hid,
            device,
            red_green_order,
            channels,
            counts,
            colors: [[0; 3]; 8],
            direct: 0,
        })
    }

    fn channel_color(&self, channel: usize, [red, green, blue]: Rgb) -> Rgb {
        if self.red_green_order & (1 << channel) != 0 {
            [red, green, blue]
        } else {
            [green, red, blue]
        }
    }

    fn write_mode(&self, channel: usize, mode: u8, color: Rgb) -> Result<(), String> {
        let [first, second, blue] = self.channel_color(channel, color);
        let mut report = [0; 65];
        report[1] = SET_ZONE;
        report[3..10].copy_from_slice(&[channel as u8, mode, first, second, blue, DEFAULT_SPEED, 0xff]);
        exchange(&self.hid, &report).map(|_| ())
    }

    /// The controller's own R/G swap makes Direct mode flash on every update
    /// (as noted in OpenRGB), so it is off while any channel is in Direct mode
    /// and the swap is done here instead. The original setting is restored after.
    fn hardware_swap(&self, enabled: bool) -> Result<(), String> {
        let mut report = [0; 65];
        report[1] = WRITE_HEADER;
        report[3] = RGSWAP_CONFIG;
        report[4] = if enabled { self.red_green_order } else { 0 };
        exchange(&self.hid, &report).map(|_| ())
    }

    /// Streams every LED of every channel, matching OpenRGB's Direct update.
    /// Channels that are not in Direct mode ignore their part of the stream.
    fn write_direct(&self) -> Result<(), String> {
        let mut bytes = Vec::new();
        let mut capacity = 0;
        for &channel in &self.channels {
            let count = usize::from(self.counts[channel]);
            let color = self.channel_color(channel, self.colors[channel]);
            for _ in 0..count {
                bytes.extend_from_slice(&color);
            }
            capacity += if matches!(channel, 2 | 3 | 7) { ADDRESSABLE_MAX_LEDS } else { count };
        }
        bytes.resize((capacity + 8) * 3, 0);
        let mut report = [0; 65];
        report[1] = SET_ZONE;
        report[3] = 0xff;
        report[4] = 0xe3;
        report[7] = 0xff;
        report[0x40] = 0x65;
        let (mut offset, mut chunk) = (9, 54);
        let mut sent = 0;
        while sent < bytes.len() {
            let length = chunk.min(bytes.len() - sent);
            report[offset..offset + length].copy_from_slice(&bytes[sent..sent + length]);
            sent += length;
            exchange(&self.hid, &report)?;
            report = [0; 65];
            report[1] = SET_ZONE;
            report[3] = 0xff;
            report[4] = 0xe4;
            report[0x40] = 0x65;
            (offset, chunk) = (5, 57);
        }
        Ok(())
    }
}

impl Drop for Board {
    fn drop(&mut self) {
        if self.direct != 0 {
            let _ = self.hardware_swap(true);
        }
    }
}

fn channel(zone: &str) -> Result<usize, String> {
    zone.parse::<usize>()
        .ok()
        .filter(|&value| value < ZONES.len())
        .ok_or_else(|| "Invalid Polychrome zone.".into())
}

impl Controller for Board {
    fn device(&self) -> &Device {
        &self.device
    }

    fn apply(&mut self, zone: &str, mode: &str, color: Rgb, brightness: u8) -> Result<(), String> {
        let channel = channel(zone)?;
        let mode = match (mode, brightness) {
            ("off", _) | (_, 0) => 0x00,
            ("solid", _) => MODE_STATIC,
            ("breathing", _) => 0x02,
            ("spectrum", _) => 0x04,
            ("rainbow", _) => 0x0e,
            _ => return Err("Mode unavailable on Polychrome.".into()),
        };
        let color = dim(color, brightness);
        let remaining = self.direct & !(1 << channel);
        if self.direct != 0 && remaining == 0 {
            self.hardware_swap(true)?;
        }
        self.direct = remaining;
        self.colors[channel] = color;
        self.write_mode(channel, mode, color)
    }

    fn begin_animation(&mut self, zone: &str, color: Rgb) -> Result<(), String> {
        let channel = channel(zone)?;
        if self.direct == 0 {
            self.hardware_swap(false)?;
        }
        self.direct |= 1 << channel;
        self.colors[channel] = color;
        self.write_mode(channel, MODE_DIRECT, color)?;
        self.write_direct()
    }

    fn frame(&mut self, zone: &str, color: Rgb) -> Result<(), String> {
        self.colors[channel(zone)?] = color;
        self.write_direct()
    }

    fn frames(&mut self, updates: &[(&str, Rgb)]) -> Result<Vec<(String, String)>, String> {
        // One Direct stream carries every channel, so send it once per batch.
        for &(zone, color) in updates {
            self.colors[channel(zone)?] = color;
        }
        self.write_direct()?;
        Ok(Vec::new())
    }
}

fn read_configuration(hid: &File, page: u8) -> Result<[u8; 8], String> {
    let mut report = [0; 65];
    report[1] = 0x14;
    report[3] = page;
    let reply = exchange(hid, &report)?;
    // Windows includes the leading report ID. HIDAPI removes it for unnumbered input reports.
    if reply[0] != 0 {
        return Err("Unexpected Polychrome HID response.".into());
    }
    Ok(reply[5..13].try_into().unwrap())
}

fn exchange(hid: &File, report: &[u8; 65]) -> Result<[u8; 65], String> {
    let mut output = *report;
    let mut input = [0; 65];
    let sent =
        transfer(hid, &mut output, true).map_err(|error| format!("Polychrome write: {error}"))?;
    if sent != output.len() {
        return Err("Incomplete Polychrome write.".into());
    }
    let read = transfer(hid, &mut input, false)
        .map_err(|error| format!("Polychrome response: {error}"))?;
    if read != input.len() {
        return Err("Incomplete Polychrome response.".into());
    }
    Ok(input)
}

fn transfer(hid: &File, buffer: &mut [u8], write: bool) -> io::Result<usize> {
    // SAFETY: The buffer, event and OVERLAPPED stay alive until the request completes or is cancelled.
    unsafe {
        let handle = hid.as_raw_handle();
        let event = CreateEventW(null(), 1, 0, null());
        if event.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut request = OVERLAPPED {
            hEvent: event,
            ..Default::default()
        };
        let mut count = 0;
        let started = if write {
            WriteFile(
                handle,
                buffer.as_ptr(),
                buffer.len() as u32,
                null_mut(),
                &mut request,
            )
        } else {
            ReadFile(
                handle,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                null_mut(),
                &mut request,
            )
        };
        let result = if started == 0
            && io::Error::last_os_error().raw_os_error() != Some(ERROR_IO_PENDING as i32)
        {
            Err(io::Error::last_os_error())
        } else if GetOverlappedResultEx(handle, &request, &mut count, 350, 0) != 0 {
            Ok(count as usize)
        } else {
            let error = io::Error::last_os_error();
            CancelIoEx(handle, &request);
            GetOverlappedResult(handle, &request, &mut count, 1);
            Err(error)
        };
        CloseHandle(event);
        result
    }
}

struct DeviceList(HDEVINFO);

impl Drop for DeviceList {
    fn drop(&mut self) {
        // SAFETY: The list owns the valid handle returned by SetupDiGetClassDevsW.
        unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

fn open_hid() -> io::Result<File> {
    const CONTROLLER_PATH: &str = "hid#vid_26ce&pid_01a2";
    // SAFETY: SetupAPI writes into aligned buffers. Only a path matching the target HID is opened.
    unsafe {
        let mut hid_class = std::mem::zeroed();
        HidD_GetHidGuid(&mut hid_class);
        let raw_list = SetupDiGetClassDevsW(
            &hid_class,
            null(),
            null_mut(),
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        );
        if raw_list == INVALID_HANDLE_VALUE as HDEVINFO {
            return Err(io::Error::last_os_error());
        }
        let list = DeviceList(raw_list);
        let mut open_error = None;
        for index in 0.. {
            let mut interface = SP_DEVICE_INTERFACE_DATA {
                cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..Default::default()
            };
            if SetupDiEnumDeviceInterfaces(list.0, null(), &hid_class, index, &mut interface) == 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() == Some(ERROR_NO_MORE_ITEMS as i32) {
                    break;
                }
                return Err(error);
            }
            let mut storage = [0u32; 1024];
            let detail = storage
                .as_mut_ptr()
                .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
            (*detail).cbSize = if cfg!(target_pointer_width = "64") {
                8
            } else {
                6
            };
            if SetupDiGetDeviceInterfaceDetailW(
                list.0,
                &interface,
                detail,
                size_of_val(&storage) as u32,
                null_mut(),
                null_mut(),
            ) == 0
            {
                continue;
            }
            let wide = std::slice::from_raw_parts(
                std::ptr::addr_of!((*detail).DevicePath).cast::<u16>(),
                2046,
            );
            let Some(end) = wide.iter().position(|character| *character == 0) else {
                continue;
            };
            let path = OsString::from_wide(&wide[..end]);
            if !path
                .to_string_lossy()
                .to_ascii_lowercase()
                .starts_with(&format!(r"\\?\{CONTROLLER_PATH}"))
            {
                continue;
            }
            match OpenOptions::new()
                .read(true)
                .write(true)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .custom_flags(FILE_FLAG_OVERLAPPED)
                .open(path)
            {
                Ok(file) => return Ok(file),
                Err(error) => open_error = Some(error),
            }
        }
        Err(open_error
            .unwrap_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HID 26CE:01A2 not found")))
    }
}

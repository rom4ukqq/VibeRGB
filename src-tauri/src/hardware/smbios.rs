use super::Device;
use windows_sys::Win32::System::{
    Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_BINARY},
    SystemInformation::GetSystemFirmwareTable,
};

#[derive(Default)]
pub struct Firmware {
    bytes: Vec<u8>,
}

impl Firmware {
    pub fn read() -> Self {
        let bytes = firmware_table().or_else(registry_table).unwrap_or_default();
        let mut firmware = Self { bytes };
        if let Some(length) = firmware.bytes.get(4..8) {
            let table_size = u32::from_le_bytes(length.try_into().unwrap()) as usize;
            firmware.bytes.truncate(8usize.saturating_add(table_size));
        }
        firmware
    }

    pub fn board_name(&self) -> Option<String> {
        self.structures()
            .find(|(fields, _)| fields[0] == 2)
            .map(|(fields, strings)| field(fields, strings, 5).trim().to_owned())
            .filter(|name| !name.is_empty())
    }

    pub fn board_matches(&self, model: &str) -> bool {
        self.structures().any(|(fields, strings)| {
            fields[0] == 2 && field(fields, strings, 5).eq_ignore_ascii_case(model)
        })
    }

    pub fn memory(&self) -> Vec<Device> {
        self.structures()
            .filter(|(fields, _)| {
                fields[0] == 17 && fields.get(12..14).is_some_and(|size| size != [0, 0])
            })
            .map(|(fields, strings)| {
                let slot = field(fields, strings, 16);
                let bank = field(fields, strings, 17);
                let vendor = field(fields, strings, 23);
                let part = field(fields, strings, 26);
                let unidentified = part.is_empty()
                    || part.starts_with("DDR")
                    || part.eq_ignore_ascii_case("unknown");
                let detail = if unidentified {
                    "SMBIOS did not report the exact part number. The RGB controller is unconfirmed.".into()
                } else {
                    format!("{part} · This build does not support the RGB controller.")
                };
                Device {
                    id: format!("dimm-{:04x}", u16::from_le_bytes([fields[2], fields[3]])),
                    name: format!("Memory · {slot} {bank} {vendor}")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                    detail,
                    zones: Vec::new(),
                    needs_pawnio_install: false,
                    needs_pawnio_elevation: false,
                }
            })
            .collect()
    }

    fn structures(&self) -> impl Iterator<Item = (&[u8], &[u8])> {
        let mut data = self.bytes.get(8..).unwrap_or_default();
        std::iter::from_fn(move || {
            let length = usize::from(*data.get(1)?);
            if length < 4 || data[0] == 127 {
                return None;
            }
            let fields = data.get(..length)?;
            let strings = data.get(length..)?;
            let end = strings.windows(2).position(|pair| pair == [0, 0])?;
            data = &strings[end + 2..];
            Some((fields, &strings[..end]))
        })
    }
}

fn field<'a>(fields: &[u8], strings: &'a [u8], offset: usize) -> &'a str {
    fields
        .get(offset)
        .and_then(|index| index.checked_sub(1))
        .and_then(|index| strings.split(|byte| *byte == 0).nth(index as usize))
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .unwrap_or_default()
        .trim()
}

fn firmware_table() -> Option<Vec<u8>> {
    const RSMB: u32 = u32::from_be_bytes(*b"RSMB");
    // SAFETY: The first call requests the length. The second receives an owned buffer.
    let length = unsafe { GetSystemFirmwareTable(RSMB, 0, std::ptr::null_mut(), 0) };
    if length < 8 {
        return None;
    }
    let mut bytes = vec![0; length as usize];
    let read = unsafe { GetSystemFirmwareTable(RSMB, 0, bytes.as_mut_ptr().cast(), length) };
    (read == length).then_some(bytes)
}

fn registry_table() -> Option<Vec<u8>> {
    let key: Vec<_> = "SYSTEM\\CurrentControlSet\\Services\\mssmbios\\Data\0"
        .encode_utf16()
        .collect();
    let name: Vec<_> = "SMBiosData\0".encode_utf16().collect();
    let read = |buffer, length| {
        // SAFETY: The strings are null terminated and the caller provides or sizes the output buffer.
        unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_BINARY,
                std::ptr::null_mut(),
                buffer,
                length,
            )
        }
    };
    let mut length = 0;
    if read(std::ptr::null_mut(), &mut length) != 0 || length < 8 {
        return None;
    }
    let mut bytes = vec![0; length as usize];
    if read(bytes.as_mut_ptr().cast(), &mut length) != 0 {
        return None;
    }
    bytes.truncate(length as usize);
    Some(bytes)
}

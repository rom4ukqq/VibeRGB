use std::{
    env, fs, io,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::{Path, PathBuf},
    ptr::null,
    sync::Mutex,
};

use windows_sys::Win32::{
    Foundation::{ERROR_CANCELLED, ERROR_SUCCESS_REBOOT_REQUIRED, WAIT_OBJECT_0},
    System::{
        Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE},
        Threading::{
            GetExitCodeProcess, OpenProcess, WaitForSingleObject, INFINITE, PROCESS_SYNCHRONIZE,
        },
    },
    UI::{
        Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW},
        WindowsAndMessaging::SW_SHOWNORMAL,
    },
};

const SETUP: &[u8] = include_bytes!("../../resources/pawnio/PawnIO_setup.exe");
static INSTALLING: Mutex<()> = Mutex::new(());

pub(super) fn library_path() -> Result<PathBuf, String> {
    let program_files = env::var_os("ProgramW6432")
        .or_else(|| env::var_os("ProgramFiles"))
        .ok_or("The Program Files directory is unavailable.")?;
    Ok(PathBuf::from(program_files)
        .join("PawnIO")
        .join("PawnIOLib.dll"))
}

pub(super) fn is_missing() -> bool {
    library_path().is_ok_and(|path| matches!(path.try_exists(), Ok(false)))
}

pub(crate) fn install() -> Result<&'static str, String> {
    let _installation = INSTALLING
        .try_lock()
        .map_err(|_| "PawnIO installation is already running.")?;
    if library_path()?
        .try_exists()
        .map_err(|error| error.to_string())?
    {
        return Ok("installed");
    }

    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let setup = directory.path().join("PawnIO_setup.exe");
    fs::write(&setup, SETUP).map_err(|error| error.to_string())?;

    let Some(process) = launch_elevated(&setup, "")? else {
        return Ok("cancelled");
    };
    let handle = process.as_raw_handle();
    let mut exit_code = 0;
    // SAFETY: The child handle and temporary directory remain valid until setup exits.
    if unsafe {
        WaitForSingleObject(handle, INFINITE) != WAIT_OBJECT_0
            || GetExitCodeProcess(handle, &mut exit_code) == 0
    } {
        return Err(io::Error::last_os_error().to_string());
    }
    match exit_code {
        ERROR_SUCCESS_REBOOT_REQUIRED => Ok("restartRequired"),
        ERROR_CANCELLED => Ok("cancelled"),
        0 if library_path()?.is_file() => Ok("installed"),
        0 => Err("PawnIO was not installed.".into()),
        code => Err(format!("PawnIO installer exited with code {code}.")),
    }
}

pub(crate) fn restart_as_admin() -> Result<bool, String> {
    let exe = env::current_exe().map_err(|error| error.to_string())?;
    let args = format!("--wait-for-instance {}", std::process::id());
    Ok(launch_elevated(&exe, &args)?.is_some())
}

pub(crate) fn wait_for_previous_instance() -> bool {
    let mut args = env::args().skip(1);
    if args.next().as_deref() != Some("--wait-for-instance") {
        return true;
    }
    let Some(pid) = args.next().and_then(|id| id.parse::<u32>().ok()) else {
        return true;
    };
    // SAFETY: The child waits only on a handle opened for synchronization.
    unsafe {
        let parent = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if parent.is_null() {
            return true;
        }
        let parent = OwnedHandle::from_raw_handle(parent);
        WaitForSingleObject(parent.as_raw_handle(), 10_000) == WAIT_OBJECT_0
    }
}

fn launch_elevated(path: &Path, args: &str) -> Result<Option<OwnedHandle>, String> {
    let file: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
    let parameters: Vec<_> = args.encode_utf16().chain([0]).collect();
    let verb: Vec<_> = "runas".encode_utf16().chain([0]).collect();
    let mut request: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    request.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    request.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    request.lpVerb = verb.as_ptr();
    request.lpFile = file.as_ptr();
    request.lpParameters = parameters.as_ptr();
    request.nShow = SW_SHOWNORMAL;

    // SAFETY: The strings and request outlive ShellExecuteExW.
    unsafe {
        let com = CoInitializeEx(
            null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        );
        let launched = ShellExecuteExW(&mut request);
        let launch_error = (launched == 0).then(io::Error::last_os_error);
        if com >= 0 {
            CoUninitialize();
        }
        if let Some(error) = launch_error {
            return if error.raw_os_error() == Some(ERROR_CANCELLED as i32) {
                Ok(None)
            } else {
                Err(error.to_string())
            };
        }
        let handle = request.hProcess;
        if handle.is_null() {
            return Err("Could not track the elevated process.".into());
        }
        Ok(Some(OwnedHandle::from_raw_handle(handle)))
    }
}

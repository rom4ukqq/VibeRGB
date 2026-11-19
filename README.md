# VibeRGB

VibeRGB is a Windows desktop app for PC lighting. Its Rust backend talks directly to supported controllers, and its Tauri interface includes a shared animation timeline, brightness, speed, profiles, and a system tray.

## Use VibeRGB

- In **Lighting Studio → Sync**, select **Pulse**, **Spectrum**, or **Rainbow**, set the brightness and speed, then click **Apply**. Rainbow shifts the colors across zones on the shared timeline. Built-in effects under **Hardware** run on their own controller clocks.
- Closing the window hides it in the notification area. Use the tray icon to reopen it, or choose **Exit VibeRGB** to stop the animation engine and quit.
- **Settings → Language** switches between English and Russian. English is the default.

## Startup and updates

Settings lets you launch VibeRGB in the tray when you sign in to Windows, select a startup profile, or turn off the animated backdrop. Install the Windows setup once from [GitHub Releases](https://github.com/rom4ukqq/VibeRGB/releases/latest). VibeRGB checks for signed updates at startup and every six hours. Select **Settings → Updates → Install and restart** to install a new version.

For a release, update the package, Cargo and Tauri versions and their lockfiles together, then push the matching `vX.Y.Z` tag. GitHub Actions builds the Windows x64 NSIS installer, signs it using the repository secret `TAURI_SIGNING_PRIVATE_KEY`, and publishes it alongside its signature and `latest.json`.

## Hardware

| Device | Connection |
| --- | --- |
| ASRock B550M Pro4, B550 Pro4, B550 Steel Legend, B550 Phantom Gaming-ITX/ax | Polychrome USB HID (26CE:01A2), including lighting connected to the reported RGB/ARGB headers |
| MSI GeForce RTX 4070 SUPER Gaming X Slim MLG and Gaming X Slim White | NVIDIA NVAPI I²C (subsystems 1462:E134 and 1462:E131 respectively) |
| Supported ENE DDR4 modules on ASRock B550M Pro4 | AMD SMBus via [PawnIO](https://pawnio.eu/) and the signed [SmbusPIIX4 module](https://github.com/namazso/PawnIO.Modules/releases) |

If PawnIO is missing, select a memory entry and click **Install PawnIO**. The app includes the official installer and signed SMBus module. If Windows denies access to the installed driver, select the memory entry and click **Restart as administrator**. Approve the Windows elevation prompt. VibeRGB then detects the ENE modules at their individual lighting addresses. For portable distribution, include the notices from `src-tauri/resources/pawnio` (`COPYING` and `README.txt`) alongside the executable.

## Build

Install Node.js with npm, the Rust stable MSVC toolchain, Visual Studio C++ Build Tools, and WebView2 on Windows. Then run:

```powershell
npm ci
npm run tauri dev
```

Use `npm run tauri build` to produce the Windows installer. The [OpenRGB controllers](https://gitlab.com/CalcProgrammer1/OpenRGB/-/tree/master/Controllers) are a reference for the device protocols.

# VibeRGB

VibeRGB is a Windows desktop app for PC lighting. Its Rust backend talks directly to supported controllers, and its Tauri interface includes a shared animation timeline, brightness, speed, profiles, and a system tray.

## Use VibeRGB

- In **Lighting Studio → Sync**, select **Pulse** or **Spectrum**, set the brightness and speed, then click **Apply**. Built-in effects under **Hardware** run on their own controller clocks.
- Closing the window hides it in the notification area. Use the tray icon to reopen it, or choose **Exit VibeRGB** to stop the animation engine and quit.
- **Settings → Language** switches between English and Russian. English is the default.

## Hardware

| Device | Connection |
| --- | --- |
| ASRock B550M Pro4 | Polychrome USB HID |
| MSI GeForce RTX 4070 SUPER Gaming X Slim MLG | NVIDIA NVAPI I²C |
| Supported ENE DDR4 modules on ASRock B550M Pro4 | AMD SMBus via [PawnIO](https://pawnio.eu/) and the signed [SmbusPIIX4 module](https://github.com/namazso/PawnIO.Modules/releases) |

For ENE memory, place the signed module named `SmbusPIIX4.bin` beside the executable. Memory control is available when the ENE RGB controller is detected and PawnIO is accessible to the app.

## Build

Install Node.js with npm, the Rust stable MSVC toolchain, Visual Studio C++ Build Tools, and WebView2 on Windows. Then run:

```powershell
npm ci
npm run tauri dev
```

Use `npm run tauri build` to produce the Windows installer. The [OpenRGB controllers](https://gitlab.com/CalcProgrammer1/OpenRGB/-/tree/master/Controllers) are a reference for the device protocols.

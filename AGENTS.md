# VibeRGB

## Builds and releases

- Run VibeRGB builds in GitHub Actions. Do not build Windows executables or installers locally.
- Publish releases through the **Windows release** workflow in `.github/workflows/release.yml`. Use the signed NSIS installer and updater manifest produced by that workflow.
- For a requested release, update the version in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`. Push the release commit and matching `vX.Y.Z` tag.
- If the tag does not trigger a run, dispatch the workflow with that existing tag. Wait for publishing to succeed and share the GitHub release link.

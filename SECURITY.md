# Security

## Model
- Wallgates never runs executables from wallpapers.
- HTML wallpapers run in a sandboxed iframe with an opaque origin: no access to the app's IPC,
  files or other wallpapers. They are **offline** (Content-Security-Policy) unless the manifest
  declares the `network` capability.
- Live data (audio spectrum, cursor, media, system, weather) is only delivered to wallpapers
  that declare the matching capability, and only while they are visible.
- Packages and gallery downloads are checked for path traversal (zip-slip) and size limits.
- The app uses no hooks, services or code injection, and installs per user without admin rights.

## Reporting a vulnerability
Please open a private advisory: **Security → Report a vulnerability** on
<https://github.com/CerLioX17/WallGates/security>. Do not open a public issue for security problems.

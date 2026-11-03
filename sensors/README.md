# Wallgates Sensors

Optional helper that gives Wallgates CPU / RAM temperatures, fans and other motherboard sensors
using [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor) (MPL-2.0)
and the [PawnIO](https://pawnio.eu) driver (Microsoft-signed, shared by several hardware tools).

- Shipped inside the Wallgates installer, **off by default**.
- Enabled from Wallgates → Settings → Advanced sensors (one administrator prompt):
  installs PawnIO if missing and a hidden scheduled task that runs this helper at logon with the
  privileges needed to read sensors.
- The helper only writes a JSON file with sensor readings into the user's Wallgates data folder
  while Wallgates is running.
- Disabling it, or uninstalling Wallgates, removes the task and — if Wallgates installed it — PawnIO.

```
WallgatesSensors.exe --run --out <file.json>
WallgatesSensors.exe --install --user <DOMAIN\user> --out <file.json> [--pawnio <PawnIO_setup.exe>]
WallgatesSensors.exe --uninstall
```

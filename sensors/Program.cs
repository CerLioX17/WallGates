// Wallgates Sensors — hardware sensor helper for Wallgates (GPL-3.0-or-later).
// Uses LibreHardwareMonitorLib (MPL-2.0) with the PawnIO driver.
//
//   --run --out <file>                 read sensors every 2 s while Wallgates runs, write JSON
//   --install --user <u> --out <file> [--pawnio <setup.exe>]   (elevated) install driver + logon task
//   --uninstall                        (elevated) remove task, and PawnIO if Wallgates installed it
//
// Exit codes: 0 ok, 2 bad arguments, 10 PawnIO setup failed, 11 task creation failed, 12 not elevated.

using System;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Security.Principal;
using System.Text;
using System.Threading;
using LibreHardwareMonitor.Hardware;
using Microsoft.Win32;

namespace Wallgates.Sensors
{
    internal static class Program
    {
        private const string TaskName = "Wallgates Sensors";
        private const string MarkerKey = @"SOFTWARE\Wallgates\Sensors";

        [STAThread]
        private static int Main(string[] args)
        {
            try
            {
                if (Has(args, "--run")) return Run(Arg(args, "--out"));
                if (Has(args, "--install")) return Install(Arg(args, "--user"), Arg(args, "--out"), Arg(args, "--pawnio"));
                if (Has(args, "--uninstall")) return Uninstall();
                return 2;
            }
            catch (Exception e)
            {
                Log(e.ToString());
                return 1;
            }
        }

        private static bool Has(string[] a, string k) => a.Any(x => string.Equals(x, k, StringComparison.OrdinalIgnoreCase));

        private static string Arg(string[] a, string k)
        {
            for (int i = 0; i < a.Length - 1; i++)
                if (string.Equals(a[i], k, StringComparison.OrdinalIgnoreCase)) return a[i + 1];
            return null;
        }

        private static void Log(string line)
        {
            try
            {
                var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData), "Wallgates");
                Directory.CreateDirectory(dir);
                File.AppendAllText(Path.Combine(dir, "sensors.log"), DateTime.Now.ToString("s") + " " + line + Environment.NewLine);
            }
            catch { /* logging is best effort */ }
        }

        private static bool IsElevated() => new WindowsPrincipal(WindowsIdentity.GetCurrent()).IsInRole(WindowsBuiltInRole.Administrator);

        // ------------------------------------------------------------------ run

        private sealed class UpdateVisitor : IVisitor
        {
            public void VisitComputer(IComputer computer) => computer.Traverse(this);
            public void VisitHardware(IHardware hardware)
            {
                hardware.Update();
                foreach (var sub in hardware.SubHardware) sub.Accept(this);
            }
            public void VisitSensor(ISensor sensor) { }
            public void VisitParameter(IParameter parameter) { }
        }

        private static bool WallgatesRunning() =>
            Process.GetProcessesByName("Wallgates").Length > 0 || Process.GetProcessesByName("wallgates").Length > 0;

        private static int Run(string outFile)
        {
            if (string.IsNullOrEmpty(outFile)) return 2;
            // Only one helper per user session.
            using (var mutex = new Mutex(true, @"Local\WallgatesSensorsHelper", out bool owner))
            {
                if (!owner) return 0;
                Computer computer = null;
                var visitor = new UpdateVisitor();
                var lastSeen = DateTime.UtcNow;
                while (true)
                {
                    if (WallgatesRunning()) lastSeen = DateTime.UtcNow;
                    else if ((DateTime.UtcNow - lastSeen).TotalSeconds > 30)
                    {
                        // Exit when Wallgates is closed: frees the driver and never locks
                        // our files during updates. Wallgates restarts the task on launch.
                        computer?.Close();
                        return 0;
                    }
                    else { Thread.Sleep(2000); continue; }
                    if (computer == null)
                    {
                        computer = new Computer
                        {
                            IsCpuEnabled = true, IsGpuEnabled = true, IsMemoryEnabled = true,
                            IsMotherboardEnabled = true, IsStorageEnabled = true, IsControllerEnabled = true,
                        };
                        computer.Open();
                    }
                    try
                    {
                        computer.Accept(visitor);
                        Write(outFile, computer);
                    }
                    catch (Exception e) { Log("read: " + e.Message); }
                    Thread.Sleep(2000);
                }
            }
        }

        private static string Esc(string s)
        {
            var sb = new StringBuilder(s.Length + 8);
            foreach (var c in s)
            {
                if (c == '"' || c == '\\') sb.Append('\\').Append(c);
                else if (c < 0x20) sb.AppendFormat("\\u{0:x4}", (int)c);
                else sb.Append(c);
            }
            return sb.ToString();
        }

        private static void Collect(IHardware hw, StringBuilder sb, ref bool first)
        {
            foreach (var s in hw.Sensors)
            {
                if (s.Value == null || float.IsNaN(s.Value.Value) || float.IsInfinity(s.Value.Value)) continue;
                if (!first) sb.Append(',');
                first = false;
                sb.Append("{\"hw\":\"").Append(Esc(hw.Name))
                  .Append("\",\"hwType\":\"").Append(hw.HardwareType)
                  .Append("\",\"name\":\"").Append(Esc(s.Name))
                  .Append("\",\"type\":\"").Append(s.SensorType)
                  .Append("\",\"value\":").Append(s.Value.Value.ToString("0.###", CultureInfo.InvariantCulture)).Append('}');
            }
            foreach (var sub in hw.SubHardware) Collect(sub, sb, ref first);
        }

        private static void Write(string outFile, Computer computer)
        {
            var sb = new StringBuilder(16384);
            sb.Append("{\"version\":1,\"time\":").Append(DateTimeOffset.UtcNow.ToUnixTimeSeconds()).Append(",\"readings\":[");
            bool first = true;
            foreach (var hw in computer.Hardware) Collect(hw, sb, ref first);
            sb.Append("]}");
            var tmp = outFile + ".tmp";
            Directory.CreateDirectory(Path.GetDirectoryName(outFile));
            File.WriteAllText(tmp, sb.ToString(), new UTF8Encoding(false));
            if (File.Exists(outFile)) File.Replace(tmp, outFile, null); else File.Move(tmp, outFile);
        }

        // ------------------------------------------------------------------ install / uninstall

        private static int Exec(string file, string args, int timeoutMs = 180000)
        {
            var psi = new ProcessStartInfo(file, args) { UseShellExecute = false, CreateNoWindow = true };
            using (var p = Process.Start(psi))
            {
                if (!p.WaitForExit(timeoutMs)) { try { p.Kill(); } catch { } return -1; }
                return p.ExitCode;
            }
        }

        private static bool PawnIoInstalled()
        {
            using (var k = Registry.LocalMachine.OpenSubKey(@"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO")) return k != null;
        }

        private static string Xml(string s) => System.Security.SecurityElement.Escape(s);

        private static int Install(string user, string outFile, string pawnioSetup)
        {
            if (!IsElevated()) return 12;
            if (string.IsNullOrEmpty(user) || string.IsNullOrEmpty(outFile)) return 2;

            if (!PawnIoInstalled() && !string.IsNullOrEmpty(pawnioSetup) && File.Exists(pawnioSetup))
            {
                int code = Exec(pawnioSetup, "-install -silent");
                Log("PawnIO setup exit code " + code);
                if (!PawnIoInstalled()) return 10;
                using (var k = Registry.LocalMachine.CreateSubKey(MarkerKey)) k.SetValue("InstalledPawnIO", 1, RegistryValueKind.DWord);
            }

            var exe = Process.GetCurrentProcess().MainModule.FileName;
            var xml = $@"<?xml version=""1.0"" encoding=""UTF-16""?>
<Task version=""1.2"" xmlns=""http://schemas.microsoft.com/windows/2004/02/mit/task"">
  <RegistrationInfo><Author>Wallgates</Author><Description>Reads hardware sensors for Wallgates wallpapers. Remove it from Wallgates → Settings → Advanced sensors.</Description></RegistrationInfo>
  <Triggers><LogonTrigger><Enabled>true</Enabled><UserId>{Xml(user)}</UserId></LogonTrigger></Triggers>
  <Principals><Principal id=""Author""><UserId>{Xml(user)}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>HighestAvailable</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>true</Hidden>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context=""Author""><Exec><Command>{Xml(exe)}</Command><Arguments>{Xml("--run --out \"" + outFile + "\"")}</Arguments></Exec></Actions>
</Task>";
            var xmlPath = Path.Combine(Path.GetTempPath(), "wallgates-sensors-task.xml");
            File.WriteAllText(xmlPath, xml, Encoding.Unicode);
            int created = Exec("schtasks.exe", $"/Create /F /TN \"{TaskName}\" /XML \"{xmlPath}\"");
            try { File.Delete(xmlPath); } catch { }
            if (created != 0) { Log("schtasks create exit " + created); return 11; }
            Exec("schtasks.exe", $"/Run /TN \"{TaskName}\"");
            return 0;
        }

        private static int Uninstall()
        {
            if (!IsElevated()) return 12;
            Exec("schtasks.exe", $"/End /TN \"{TaskName}\"");
            Exec("schtasks.exe", $"/Delete /F /TN \"{TaskName}\"");
            var me = Process.GetCurrentProcess().Id;
            foreach (var p in Process.GetProcessesByName("WallgatesSensors").Where(p => p.Id != me))
            {
                try { p.Kill(); p.WaitForExit(5000); } catch { }
            }

            // Remove PawnIO only if Wallgates installed it (other tools may share the driver).
            bool ours;
            using (var k = Registry.LocalMachine.OpenSubKey(MarkerKey)) ours = k != null && Convert.ToInt32(k.GetValue("InstalledPawnIO", 0)) == 1;
            if (ours)
            {
                string cmd = null;
                using (var k = Registry.LocalMachine.OpenSubKey(@"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO"))
                    cmd = k?.GetValue("QuietUninstallString") as string ?? k?.GetValue("UninstallString") as string;
                if (!string.IsNullOrEmpty(cmd))
                {
                    // Split "C:\path\setup.exe" -uninstall into file + args.
                    string file, args;
                    if (cmd.StartsWith("\"")) { int end = cmd.IndexOf('"', 1); file = cmd.Substring(1, end - 1); args = cmd.Substring(end + 1).Trim(); }
                    else { int sp = cmd.IndexOf(".exe", StringComparison.OrdinalIgnoreCase) + 4; file = cmd.Substring(0, sp); args = cmd.Substring(sp).Trim(); }
                    if (!args.Contains("-silent")) args += " -silent";
                    if (!args.Contains("-uninstall")) args = "-uninstall " + args;
                    Log("PawnIO uninstall exit code " + Exec(file, args));
                }
                Registry.LocalMachine.DeleteSubKeyTree(@"SOFTWARE\Wallgates", false);
            }
            return 0;
        }
    }
}

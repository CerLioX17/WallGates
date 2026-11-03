; Wallgates installer hooks.

!macro NSIS_HOOK_PREUNINSTALL
  ; Uninstalling Wallgates also removes the optional advanced sensors
  ; (hidden logon task + the PawnIO driver if Wallgates installed it).
  ; Skipped during updates so enabled sensors survive an upgrade.
  ${If} $UpdateMode <> 1
    nsExec::ExecToStack 'schtasks.exe /Query /TN "Wallgates Sensors"'
    Pop $0
    Pop $1
    ${If} $0 == 0
    ${AndIf} ${FileExists} "$INSTDIR\sensors\WallgatesSensors.exe"
      ExecShellWait "runas" "$INSTDIR\sensors\WallgatesSensors.exe" "--uninstall" SW_HIDE
    ${EndIf}
  ${EndIf}
!macroend

; MusicFrog Infiltrator Slint installer (Modern UI 2). Peer of infiltrator.nsi
; and infiltrator-bevy.nsi. Display name, installer prefix and registry keys
; come from product-identity.properties; packaging/windows/nsis/build-nsis.sh
; passes VERSION and ICON_PATH and verifies this file.
Unicode true
ManifestDPIAware true

!include "MUI2.nsh"
!include "x64.nsh"
!include "FileFunc.nsh"

; ---------------------------------------------------------------------------
; General Configuration
; ---------------------------------------------------------------------------
!ifndef VERSION
  !error "VERSION must be supplied from the workspace product version"
!endif
!ifndef ARCH
  !define ARCH "x64"
!endif
!ifndef BINARY_PATH
  !define BINARY_PATH "target\x86_64-pc-windows-msvc\release\infiltrator-slint.exe"
!endif
!ifndef OUTFILE
  !define OUTFILE "dist\Infiltrator-Slint-Setup-${ARCH}.exe"
!endif
!ifndef KERNEL_PATH
  !error "KERNEL_PATH must name a verified packaged mihomo executable"
!endif

Name "MusicFrog Infiltrator Slint"
OutFile "${OUTFILE}"
InstallDir "$PROGRAMFILES64\MusicFrog Infiltrator Slint"
InstallDirRegKey HKLM "Software\MusicFrog\InfiltratorSlint" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

; ---------------------------------------------------------------------------
; Interface Settings
; ---------------------------------------------------------------------------
!define MUI_ABORTWARNING
; NSIS resolves relative MUI_ICON paths against the *script* directory, so a
; repo-root-relative literal would break. The workflow passes an absolute
; -DICON_PATH; this default keeps a bare `makensis` invocation usable.
!ifndef ICON_PATH
  !define ICON_PATH "crates\infiltrator-shared\assets\product\icon.ico"
!endif
!define MUI_ICON "${ICON_PATH}"
!define MUI_UNICON "${ICON_PATH}"

!define MUI_HEADERIMAGE
!define MUI_WELCOMEFINISHPAGE_BITMAP_NOSTRETCH

; Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\infiltrator-slint.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch MusicFrog Infiltrator Slint"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"

; ---------------------------------------------------------------------------
; Installer Section
; ---------------------------------------------------------------------------
Section "MusicFrog Infiltrator Slint Core & GUI" SecCore
  SectionIn RO
  ${If} ${RunningX64}
    SetRegView 64
  ${EndIf}

  ; Terminate existing running instance to prevent file lock
  DetailPrint "Checking for running instances..."
  nsExec::Exec 'taskkill /F /IM infiltrator-slint.exe'

  SetOutPath "$INSTDIR"
  File "/oname=infiltrator-slint.exe" "${BINARY_PATH}"
  File "/oname=mihomo.exe" "${KERNEL_PATH}"
  File "/oname=Mihomo-LICENSE.txt" "${KERNEL_LICENSE_PATH}"
  File "/oname=mihomo-assets.json" "${KERNEL_METADATA_PATH}"

  ; Write Uninstaller
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Create Start Menu Shortcuts
  CreateDirectory "$SMPROGRAMS\MusicFrog Infiltrator Slint"
  CreateShortcut "$SMPROGRAMS\MusicFrog Infiltrator Slint\MusicFrog Infiltrator Slint.lnk" "$INSTDIR\infiltrator-slint.exe" "" "$INSTDIR\infiltrator-slint.exe" 0
  CreateShortcut "$SMPROGRAMS\MusicFrog Infiltrator Slint\Uninstall MusicFrog Infiltrator Slint.lnk" "$INSTDIR\uninstall.exe" "" "$INSTDIR\uninstall.exe" 0

  ; Create Desktop Shortcut
  CreateShortcut "$DESKTOP\MusicFrog Infiltrator Slint.lnk" "$INSTDIR\infiltrator-slint.exe" "" "$INSTDIR\infiltrator-slint.exe" 0

  ; Registry: Installation Path
  WriteRegStr HKLM "Software\MusicFrog\InfiltratorSlint" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\MusicFrog\InfiltratorSlint" "Version" "${VERSION}"

  ; Registry: Windows Add/Remove Programs (ARP)
  !define ARP_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\InfiltratorSlint"
  WriteRegStr HKLM "${ARP_KEY}" "DisplayName" "MusicFrog Infiltrator Slint"
  WriteRegStr HKLM "${ARP_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${ARP_KEY}" "Publisher" "MusicFrog Team"
  WriteRegStr HKLM "${ARP_KEY}" "DisplayIcon" "$INSTDIR\infiltrator-slint.exe,0"
  WriteRegStr HKLM "${ARP_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "${ARP_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKLM "${ARP_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${ARP_KEY}" "NoRepair" 1

  ; Calculate and write EstimatedSize (in KB)
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "${ARP_KEY}" "EstimatedSize" "$0"

  ; Register URL Protocol (infiltrator-slint://)
  WriteRegStr HKCR "infiltrator-slint" "" "URL:MusicFrog Infiltrator Slint Protocol"
  WriteRegStr HKCR "infiltrator-slint" "URL Protocol" ""
  WriteRegStr HKCR "infiltrator-slint\DefaultIcon" "" "$INSTDIR\infiltrator-slint.exe,0"
  WriteRegStr HKCR "infiltrator-slint\shell\open\command" "" '"$INSTDIR\infiltrator-slint.exe" "%1"'

  ; Firewall rule registration (Windows Firewall)
  DetailPrint "Configuring Windows Firewall exception..."
  nsExec::Exec 'netsh advfirewall firewall add rule name="MusicFrog Infiltrator Slint" dir=in action=allow program="$INSTDIR\infiltrator-slint.exe" enable=yes profile=any'

SectionEnd

; ---------------------------------------------------------------------------
; Uninstaller Section
; ---------------------------------------------------------------------------
Section "Uninstall"
  ${If} ${RunningX64}
    SetRegView 64
  ${EndIf}

  ; Terminate running instance
  nsExec::Exec 'taskkill /F /IM infiltrator-slint.exe'

  ; Remove Firewall Rule
  nsExec::Exec 'netsh advfirewall firewall delete rule name="MusicFrog Infiltrator Slint"'

  ; Remove Shortcuts
  Delete "$DESKTOP\MusicFrog Infiltrator Slint.lnk"
  Delete "$SMPROGRAMS\MusicFrog Infiltrator Slint\MusicFrog Infiltrator Slint.lnk"
  Delete "$SMPROGRAMS\MusicFrog Infiltrator Slint\Uninstall MusicFrog Infiltrator Slint.lnk"
  RMDir "$SMPROGRAMS\MusicFrog Infiltrator Slint"

  ; Remove Installed Files
  Delete "$INSTDIR\infiltrator-slint.exe"
  Delete "$INSTDIR\mihomo.exe"
  Delete "$INSTDIR\Mihomo-LICENSE.txt"
  Delete "$INSTDIR\mihomo-assets.json"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  ; Remove Registry Keys
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\InfiltratorSlint"
  DeleteRegKey HKLM "Software\MusicFrog\InfiltratorSlint"
  DeleteRegKey HKCR "infiltrator-slint"

SectionEnd

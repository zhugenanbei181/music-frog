; MusicFrog Infiltrator NSIS Installer Script (Modern UI 2)
; Builds high-reliability, 64-bit Windows installer with clean uninstaller & protocol registration.
; Display name, installer prefix and registry keys come from product-identity.properties;
; packaging/windows/nsis/build-nsis.sh passes VERSION and ICON_PATH and verifies this file.

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
  !define BINARY_PATH "target\x86_64-pc-windows-msvc\release\infiltrator-iced.exe"
!endif
!ifndef OUTFILE
  !define OUTFILE "dist\Infiltrator-Setup-${ARCH}.exe"
!endif
!ifndef KERNEL_PATH
  !error "KERNEL_PATH must name a verified packaged mihomo executable"
!endif

Name "MusicFrog Infiltrator"
OutFile "${OUTFILE}"
InstallDir "$PROGRAMFILES64\MusicFrog Infiltrator"
InstallDirRegKey HKLM "Software\MusicFrog\Infiltrator" "InstallDir"
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
!define MUI_FINISHPAGE_RUN "$INSTDIR\infiltrator-iced.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch MusicFrog Infiltrator"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"

; ---------------------------------------------------------------------------
; Installer Section
; ---------------------------------------------------------------------------
Section "MusicFrog Infiltrator Core & GUI" SecCore
  SectionIn RO
  ${If} ${RunningX64}
    SetRegView 64
  ${EndIf}

  ; Terminate existing running instance to prevent file lock
  DetailPrint "Checking for running instances..."
  nsExec::Exec 'taskkill /F /IM infiltrator-iced.exe'

  SetOutPath "$INSTDIR"
  File "/oname=infiltrator-iced.exe" "${BINARY_PATH}"
  File "/oname=mihomo.exe" "${KERNEL_PATH}"
  File "/oname=Mihomo-LICENSE.txt" "${KERNEL_LICENSE_PATH}"
  File "/oname=mihomo-assets.json" "${KERNEL_METADATA_PATH}"

  ; Write Uninstaller
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Create Start Menu Shortcuts
  CreateDirectory "$SMPROGRAMS\MusicFrog Infiltrator"
  CreateShortcut "$SMPROGRAMS\MusicFrog Infiltrator\MusicFrog Infiltrator.lnk" "$INSTDIR\infiltrator-iced.exe" "" "$INSTDIR\infiltrator-iced.exe" 0
  CreateShortcut "$SMPROGRAMS\MusicFrog Infiltrator\Uninstall MusicFrog Infiltrator.lnk" "$INSTDIR\uninstall.exe" "" "$INSTDIR\uninstall.exe" 0

  ; Create Desktop Shortcut
  CreateShortcut "$DESKTOP\MusicFrog Infiltrator.lnk" "$INSTDIR\infiltrator-iced.exe" "" "$INSTDIR\infiltrator-iced.exe" 0

  ; Registry: Installation Path
  WriteRegStr HKLM "Software\MusicFrog\Infiltrator" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\MusicFrog\Infiltrator" "Version" "${VERSION}"

  ; Registry: Windows Add/Remove Programs (ARP)
  !define ARP_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Infiltrator"
  WriteRegStr HKLM "${ARP_KEY}" "DisplayName" "MusicFrog Infiltrator"
  WriteRegStr HKLM "${ARP_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${ARP_KEY}" "Publisher" "MusicFrog Team"
  WriteRegStr HKLM "${ARP_KEY}" "DisplayIcon" "$INSTDIR\infiltrator-iced.exe,0"
  WriteRegStr HKLM "${ARP_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "${ARP_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKLM "${ARP_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${ARP_KEY}" "NoRepair" 1

  ; Calculate and write EstimatedSize (in KB)
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "${ARP_KEY}" "EstimatedSize" "$0"

  ; Register URL Protocol (infiltrator://)
  WriteRegStr HKCR "infiltrator" "" "URL:MusicFrog Infiltrator Protocol"
  WriteRegStr HKCR "infiltrator" "URL Protocol" ""
  WriteRegStr HKCR "infiltrator\DefaultIcon" "" "$INSTDIR\infiltrator-iced.exe,0"
  WriteRegStr HKCR "infiltrator\shell\open\command" "" '"$INSTDIR\infiltrator-iced.exe" "%1"'

  ; Firewall rule registration (Windows Firewall)
  DetailPrint "Configuring Windows Firewall exception..."
  nsExec::Exec 'netsh advfirewall firewall add rule name="MusicFrog Infiltrator" dir=in action=allow program="$INSTDIR\infiltrator-iced.exe" enable=yes profile=any'

SectionEnd

; ---------------------------------------------------------------------------
; Uninstaller Section
; ---------------------------------------------------------------------------
Section "Uninstall"
  ${If} ${RunningX64}
    SetRegView 64
  ${EndIf}

  ; Terminate running instance
  nsExec::Exec 'taskkill /F /IM infiltrator-iced.exe'

  ; Remove Firewall Rule
  nsExec::Exec 'netsh advfirewall firewall delete rule name="MusicFrog Infiltrator"'

  ; Remove Shortcuts
  Delete "$DESKTOP\MusicFrog Infiltrator.lnk"
  Delete "$SMPROGRAMS\MusicFrog Infiltrator\MusicFrog Infiltrator.lnk"
  Delete "$SMPROGRAMS\MusicFrog Infiltrator\Uninstall MusicFrog Infiltrator.lnk"
  RMDir "$SMPROGRAMS\MusicFrog Infiltrator"

  ; Remove Installed Files
  Delete "$INSTDIR\infiltrator-iced.exe"
  Delete "$INSTDIR\mihomo.exe"
  Delete "$INSTDIR\Mihomo-LICENSE.txt"
  Delete "$INSTDIR\mihomo-assets.json"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  ; Remove Registry Keys
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Infiltrator"
  DeleteRegKey HKLM "Software\MusicFrog\Infiltrator"
  DeleteRegKey HKCR "infiltrator"

SectionEnd

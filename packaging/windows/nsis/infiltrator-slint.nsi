; Infiltrator Slint installer. Product identity is registered in
; product-identity.properties and supplied by build-nsis.sh.
Unicode true
ManifestDPIAware true

!include "MUI2.nsh"
!include "x64.nsh"
!include "FileFunc.nsh"

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

Name "Infiltrator Slint"
OutFile "${OUTFILE}"
InstallDir "$PROGRAMFILES64\InfiltratorSlint"
InstallDirRegKey HKLM "Software\MusicFrog\InfiltratorSlint" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

!ifndef ICON_PATH
  !define ICON_PATH "crates\infiltrator-shared\assets\product\icon.ico"
!endif
!define MUI_ICON "${ICON_PATH}"
!define MUI_UNICON "${ICON_PATH}"
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\infiltrator-slint.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch Infiltrator Slint"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"

Section "Infiltrator Slint Core & GUI" SecCore
  SectionIn RO
  ${If} ${RunningX64}
    SetRegView 64
  ${EndIf}
  nsExec::Exec 'taskkill /F /IM infiltrator-slint.exe'
  SetOutPath "$INSTDIR"
  File "/oname=infiltrator-slint.exe" "${BINARY_PATH}"
  File "/oname=mihomo.exe" "${KERNEL_PATH}"
  File "/oname=Mihomo-LICENSE.txt" "${KERNEL_LICENSE_PATH}"
  File "/oname=mihomo-assets.json" "${KERNEL_METADATA_PATH}"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateDirectory "$SMPROGRAMS\Infiltrator Slint"
  CreateShortcut "$SMPROGRAMS\Infiltrator Slint\Infiltrator Slint.lnk" "$INSTDIR\infiltrator-slint.exe"
  CreateShortcut "$DESKTOP\Infiltrator Slint.lnk" "$INSTDIR\infiltrator-slint.exe"
  WriteRegStr HKLM "Software\MusicFrog\InfiltratorSlint" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\MusicFrog\InfiltratorSlint" "Version" "${VERSION}"
  !define ARP_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\InfiltratorSlint"
  WriteRegStr HKLM "${ARP_KEY}" "DisplayName" "Infiltrator Slint"
  WriteRegStr HKLM "${ARP_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${ARP_KEY}" "Publisher" "MusicFrog Team"
  WriteRegStr HKLM "${ARP_KEY}" "DisplayIcon" "$INSTDIR\infiltrator-slint.exe,0"
  WriteRegStr HKLM "${ARP_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKLM "${ARP_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${ARP_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "${ARP_KEY}" "EstimatedSize" "$0"
  WriteRegStr HKCR "infiltrator-slint" "" "URL:Infiltrator Slint Protocol"
  WriteRegStr HKCR "infiltrator-slint" "URL Protocol" ""
  WriteRegStr HKCR "infiltrator-slint\DefaultIcon" "" "$INSTDIR\infiltrator-slint.exe,0"
  WriteRegStr HKCR "infiltrator-slint\shell\open\command" "" '"$INSTDIR\infiltrator-slint.exe" "%1"'
SectionEnd

Section "Uninstall"
  ${If} ${RunningX64}
    SetRegView 64
  ${EndIf}
  nsExec::Exec 'taskkill /F /IM infiltrator-slint.exe'
  Delete "$DESKTOP\Infiltrator Slint.lnk"
  Delete "$SMPROGRAMS\Infiltrator Slint\Infiltrator Slint.lnk"
  RMDir "$SMPROGRAMS\Infiltrator Slint"
  Delete "$INSTDIR\infiltrator-slint.exe"
  Delete "$INSTDIR\mihomo.exe"
  Delete "$INSTDIR\Mihomo-LICENSE.txt"
  Delete "$INSTDIR\mihomo-assets.json"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\InfiltratorSlint"
  DeleteRegKey HKLM "Software\MusicFrog\InfiltratorSlint"
  DeleteRegKey HKCR "infiltrator-slint"
SectionEnd

; Yuumi Windows NSIS Installer Script
; Modern UI 2 with support for /S (silent), /P (passive/updater), /R (relaunch)

Unicode True

!include "MUI2.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !define VERSION "0.1.0"
!endif

!ifndef VI_VERSION
  !define VI_VERSION "${VERSION}.0"
!endif

!ifndef BUILD_DIR
  !define BUILD_DIR "..\..\target\release"
!endif

!define PRODUCT_NAME "Yuumi"
!define PUBLISHER "Yuumi"
!define APP_EXE "yuumi.exe"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Yuumi"
!define REG_KEY "Software\Yuumi"

Name "${PRODUCT_NAME}"
OutFile "..\..\Yuumi_${VERSION}_x64-setup.exe"
InstallDir "$LOCALAPPDATA\Yuumi"
InstallDirRegKey HKCU "${REG_KEY}" ""
RequestExecutionLevel user
SetCompressor /SOLID lzma

VIProductVersion "${VI_VERSION}"
VIAddVersionKey "ProductName" "${PRODUCT_NAME}"
VIAddVersionKey "CompanyName" "${PUBLISHER}"
VIAddVersionKey "LegalCopyright" "Yuumi"
VIAddVersionKey "FileDescription" "${PRODUCT_NAME} Installer"
VIAddVersionKey "FileVersion" "${VI_VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"

!define MUI_ICON "..\..\crates\yuumi-gpui\assets\icons\icon.ico"
!define MUI_UNICON "..\..\crates\yuumi-gpui\assets\icons\icon.ico"
!define MUI_ABORTWARNING

Var PassiveMode
Var Relaunch

; MUI Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "运行 ${PRODUCT_NAME}"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  ; Detect existing install directory from Vue / Tauri / GPUI registry keys if not specified via /D
  ${If} $INSTDIR == "$LOCALAPPDATA\Yuumi"
    ReadRegStr $0 HKCU "${UNINST_KEY}" "InstallLocation"
    ${If} $0 != ""
      StrCpy $INSTDIR $0
    ${Else}
      ReadRegStr $0 HKCU "${REG_KEY}" ""
      ${If} $0 != ""
        StrCpy $INSTDIR $0
      ${Else}
        ReadRegStr $0 HKLM "${UNINST_KEY}" "InstallLocation"
        ${If} $0 != ""
          StrCpy $INSTDIR $0
        ${EndIf}
      ${EndIf}
    ${EndIf}
  ${EndIf}

  ; Parse command-line parameters
  StrCpy $PassiveMode 0
  StrCpy $Relaunch 0

  ${GetParameters} $R0
  ${GetOptions} $R0 "/P" $R1
  ${IfNot} ${Errors}
    StrCpy $PassiveMode 1
    SetSilent silent
  ${EndIf}

  ${GetOptions} $R0 "/UPDATE" $R1
  ${IfNot} ${Errors}
    StrCpy $PassiveMode 1
    SetSilent silent
  ${EndIf}

  ${GetOptions} $R0 "/R" $R1
  ${IfNot} ${Errors}
    StrCpy $Relaunch 1
  ${EndIf}
FunctionEnd

Section "MainSection" SecMain
  SetShellVarContext current

  ; Kill any running instance before overwriting
  nsExec::Exec 'taskkill /F /IM ${APP_EXE}'
  Sleep 500

  SetOutPath "$INSTDIR"
  File "${BUILD_DIR}\${APP_EXE}"

  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Registry keys for uninstaller and app location
  WriteRegStr HKCU "${REG_KEY}" "" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${PRODUCT_NAME}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" '"$INSTDIR\${APP_EXE}",0'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1

  ; Shortcuts
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0
  CreateShortCut "$DESKTOP\${PRODUCT_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0
SectionEnd

Function .onInstSuccess
  ${If} $Relaunch == 1
    Exec '"$INSTDIR\${APP_EXE}"'
  ${EndIf}
FunctionEnd

Section "Uninstall"
  SetShellVarContext current

  nsExec::Exec 'taskkill /F /IM ${APP_EXE}'
  Sleep 500

  Delete "$SMPROGRAMS\${PRODUCT_NAME}.lnk"
  Delete "$DESKTOP\${PRODUCT_NAME}.lnk"

  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\uninstall.exe"

  ; Clean old files if upgraded from Tauri
  Delete "$INSTDIR\WebView2Loader.dll"

  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegKey HKCU "${REG_KEY}"

  RMDir "$INSTDIR"
SectionEnd

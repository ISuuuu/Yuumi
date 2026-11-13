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
Var Elevated

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
  StrCpy $Elevated 0

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

  ; 旧版更新器（Tauri/Vue 客户端）只传 /S：静默安装同样属于更新流程，
  ; 必须由安装包自己拉起程序，否则调用方进程被结束后用户再也看不到应用。
  ${If} ${Silent}
    StrCpy $PassiveMode 1
  ${EndIf}

  ; /ELEVATED 由本脚本在探测到安装目录不可写后以管理员身份重试时带上，避免提权循环
  ${GetOptions} $R0 "/ELEVATED" $R1
  ${IfNot} ${Errors}
    StrCpy $Elevated 1
  ${EndIf}

  ; 修正历史注册表里损坏的安装目录：实测存在丢失盘符冒号的 "D\Program Files\Yuumi"，
  ; 这种相对路径会让 SetOutPath/File 静默写到未知位置或直接失败，而旧流程已先杀掉了程序。
  StrCpy $R2 $INSTDIR 1 1
  ${If} $R2 != ":"
    StrCpy $R3 $INSTDIR 1 0
    ${If} $R2 == "\"
    ${AndIf} $R3 >= "A"
    ${AndIf} $R3 <= "Z"
      StrCpy $R4 $INSTDIR 1024 1
      StrCpy $INSTDIR "$R3:$R4"
    ${Else}
      StrCpy $INSTDIR "$LOCALAPPDATA\Yuumi"
    ${EndIf}
  ${EndIf}

  Call EnsureWritableInstallDir
FunctionEnd

; 安装目录不可写时（典型：装在 D:\Program Files 却以普通权限跑静默更新）
; 先以管理员身份重试一次，仍不可写则退回当前用户目录，避免更新把程序弄丢。
Function EnsureWritableInstallDir
  CreateDirectory "$INSTDIR"
  ClearErrors
  FileOpen $9 "$INSTDIR\.yuumi-write-test" w
  ${IfNot} ${Errors}
    FileClose $9
    Delete "$INSTDIR\.yuumi-write-test"
    Return
  ${EndIf}

  ${If} $Elevated == 1
    StrCpy $INSTDIR "$LOCALAPPDATA\Yuumi"
    Return
  ${EndIf}

  ExecShell "runas" '"$EXEPATH" $R0 /ELEVATED'
  Quit
FunctionEnd

Section "MainSection" SecMain
  SetShellVarContext current

  ; 先把新程序释放成临时文件：旧实现一上来就 taskkill，一旦复制失败
  ; （目录不可写、磁盘满）用户就只剩一个被杀掉、又不会自动重启的程序。
  CreateDirectory "$INSTDIR"
  SetOutPath "$INSTDIR"
  Delete "$INSTDIR\${APP_EXE}.new"
  ClearErrors
  File /oname=${APP_EXE}.new "${BUILD_DIR}\${APP_EXE}"
  ${If} ${Errors}
    Call AbortWithOldApp
  ${EndIf}

  ; 新文件确认就位后才结束运行中的实例
  nsExec::Exec 'taskkill /F /IM ${APP_EXE}'
  Pop $0
  Sleep 800

  Delete "$INSTDIR\${APP_EXE}.old"
  ClearErrors
  Rename "$INSTDIR\${APP_EXE}" "$INSTDIR\${APP_EXE}.old"
  ${If} ${Errors}
    ; 进程可能仍在退出过程中，重试一次再判定占用
    Sleep 700
    ClearErrors
    Rename "$INSTDIR\${APP_EXE}" "$INSTDIR\${APP_EXE}.old"
  ${EndIf}
  ${If} ${Errors}
    ; 旧程序仍被占用（进程未退出）：保留原文件并中止，保证程序还能启动
    Call AbortWithOldApp
  ${EndIf}

  ClearErrors
  Rename "$INSTDIR\${APP_EXE}.new" "$INSTDIR\${APP_EXE}"
  ${If} ${Errors}
    ; 换装失败时回滚，避免目录里既没有可用程序也没有备份
    Rename "$INSTDIR\${APP_EXE}.old" "$INSTDIR\${APP_EXE}"
    Call AbortWithOldApp
  ${EndIf}
  Delete "$INSTDIR\${APP_EXE}.old"

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
  ; 静默/被动更新（/S、/P、/UPDATE）必须由安装包自己拉起程序：
  ; 调用方进程此时已被结束，不拉起用户就会看到「点了更新，重启后程序没了」。
  ${If} $PassiveMode == 1
    Exec '"$INSTDIR\${APP_EXE}"'
  ${ElseIf} $Relaunch == 1
    Exec '"$INSTDIR\${APP_EXE}"'
  ${EndIf}
FunctionEnd

; 安装失败的统一出口：写日志、保留（或回滚出）旧程序并重新拉起，绝不留下一台空目录
Function AbortWithOldApp
  FileOpen $9 "$INSTDIR\yuumi-update-error.log" a
  ${IfNot} ${Errors}
    FileWrite $9 "[${VERSION}] 无法写入 $INSTDIR\${APP_EXE}$\r$\n"
    FileClose $9
  ${EndIf}
  Delete "$INSTDIR\${APP_EXE}.new"
  ${If} $PassiveMode != 1
    MessageBox MB_ICONSTOP "无法写入 $\"$INSTDIR$\"，安装已取消。请换一个可写目录，或以管理员身份运行安装包。"
  ${EndIf}
  ${If} ${FileExists} "$INSTDIR\${APP_EXE}"
    Exec '"$INSTDIR\${APP_EXE}"'
  ${EndIf}
  SetErrorLevel 2
  Quit
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

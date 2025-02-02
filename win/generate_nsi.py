#!/usr/bin/env python3
import os
import semver
import toml

with open(os.path.join(os.path.dirname(__file__), "..", "tango", "Cargo.toml")) as f:
    cargo_toml = toml.load(f)


version = semver.Version.parse(cargo_toml["package"]["version"])

print(
    f"""\
!define NAME "Tango"
!define REGPATH_UNINSTSUBKEY "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\${{NAME}}"

LoadLanguageFile "${{NSISDIR}}\Contrib\Language files\English.nlf"
LoadLanguageFile "${{NSISDIR}}\Contrib\Language files\Japanese.nlf"
LoadLanguageFile "${{NSISDIR}}\Contrib\Language files\TradChinese.nlf"
LoadLanguageFile "${{NSISDIR}}\Contrib\Language files\SimpChinese.nlf"

Name "${{NAME}}"
Icon "icon.ico"
OutFile "installer.exe"

VIProductVersion "{version.major}.{version.minor}.{version.patch}.0"
VIAddVersionKey "ProductName" "${{NAME}}"
VIAddVersionKey "FileVersion" "{version.major}.{version.minor}.{version.patch}.0"
VIAddVersionKey "FileDescription" "Tango Installer"
VIAddVersionKey "LegalCopyright" "© Copyright The Tango Developers"

SetCompressor /solid /final lzma
Unicode true
RequestExecutionLevel user
AutoCloseWindow true
ShowInstDetails nevershow
ShowUninstDetails nevershow
BrandingText " "
ChangeUI all "${{NSISDIR}}\\Contrib\\UIs\\sdbarker_tiny.exe"

InstallDir ""
InstallDirRegKey HKCU "${{REGPATH_UNINSTSUBKEY}}" "UninstallString"

!include LogicLib.nsh
!include WinCore.nsh
!include FileFunc.nsh

Function .onInit
    SetShellVarContext Current

    ${{If}} $INSTDIR == ""
        GetKnownFolderPath $INSTDIR ${{FOLDERID_UserProgramFiles}}
        StrCmp $INSTDIR "" 0 +2
        StrCpy $INSTDIR "$LocalAppData\\Programs"
        StrCpy $INSTDIR "$INSTDIR\\$(^Name)"
    ${{EndIf}}

    ExecWait '"$INSTDIR\\Uninstall Tango.exe" /S'
FunctionEnd

Function un.onInit
    SetShellVarContext Current
FunctionEnd

LangString MessageDeleteConfig ${{LANG_ENGLISH}} "Would you also like to delete configuration settings?"
LangString MessageDeleteConfig ${{LANG_JAPANESE}} "コンフィギュレーション設定も削除しますか？"
LangString MessageDeleteConfig ${{LANG_SIMPCHINESE}} "您是否也想删除配置设置？"
LangString MessageDeleteConfig ${{LANG_TRADCHINESE}} "您是否也想刪除配置設置？"

Function un.onGUIInit
    MessageBox MB_YESNO "$(MessageDeleteConfig)" /SD IDNO IDYES true IDNO false
    true:
        Delete "$APPDATA\\Tango\\config\\config.json"
    false:
FunctionEnd

Function .onInstSuccess
    Exec "$INSTDIR\\tango.exe"
FunctionEnd

Section
    SetDetailsPrint none
    SetOutPath $INSTDIR
    File "libstdc++-6.dll"
    File "libEGL.dll"
    File "libGLESv2.dll"
    File "libgcc_s_seh-1.dll"
    File "libwinpthread-1.dll"
    File "ffmpeg.exe"
    File "tango.exe"
    WriteUninstaller "$INSTDIR\\uninstall.exe"
    WriteRegStr HKCU "${{REGPATH_UNINSTSUBKEY}}" "DisplayName" "${{NAME}}"
    WriteRegStr HKCU "${{REGPATH_UNINSTSUBKEY}}" "DisplayIcon" "$INSTDIR\\tango.exe,0"
    WriteRegStr HKCU "${{REGPATH_UNINSTSUBKEY}}" "Publisher" "The Tango Developers"
    WriteRegStr HKCU "${{REGPATH_UNINSTSUBKEY}}" "InstallLocation" "$INSTDIR"

    IntFmt $0 "0x%08X" "{version.major}"
    WriteRegDWORD HKCU "${{REGPATH_UNINSTSUBKEY}}" "VersionMajor" "$0"

    IntFmt $0 "0x%08X" "{version.minor}"
    WriteRegDWORD HKCU "${{REGPATH_UNINSTSUBKEY}}" "VersionMinor" "$0"

    WriteRegStr HKCU "${{REGPATH_UNINSTSUBKEY}}" "DisplayVersion" "{version}"
    WriteRegStr HKCU "${{REGPATH_UNINSTSUBKEY}}" "UninstallString" '"$INSTDIR\\uninstall.exe"'
    WriteRegStr HKCU "${{REGPATH_UNINSTSUBKEY}}" "QuietUninstallString" '"$INSTDIR\\uninstall.exe" /S'

    ${{GetSize}} "$INSTDIR" "/S=0K" $0 $1 $2
    IntFmt $0 "0x%08X" $0
    WriteRegDWORD HKCU "${{REGPATH_UNINSTSUBKEY}}" "EstimatedSize" "$0"

    WriteRegDWORD HKCU "${{REGPATH_UNINSTSUBKEY}}" "NoModify" 1
    WriteRegDWORD HKCU "${{REGPATH_UNINSTSUBKEY}}" "NoRepair" 1
    CreateShortcut "$SMPROGRAMS\\Tango.lnk" "$INSTDIR\\tango.exe"
    CreateShortcut "$DESKTOP\\Tango.lnk" "$INSTDIR\\tango.exe"
SectionEnd

Section "uninstall"
    SetDetailsPrint none
    Delete "$DESKTOP\\Tango.lnk"
    Delete "$SMPROGRAMS\\Tango.lnk"
    Delete "$INSTDIR\\libstdc++-6.dll"
    Delete "$INSTDIR\\libEGL.dll"
    Delete "$INSTDIR\\libGLESv2.dll"
    Delete "$INSTDIR\\libgcc_s_seh-1.dll"
    Delete "$INSTDIR\\libwinpthread-1.dll"
    Delete "$INSTDIR\\ffmpeg.exe"
    Delete "$INSTDIR\\tango.exe"
    Delete "$INSTDIR\\uninstall.exe"
    RMDir $INSTDIR
    DeleteRegKey HKCU "${{REGPATH_UNINSTSUBKEY}}"
SectionEnd
"""
)

; Includes
!include "MUI2.nsh"
!include "logiclib.nsh"

!define PRODUCT "Flux Screensaver"
!define ORG "sandydoo"
!define SLUG "${PRODUCT} v${VERSION}"
!define SCRFILE "Flux.scr"
!define REGKEY "Software\${ORG}\${PRODUCT}"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT}"
!define WEBSITE "https://flux.sandydoo.me/"

Unicode True
Name "${PRODUCT}"
OutFile "${OUTDIR}/flux-screensaver-setup-v${version}.exe"
InstallDir "$PROGRAMFILES64\${ORG}\${PRODUCT}"
InstallDirRegKey HKCU "${REGKEY}" "Install_Dir"
SetCompressor /SOLID lzma
RequestExecutionLevel admin

; Icons
!define MUI_ICON "flux-screensaver.ico"
!define MUI_UNICON "flux-screensaver.ico"
; !define MUI_HEADERIMAGE
; !define MUI_HEADERIMAGE_BITMAP "header.bmp"
; !define MUI_UNHEADERIMAGE_BITMAP "header.bmp"
; !define MUI_WELCOMEFINISHPAGE_BITMAP ${BANNER}
; !define MUI_UNWELCOMEFINISHPAGE_BITMAP ${BANNER}

; Disable hover descriptions for components
!define MUI_COMPONENTSPAGE_NODESC
; Ask for confirmation before exiting
!define MUI_ABORTWARNING

; Configure the welcome page
!define MUI_WELCOMEPAGE_TITLE "${SLUG} Setup"

; Configure the finish page
!define MUI_FINISHPAGE_NOAUTOCLOSE
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Configure Flux Screensaver"
!define MUI_FINISHPAGE_RUN_FUNCTION OpenScreensaverPanel
!define MUI_UNFINISHPAGE_NOAUTOCLOSE

; Launch the Windows screensaver configuration panel
; TODO: the screensaver list will be missing a bunch of default screensavers when first launched. Why?
Function OpenScreensaverPanel
  Exec 'Rundll32.exe shell32.dll,Control_RunDLL desk.cpl,,@screensaver'
FunctionEnd

; Installer pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

; Uninstaller pages
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

; Set UI language
!insertmacro MUI_LANGUAGE "English"

Section "${PRODUCT} (Required)" Flux
  SectionIn RO

  ; All the files we want
  SetOutPath "$INSTDIR"
  File "${DSTDIR}/${SCRFILE}"

  ; Write the installation path into the registry
  WriteRegStr HKCU "${REGKEY}" "Install_Dir" "$INSTDIR"
  ; Write the uninstall keys for Windows
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  ; Set the screensaver.
  ; This in non-optional, unless you install the screensaver to $SYSDIR.
  WriteRegStr HKCU "Control Panel\Desktop" "Scrnsave.exe" "$INSTDIR\${SCRFILE}"
  WriteRegStr HKCU "Control Panel\Desktop" "ScreenSaveActive" "1"

  ; Notify the system of the change.
  ; This updates the screensaver configuration panel.
  System::Call 'user32.dll::SystemParametersInfo(17, 1, 0, 2)'
SectionEnd

;--------------------------------
; Remove empty parent directories

Function un.RMDirUP
  !define RMDirUP '!insertmacro RMDirUPCall'

  !macro RMDirUPCall _PATH
        push '${_PATH}'
        Call un.RMDirUP
  !macroend

  ; $0 - current folder
  ClearErrors

  Exch $0
  ;DetailPrint "ASDF - $0\.."
  RMDir "$0\.."

  IfErrors Skip
  ${RMDirUP} "$0\.."
  Skip:

  Pop $0
FunctionEnd

Section "Uninstall"
  ; Remove registry keys
  DeleteRegKey HKCU "${UNINSTKEY}"
  DeleteRegKey HKCU "${REGKEY}"

  ; Remove files
  RMDir /r "$INSTDIR"
  ${RMDirUP} "$INSTDIR"
SectionEnd

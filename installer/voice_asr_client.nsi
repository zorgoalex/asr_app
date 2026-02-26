!define APP_NAME "Voice ASR Client"
!define APP_EXE "voice_asr_client.exe"
!define APP_VERSION "0.1.0"
!define ROOT_DIR ".."
!define OUT_DIR "${ROOT_DIR}\installer\out"

OutFile "${OUT_DIR}\VoiceASRClient-${APP_VERSION}-setup.exe"
InstallDir "$LOCALAPPDATA\VoiceASRClient"
RequestExecutionLevel user

Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "Install"
  SetOutPath "$INSTDIR"
  File "${ROOT_DIR}\target\release\${APP_EXE}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "$SMPROGRAMS\${APP_NAME}.lnk"
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd

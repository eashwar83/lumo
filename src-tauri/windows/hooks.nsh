; Uncheck the "Create desktop shortcut" checkbox on the finish page by default.
; Tauri's NSIS template reuses MUI's "show readme" checkbox for the desktop
; shortcut; MUI leaves it checked unless this is defined. This file is included
; near the top of the template, before MUI_PAGE_FINISH, so the define applies.
!define MUI_FINISHPAGE_SHOWREADME_NOTCHECKED

!macro NSIS_HOOK_POSTINSTALL
  ; Make mpv runtime DLLs available next to Soia.exe for the Windows loader.
  CopyFiles /SILENT "$INSTDIR\resources\*.dll" "$INSTDIR\"
!macroend

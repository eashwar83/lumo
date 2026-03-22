!macro NSIS_HOOK_POSTINSTALL
  ; Make mpv runtime DLLs available next to Soia.exe for the Windows loader.
  CopyFiles /SILENT "$INSTDIR\resources\*.dll" "$INSTDIR\"
!macroend

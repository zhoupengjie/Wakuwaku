; Before the files go: take back what Wakuwaku added outside itself
; (its Claude Code hooks, start at login).
!macro customUnInstall
  ExecWait '"$INSTDIR\${APP_EXECUTABLE_FILENAME}" --wakuwaku-cleanup'
!macroend

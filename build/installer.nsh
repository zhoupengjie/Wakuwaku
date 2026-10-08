; Before the files go: take back what Claude Pets added outside itself
; (its Claude Code hooks, start at login).
!macro customUnInstall
  ExecWait '"$INSTDIR\${APP_EXECUTABLE_FILENAME}" --claude-pets-cleanup'
!macroend

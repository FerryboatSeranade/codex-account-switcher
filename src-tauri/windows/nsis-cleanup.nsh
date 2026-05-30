!macro NSIS_HOOK_POSTINSTALL
  Delete "$SMPROGRAMS\Codex Account Switcher.lnk"
  Delete "$SMPROGRAMS\Codex Account Switcher\Codex Account Switcher.lnk"
  RMDir "$SMPROGRAMS\Codex Account Switcher"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$SMPROGRAMS\Codex Account Switcher.lnk"
  Delete "$SMPROGRAMS\Codex Account Switcher\Codex Account Switcher.lnk"
  RMDir "$SMPROGRAMS\Codex Account Switcher"
!macroend

; Valet installer hooks. Tauri includes this file in its NSIS template
; (tauri.conf.json > bundle > windows > nsis > installerHooks).
; Checked against the template of tauri-bundler 2.6.1, the bundler in
; @tauri-apps/cli 2.8.4 (apps/desktop/pnpm-lock.yaml):
; crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi.
; ASCII only: makensis reads this file as UTF-8 and Tauri does not add a BOM.
;
; Desktop shortcut
; The template creates $DESKTOP\<productName>.lnk on every silent (/S) and
; passive (/P) install, unless /UPDATE or /NS is on the command line (/NS also
; skips the Start Menu shortcut). Interactive installs offer it on the finish
; page. Valet wants:
; - silent install (/S without /UPDATE; winget and scripted installs): no new
;   desktop shortcut; one that was already there stays;
; - updater (tauri-plugin-updater 2.14 always adds /UPDATE, after /P or /S):
;   the template neither creates nor removes the shortcut; this hook does
;   nothing;
; - interactive and passive installs: unchanged.
; NSIS_HOOK_PREINSTALL runs before the template creates shortcuts and
; NSIS_HOOK_POSTINSTALL after (documented order), so the post hook removes
; only a shortcut that the template created during this run.

Var ValetDesktopLnkExisted
Var ValetUpdateArg

!macro NSIS_HOOK_PREINSTALL
  StrCpy $ValetDesktopLnkExisted 0
  ${If} ${FileExists} "$DESKTOP\${PRODUCTNAME}.lnk"
    StrCpy $ValetDesktopLnkExisted 1
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; GetOptions sets the error flag when /UPDATE is absent.
  ClearErrors
  ${GetOptions} $CMDLINE "/UPDATE" $ValetUpdateArg
  ${If} ${Errors}
  ${AndIf} ${Silent}
  ${AndIf} $ValetDesktopLnkExisted = 0
    Delete "$DESKTOP\${PRODUCTNAME}.lnk"
  ${EndIf}
!macroend

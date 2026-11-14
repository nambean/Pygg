; Pygg's touches on Tauri's NSIS installer (tauri.conf.json → bundle.windows.nsis.installerHooks). Tauri includes this
; file before its pages, so the page texts below apply to its Welcome and Finish pages.
;
; While files are copied, the progress page plays a short animation of what Pygg does (scripts/installer-art.py draws
; it): Windows' own Animate control (SysAnimate32) plays an RLE8 AVI by itself, so no NSIS plug-in is needed. The
; control is created hidden when the window opens (on the window's thread), then moved onto the progress page when
; installing starts; it goes away with that page.

!define MUI_WELCOMEPAGE_TITLE "Welcome to Pygg"
!define MUI_WELCOMEPAGE_TEXT "One Flow. Many Agents.$\r$\n$\r$\nPygg is a local gateway for your coding agents: Claude Code, Codex, OpenCode and any OpenAI- or Anthropic-format client. It sends each request to the providers you connect, moves on to the next one when a provider is out of quota or failing, and keeps every request priced in a ledger on this computer.$\r$\n$\r$\nClick Next to continue."
!define MUI_FINISHPAGE_TITLE "Pygg is ready"
!define MUI_FINISHPAGE_TEXT "While Pygg runs, its gateway answers on http://127.0.0.1:8741.$\r$\n$\r$\nNext: connect a provider in Pygg, then set up your coding tool from CLI Tools in one click."

!define MUI_CUSTOMFUNCTION_GUIINIT PyggGuiInit

Var PyggAnim

Function PyggGuiInit
  InitPluginsDir
  ; The animation drawn for the window's scale: 100, 150 or 200 %.
  System::Call "user32::GetDpiForWindow(p $HWNDPARENT) i .r0"
  ${If} $0 >= 168
    File "/oname=$PLUGINSDIR\pygg-install.avi" "${__FILEDIR__}\install-200.avi"
  ${ElseIf} $0 >= 120
    File "/oname=$PLUGINSDIR\pygg-install.avi" "${__FILEDIR__}\install-150.avi"
  ${Else}
    File "/oname=$PLUGINSDIR\pygg-install.avi" "${__FILEDIR__}\install-100.avi"
  ${EndIf}
  ; ICC_ANIMATE_CLASS
  System::Call "*(i 8, i 0x80) p .r1"
  System::Call "comctl32::InitCommonControlsEx(p r1)"
  System::Free $1
  ; WS_CHILD | ACS_CENTER, hidden until installing starts.
  System::Call 'user32::CreateWindowExW(i 0, w "SysAnimate32", w "", i 0x40000001, i 0, i 0, i 1, i 1, p $HWNDPARENT, p 1300, p 0, p 0) p .r0'
  StrCpy $PyggAnim $0
  ${If} $PyggAnim != 0
    SendMessage $PyggAnim 0x467 0 "STR:$PLUGINSDIR\pygg-install.avi" ; ACM_OPENW
  ${Else}
    StrCpy $PyggAnim ""
  ${EndIf}
FunctionEnd

!macro NSIS_HOOK_PREINSTALL
  ${If} $PyggAnim != ""
    Push $0
    Push $1
    Push $2
    Push $3
    Push $4
    Push $5
    Push $6
    Push $7
    Push $8
    Push $9
    FindWindow $0 "#32770" "" $HWNDPARENT ; the progress page
    ; The details list and its button give way to the animation.
    GetDlgItem $1 $0 1027
    ShowWindow $1 0
    GetDlgItem $1 $0 1016
    ShowWindow $1 0
    ; Under the progress bar, the page's full width, down to its bottom.
    GetDlgItem $1 $0 1004
    System::Call "*(i 0, i 0, i 0, i 0) p .r2"
    System::Call "user32::GetWindowRect(p r1, p r2)"
    System::Call "user32::MapWindowPoints(p 0, p r0, p r2, i 2)"
    System::Call "*$2(i .r3, i .r4, i .r5, i .r6)"
    System::Call "user32::GetClientRect(p r0, p r2)"
    System::Call "*$2(i .r3, i .r4, i .r7, i .r8)"
    System::Free $2
    IntOp $4 $6 + 6
    IntOp $9 $8 - $4
    System::Call "user32::SetParent(p $PyggAnim, p r0)"
    ; HWND_TOP; SWP_SHOWWINDOW | SWP_NOACTIVATE
    System::Call "user32::SetWindowPos(p $PyggAnim, p 0, i 0, i r4, i r7, i r9, i 0x50)"
    SendMessage $PyggAnim 0x465 -1 0xFFFF0000 ; ACM_PLAY, every frame, forever
    Pop $9
    Pop $8
    Pop $7
    Pop $6
    Pop $5
    Pop $4
    Pop $3
    Pop $2
    Pop $1
    Pop $0
  ${EndIf}
!macroend

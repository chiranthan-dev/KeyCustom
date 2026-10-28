; ---------------------------------------------------------------------------
; KeyForge engine runtime (AutoHotkey v2). KeyForge appends the generated
; configuration, hotkeys and actions after this file. Do not edit generated
; scripts: KeyForge rewrites them every time a configuration is applied.
;
; Protocol: the supervisor writes one command per line to stdin
;   ping | detect | cancel-detect | exit | profile <id> | test <mapping id>
; and reads one JSON event per line from stdout. If stdin breaks (the
; supervisor is gone) the engine exits, so the hook never outlives KeyForge.
; ---------------------------------------------------------------------------
#Requires AutoHotkey v2.0
#SingleInstance Off
#NoTrayIcon
#UseHook true
#Warn All, Off
#MaxThreadsPerHotkey 1
Persistent true
KeyHistory 0          ; privacy: keep no key history in memory
ListLines false
SendMode "Input"
SetKeyDelay -1, -1
A_MaxHotkeysPerInterval := 500
ProcessSetPriority "AboveNormal"
InstallKeybdHook true

global KF_hIn := DllCall("GetStdHandle", "Int", -10, "Ptr")
global KF_InBuf := ""
global KF_Detector := 0
global KF_SoloMod := 0
global KF_WasSuspended := false
global KF_Profile := ""
global KF_Names := Map()      ; profile id -> display name
global KF_AutoApps := Map()   ; process name -> profile id
global KF_Disabled := Map()   ; profile id -> "|app.exe|other.exe|"
global KF_Tests := Map()      ; mapping id -> action function
global KF_Down := Map()       ; trigger key held down -> remap target ("" if none)
global KF_Pending := Map()    ; mapping id -> pending single-tap timer

OnError(KF_OnError)
OnExit(KF_OnExit)

KF_Start() {
    SetTimer(KF_PollStdin, 50)
    KF_Emit('{"event":"ready","ahk":' KF_Json(A_AhkVersion) ',"admin":' KF_Bool(A_IsAdmin)
        . ',"pid":' ProcessExist() ',"profile":' KF_Json(KF_Profile) '}')
}

KF_Emit(line) {
    try FileAppend(line "`n", "*", "UTF-8-RAW")
}

KF_Json(s) {
    s := StrReplace(String(s), "\", "\\")
    s := StrReplace(s, '"', '\"')
    s := StrReplace(s, "`n", "\n")
    s := StrReplace(s, "`r", "\r")
    s := StrReplace(s, "`t", "\t")
    return '"' s '"'
}

KF_Bool(v) {
    return v ? "true" : "false"
}

KF_ErrText(e) {
    return (e is Error) ? e.Message : String(e)
}

KF_PollStdin() {
    global KF_InBuf
    avail := 0
    if !DllCall("PeekNamedPipe", "Ptr", KF_hIn, "Ptr", 0, "UInt", 0, "Ptr", 0, "UInt*", &avail, "Ptr", 0)
        KF_Shutdown("supervisor-lost", 2)
    if (avail = 0)
        return
    buf := Buffer(avail)
    got := 0
    if !DllCall("ReadFile", "Ptr", KF_hIn, "Ptr", buf, "UInt", avail, "UInt*", &got, "Ptr", 0)
        KF_Shutdown("supervisor-lost", 2)
    KF_InBuf .= StrGet(buf, got, "UTF-8")
    while (p := InStr(KF_InBuf, "`n")) {
        line := Trim(SubStr(KF_InBuf, 1, p - 1), " `r`t")
        KF_InBuf := SubStr(KF_InBuf, p + 1)
        KF_Command(line)
    }
}

KF_Command(cmd) {
    switch cmd {
        case "":
            return
        case "ping":
            KF_Emit('{"event":"pong"}')
        case "detect":
            KF_StartDetect()
        case "cancel-detect":
            KF_StopDetect("cancelled")
        case "exit":
            KF_Shutdown("requested", 0)
        default:
            if (SubStr(cmd, 1, 8) = "profile ")
                KF_SetProfile(SubStr(cmd, 9), false)
            else if (SubStr(cmd, 1, 5) = "test ")
                KF_Test(SubStr(cmd, 6))
            else
                KF_Emit('{"event":"error","message":' KF_Json("unknown command: " cmd) '}')
    }
}

; --- Profiles and context ---------------------------------------------------

KF_SetProfile(id, announce := true) {
    global KF_Profile
    if !KF_Names.Has(id)
        return
    KF_Profile := id
    KF_Emit('{"event":"profile","id":' KF_Json(id) '}')
    if announce {
        ToolTip("KeyForge: " KF_Names[id])
        SetTimer(() => ToolTip(), -1200)
    }
}

KF_FgExe() {
    try return StrLower(WinGetProcessName("A"))
    return ""
}

; True when a mapping of profile `p` applies right now. `app` limits it to
; one program; `excluded` lists programs ("|a.exe|b.exe|") whose own mapping
; for the same trigger takes precedence.
KF_On(p, app, excluded) {
    exe := KF_FgExe()
    effective := (exe != "" && KF_AutoApps.Has(exe)) ? KF_AutoApps[exe] : KF_Profile
    if (effective != p)
        return false
    if (app != "")
        return exe = app
    if (exe != "" && excluded != "" && InStr(excluded, "|" exe "|"))
        return false
    return !(exe != "" && KF_Disabled.Has(p) && InStr(KF_Disabled[p], "|" exe "|"))
}

; --- Running mappings -------------------------------------------------------

KF_Fired(id) {
    KF_Emit('{"event":"fired","id":' KF_Json(id) '}')
}

KF_Do(fn) {
    try fn()
    catch as e
        KF_Emit('{"event":"error","message":' KF_Json("Action failed: " KF_ErrText(e)) '}')
}

; Every mapped key has a key-up hotkey that is active while the key is down
; (see the generated script), so releases are seen even if the profile or
; foreground app changes while the key is held, and for both physical and
; injected input.
KF_IsDown(key) {
    return KF_Down.Has(key)
}

; Plain remap: the target goes down with the trigger and up with it, with
; auto-repeat.
KF_RemapDown(id, key, target) {
    if !KF_Down.Has(key)
        KF_Fired(id)
    KF_Down[key] := target
    SetKeyDelay -1
    Send "{Blind}{" target " DownR}"
}

KF_Release(key) {
    if !KF_Down.Has(key)
        return
    target := KF_Down.Delete(key)
    if (target != "") {
        SetKeyDelay -1
        Send "{Blind}{" target " up}"
    }
}

; Wait for the trigger key to be released; false if `limitMs` passed first.
KF_WaitUp(key, limitMs := 30000) {
    start := A_TickCount
    while KF_Down.Has(key) {
        if (A_TickCount - start >= limitMs)
            return false
        Sleep 10
    }
    return true
}

; Press, tap/hold and double-tap. Fires once per physical press: while this
; waits for the release, auto-repeat presses of the same hotkey are dropped
; (#MaxThreadsPerHotkey 1).
KF_Run(id, key, holdMs, dblMs, tap, hold, dbl) {
    KF_Down[key] := ""
    KF_Fired(id)
    if hold {
        if !KF_WaitUp(key, holdMs) {
            KF_Do(hold)
            KF_WaitUp(key)
            return
        }
    }
    if dbl {
        if KF_Pending.Has(id) {
            SetTimer(KF_Pending.Delete(id), 0)
            KF_Do(dbl)
        } else {
            fn := KF_FirePending.Bind(id, tap)
            KF_Pending[id] := fn
            SetTimer(fn, -dblMs)
        }
    } else
        KF_Do(tap)
    if !hold
        KF_WaitUp(key)
}

KF_FirePending(id, tap) {
    if KF_Pending.Has(id) {
        KF_Pending.Delete(id)
        KF_Do(tap)
    }
}

KF_Test(id) {
    if !KF_Tests.Has(id) {
        KF_Emit('{"event":"error","message":' KF_Json("Mapping " id " is not active in the engine.") '}')
        return
    }
    KF_Fired(id)
    KF_Do(KF_Tests[id])
}

KF_Launch(path, args := "") {
    Run(args = "" ? '"' path '"' : '"' path '" ' args)
}

KF_Open(target) {
    Run(target)
}

KF_Script(path, kind, confirm, label) {
    ; 0x40024 = Yes/No buttons, question icon, always on top.
    if confirm && MsgBox("Run this script?`n`n" label, "KeyForge", 0x40024) != "Yes"
        return
    if (kind = "ps1")
        Run('powershell.exe -NoProfile -ExecutionPolicy Bypass -File "' path '"')
    else
        Run('"' path '"')
}

; --- Key detection ----------------------------------------------------------
; Captures exactly one key (or a modifier pressed and released on its own),
; reports it and stops. Times out after 15 seconds. Mappings are suspended
; while detecting so the physical key is reported, not its mapping.

KF_StartDetect() {
    global KF_Detector, KF_WasSuspended, KF_SoloMod
    if IsObject(KF_Detector)
        KF_StopDetect("")
    KF_WasSuspended := A_IsSuspended
    Suspend true
    KF_SoloMod := 0
    ih := InputHook("L0 T15")
    ih.KeyOpt("{All}", "NS")
    ih.KeyOpt("{LCtrl}{RCtrl}{LShift}{RShift}{LAlt}{RAlt}{LWin}{RWin}", "-S")
    ih.OnKeyDown := KF_DetectDown
    ih.OnKeyUp := KF_DetectUp
    ih.OnEnd := KF_DetectEnd
    KF_Detector := ih
    ih.Start()
    KF_Emit('{"event":"detect-started"}')
}

KF_IsModifier(vk) {
    return vk = 0x10 || vk = 0x11 || vk = 0x12 || (vk >= 0xA0 && vk <= 0xA5) || vk = 0x5B || vk = 0x5C
}

KF_DetectDown(ih, vk, sc) {
    global KF_SoloMod
    if KF_IsModifier(vk) {
        KF_SoloMod := [vk, sc]
        return
    }
    KF_SoloMod := 0
    if (vk = 0x08 && GetKeyState("Ctrl") && GetKeyState("Alt") && GetKeyState("Shift")) {
        KF_Shutdown("emergency", 3)
        return
    }
    KF_Report(vk, sc, true)
}

KF_DetectUp(ih, vk, sc) {
    if IsObject(KF_SoloMod) && KF_SoloMod[1] = vk
        KF_Report(vk, sc, false)
}

KF_Report(vk, sc, withMods) {
    name := GetKeyName(Format("vk{:x}sc{:x}", vk, sc))
    KF_Emit('{"event":"key","name":' KF_Json(name) ',"vk":' vk ',"sc":' sc
        . ',"ctrl":' KF_Bool(withMods && GetKeyState("Ctrl"))
        . ',"shift":' KF_Bool(withMods && GetKeyState("Shift"))
        . ',"alt":' KF_Bool(withMods && GetKeyState("Alt"))
        . ',"win":' KF_Bool(withMods && (GetKeyState("LWin") || GetKeyState("RWin"))) '}')
    KF_StopDetect("")
}

KF_DetectEnd(ih) {
    if (IsObject(KF_Detector) && ObjPtr(KF_Detector) = ObjPtr(ih) && ih.EndReason = "Timeout")
        KF_StopDetect("timeout")
}

KF_StopDetect(reason) {
    global KF_Detector
    if !IsObject(KF_Detector)
        return
    ih := KF_Detector
    KF_Detector := 0
    ih.Stop()
    Suspend KF_WasSuspended
    KF_Emit('{"event":"detect-ended","reason":' KF_Json(reason = "" ? "done" : reason) '}')
}

; --- Shutdown and recovery --------------------------------------------------

KF_ReleaseModifiers() {
    ; Release modifiers that are logically down but not physically held, so
    ; nothing stays stuck after the engine exits.
    for key in ["LCtrl", "RCtrl", "LShift", "RShift", "LAlt", "RAlt", "LWin", "RWin"] {
        if GetKeyState(key) && !GetKeyState(key, "P")
            Send("{Blind}{" key " up}")
    }
}

KF_Shutdown(reason, code) {
    static done := false
    if done
        return
    done := true
    KF_Emit('{"event":"exit","reason":' KF_Json(reason) '}')
    ExitApp(code)
}

KF_OnExit(reason, code) {
    for key, target in KF_Down
        if (target != "")
            try Send("{Blind}{" target " up}")
    KF_ReleaseModifiers()
}

KF_OnError(err, mode) {
    KF_Emit('{"event":"error","message":' KF_Json(KF_ErrText(err) ((err is Error) ? " (line " err.Line ")" : "")) '}')
    return 1
}

; --- Emergency disable ------------------------------------------------------
; Defined before every user hotkey and exempt from Suspend. Validation rejects
; user mappings that could shadow it.
#SuspendExempt true
^!+Backspace::KF_Shutdown("emergency", 3)
#SuspendExempt false

; --- Generated configuration ------------------------------------------------

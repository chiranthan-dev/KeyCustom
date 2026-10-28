; ---------------------------------------------------------------------------
; KeyForge engine runtime (AutoHotkey v2). Generated scripts start with this
; file and append the user's hotkeys after it. Do not edit generated scripts:
; KeyForge rewrites them every time a configuration is applied.
;
; Protocol: the supervisor writes one command per line to stdin
; (ping | detect | cancel-detect | exit) and reads one JSON event per line
; from stdout. If stdin breaks (the supervisor is gone) the engine exits, so
; the keyboard hook never outlives KeyForge.
; ---------------------------------------------------------------------------
#Requires AutoHotkey v2.0
#SingleInstance Off
#NoTrayIcon
#UseHook true
#Warn All, Off
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

OnError(KF_OnError)
OnExit(KF_OnExit)
SetTimer(KF_PollStdin, 50)
KF_Emit('{"event":"ready","ahk":' KF_Json(A_AhkVersion) ',"admin":' KF_Bool(A_IsAdmin) ',"pid":' ProcessExist() '}')

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
            KF_Emit('{"event":"error","message":' KF_Json("unknown command: " cmd) '}')
    }
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
    global KF_SoloMod
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
    KF_ReleaseModifiers()
}

KF_OnError(err, mode) {
    msg := (err is Error) ? err.Message " (line " err.Line ")" : String(err)
    KF_Emit('{"event":"error","message":' KF_Json(msg) '}')
    return 1
}

; --- Emergency disable ------------------------------------------------------
; Defined before every user hotkey and exempt from Suspend. Validation rejects
; user mappings that could shadow it.
#SuspendExempt true
^!+Backspace::KF_Shutdown("emergency", 3)
#SuspendExempt false

; --- User mappings ------------------------------------------------------------

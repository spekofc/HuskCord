# Task Manager grouping

Goal: in Task Manager, Huskcord shows as **one app** — a single named entry
with the wolf icon, the WebView2 children expandable underneath — instead of
`huskcord.exe` plus seven anonymous `msedgewebview2.exe` rows.

## What Windows actually keys on

Two different mechanisms, often conflated:

| Surface | Keyed on | Result |
|---|---|---|
| Taskbar buttons | `AppUserModelID` of the window | two Huskcord windows → one button |
| Task Manager group | `HKCU\Software\Classes\AppUserModelId\<ID>` (name + icon) | the group has a label instead of a path |
| Task Manager **subtree** | Windows **job object** membership | children nest and expand |

## Measured on Windows 11 26100

Probe: `.github/workflows/probe-aumid.yml`, reading back with
`kernel32!GetApplicationUserModelId` on a clean GitHub runner **and** on the dev
machine. Identical output:

```
shell (2848):   hr=0x00003D57 id=''
browser (1180): hr=0x00003D57 id=''
```

`0x00003D57` is `APPMODEL_ERROR_NO_APPLICATION`: the process has no bound
application identity.

So on this build:

- `SetCurrentProcessExplicitAppUserModelID` returns `S_OK` but does **not** bind
  an ID that the read-back API sees. The call is still the documented way to get
  taskbar grouping, so it stays.
- `--app-user-model-id` **does** reach WebView2's browser process command line
  (string-verified), but the read-back is the same. Setting it is harmless and
  may matter on builds where the binding works.

## What was implemented

1. `SetCurrentProcessExplicitAppUserModelID("spekofc.Huskcord.Desktop")` before
   any window exists — `src/appid.rs`.
2. `--app-user-model-id` appended to WebView2's switches — `src/appid.rs`.
3. `HKCU\Software\Classes\AppUserModelId\spekofc.Huskcord.Desktop` with
   `DisplayName=Huskcord` and `IconUri=<exe>`, written once at startup — this is
   the part that gives the Task Manager entry a name and the wolf icon.

## What is not implemented, and why

**Collapsing the WebView2 children into one expandable row.**

Task Manager nests processes that share a **job object**. WebView2 creates its
own job for the browser process; the renderer, GPU and utilities live in *that*
job, not ours. Sharing our AppUserModelID does not merge jobs.

Merging them would require either:

- running WebView2 with `--single-process` (crashes on any renderer fault and
  takes the shell with it), or
- calling `AssignProcessToJobObject` on WebView2's browser process to move it
  into a job we own, which breaks its crash isolation and is not a supported
  configuration.

Both trade stability for cosmetics. Not done.

## If you want it anyway

The honest path is to read Task Manager's own grouping: it shows a group header
when processes share `AppUserModelId` **and** an image name. Forcing every child
to report the same image name is not possible — they are genuinely
`msedgewebview2.exe` processes managed by the runtime, not by us.

What you *can* get, today, with the registry key in place:

```
Huskcord                       <- one named group with the wolf icon
  huskcord.exe
  msedgewebview2.exe  (×7)     <- still separate rows inside the group
```

versus before:

```
huskcord.exe
  msedgewebview2.exe (browser)
  msedgewebview2.exe (gpu)
  ...                          <- no common label anywhere
```

<#
.SYNOPSIS
    Guided screenshot capture for the Lumo user guide.

.DESCRIPTION
    Walks through every row in docs/SHOTLIST.md, one shot at a time. For each
    shot it prints the instructions, gives you a short countdown to arrange the
    Lumo window (open a menu, load a frame, etc.), then captures the Lumo
    window — cropped tightly to it — and saves the PNG into docs/images/ using
    the exact filename from the shot list.

    Because it captures Lumo by its window rectangle (not "the foreground
    window at the moment you press a key"), open menus and popovers stay open
    while it shoots — you never have to click away from Lumo to trigger it.

.PARAMETER DryRun
    Parse the shot list and locate the Lumo window, but capture nothing.
    Use this to check everything is wired up before a real run.

.PARAMETER Redo
    Re-capture shots even if the PNG already exists (default skips existing).

.PARAMETER Only
    Capture only the shots whose number is in this list, e.g. -Only 10,11,17.

.PARAMETER Countdown
    Seconds to wait before each capture (default 5).

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\capture_guide_shots.ps1

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\capture_guide_shots.ps1 -Only 32,33
#>

[CmdletBinding()]
param(
    [switch]$DryRun,
    [switch]$Redo,
    [int[]]$Only,
    [int]$Countdown = 5
)

$ErrorActionPreference = "Stop"

# --- paths ------------------------------------------------------------------
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot  = Split-Path -Parent $scriptDir
$shotList  = Join-Path $repoRoot "docs\SHOTLIST.md"
$imagesDir = Join-Path $repoRoot "docs\images"

if (-not (Test-Path $shotList)) { throw "Shot list not found: $shotList" }
if (-not (Test-Path $imagesDir)) { New-Item -ItemType Directory -Path $imagesDir | Out-Null }

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

# --- native helpers (window rect, DPI awareness) ----------------------------
if (-not ([System.Management.Automation.PSTypeName]'LumoCap.Win').Type) {
    Add-Type @"
using System;
using System.Runtime.InteropServices;
namespace LumoCap {
    public struct RECT { public int Left, Top, Right, Bottom; }
    public static class Win {
        [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
        [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
        [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hWnd);
        [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
        [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
        [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr hwnd, int attr, out RECT pv, int cb);
    }
}
"@
}
# Match the OS's real pixels so captured coordinates line up on high-DPI screens.
[void][LumoCap.Win]::SetProcessDPIAware()
$DWMWA_EXTENDED_FRAME_BOUNDS = 9

# --- find the Lumo window ---------------------------------------------------
function Get-LumoWindow {
    $proc = Get-Process -Name "lumo" -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
    if ($null -eq $proc) { return [IntPtr]::Zero }
    return $proc.MainWindowHandle
}

# Tight window rect (DWM frame bounds excludes the invisible resize border /
# drop shadow; falls back to GetWindowRect if DWM says no).
function Get-WindowRect([IntPtr]$hWnd) {
    $r = New-Object LumoCap.RECT
    $ok = ([LumoCap.Win]::DwmGetWindowAttribute($hWnd, $DWMWA_EXTENDED_FRAME_BOUNDS, [ref]$r, [System.Runtime.InteropServices.Marshal]::SizeOf($r)) -eq 0)
    if (-not $ok -or ($r.Right - $r.Left) -le 0) {
        [void][LumoCap.Win]::GetWindowRect($hWnd, [ref]$r)
    }
    return $r
}

function Save-WindowCapture([IntPtr]$hWnd, [string]$path) {
    $r = Get-WindowRect $hWnd
    $w = $r.Right - $r.Left
    $h = $r.Bottom - $r.Top
    if ($w -le 0 -or $h -le 0) { throw "Window has no visible area." }
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    try {
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size($w, $h)))
        $g.Dispose()
        $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $bmp.Dispose()
    }
    return "$w x $h"
}

# --- parse docs/SHOTLIST.md table -------------------------------------------
# Rows look like:  | 01 | `01-hero-player-ui.png` | Feature | How to set up… |
function Get-Shots {
    $rows = @()
    foreach ($line in Get-Content -LiteralPath $shotList -Encoding UTF8) {
        if ($line -notmatch '^\s*\|') { continue }
        $cols = $line.Trim().Trim('|').Split('|') | ForEach-Object { $_.Trim() }
        if ($cols.Count -lt 4) { continue }
        if ($cols[0] -notmatch '^\d{1,2}$') { continue }
        $rows += [pscustomobject]@{
            Num     = [int]$cols[0]
            File    = $cols[1].Trim('`').Trim()
            Feature = $cols[2]
            Setup   = $cols[3]
        }
    }
    return $rows
}

$shots = Get-Shots
if ($shots.Count -eq 0) { throw "No shots parsed from $shotList — check the table format." }
if ($Only) { $shots = $shots | Where-Object { $Only -contains $_.Num } }

Write-Host ""
Write-Host "Lumo guide capture — $($shots.Count) shot(s)" -ForegroundColor Cyan
Write-Host "Saving to: $imagesDir" -ForegroundColor DarkGray
Write-Host ""

$hWnd = Get-LumoWindow
if ($hWnd -eq [IntPtr]::Zero) {
    Write-Host "Lumo window not found. Launch Lumo first (lumo.exe), then re-run." -ForegroundColor Yellow
    if (-not $DryRun) { return }
} else {
    $r = Get-WindowRect $hWnd
    Write-Host ("Found Lumo window: {0} x {1} px" -f ($r.Right-$r.Left), ($r.Bottom-$r.Top)) -ForegroundColor Green
}

if ($DryRun) {
    Write-Host ""
    Write-Host "DRY RUN — parsed shots:" -ForegroundColor Cyan
    $shots | ForEach-Object { "{0:D2}  {1,-26} {2}" -f $_.Num, $_.File, $_.Feature } | Write-Host
    return
}

# --- capture one shot: re-find window, count down, save ---------------------
function Invoke-CaptureShot($dest) {
    $h = Get-LumoWindow
    if ($h -eq [IntPtr]::Zero) { throw "Lumo window not found - is it running?" }
    Write-Host ("Arrange the shot in Lumo now. Capturing in {0}s..." -f $Countdown) -ForegroundColor Green
    for ($i = $Countdown; $i -ge 1; $i--) {
        Write-Host ("  {0}" -f $i) -NoNewline -ForegroundColor Green
        Start-Sleep -Seconds 1
    }
    Write-Host ""
    [System.Console]::Beep(880, 150)
    return (Save-WindowCapture $h $dest)
}

# --- interactive capture loop -----------------------------------------------
$saved = @(); $skipped = @()
$maxNum = ($shots | Measure-Object -Maximum Num).Maximum
:shots foreach ($shot in $shots) {
    $dest = Join-Path $imagesDir $shot.File
    if ((Test-Path $dest) -and -not $Redo) {
        Write-Host ("[{0:D2}] {1}  - already captured, skipping (use -Redo to replace)" -f $shot.Num, $shot.File) -ForegroundColor DarkGray
        continue
    }

    Write-Host ""
    Write-Host ("================ Shot {0:D2}/{1}  {2} ================" -f $shot.Num, $maxNum, $shot.Feature) -ForegroundColor Cyan
    Write-Host ("File : {0}" -f $shot.File) -ForegroundColor Gray
    Write-Host ("Setup: {0}" -f $shot.Setup) -ForegroundColor White
    Write-Host ""
    $gate = (Read-Host "[Enter] capture   [s] skip   [q] quit").Trim().ToLower()
    if ($gate -eq "q") { Write-Host "Stopped." -ForegroundColor Yellow; break shots }
    if ($gate -eq "s") { $skipped += $shot; Write-Host "Skipped $($shot.File)." -ForegroundColor Yellow; continue }

    # Capture, then let the user inspect / keep / redo the SAME shot before
    # advancing. 'r' re-runs the countdown and overwrites; 'o' opens the PNG so
    # you can eyeball it, then asks again.
    $kept = $false
    while (-not $kept) {
        try {
            $size = Invoke-CaptureShot $dest
            Write-Host ("  saved {0}  ({1})" -f $shot.File, $size) -ForegroundColor Green
        } catch {
            Write-Host ("  capture failed: {0}" -f $_.Exception.Message) -ForegroundColor Red
            $skipped += $shot
            break
        }
        $decided = $false
        while (-not $decided) {
            $ans = (Read-Host "  [Enter] keep & next   [r] redo   [o] open to inspect   [q] quit").Trim().ToLower()
            if ($ans -eq "")      { $saved += $shot; $kept = $true; $decided = $true }
            elseif ($ans -eq "r") { Write-Host "  Redoing this shot..." -ForegroundColor Yellow; $decided = $true }
            elseif ($ans -eq "o") { try { Invoke-Item -LiteralPath $dest } catch { Write-Host "  (could not open the file)" -ForegroundColor DarkGray } }
            elseif ($ans -eq "q") { Write-Host "Stopped." -ForegroundColor Yellow; break shots }
            else { Write-Host "  (unrecognized - [Enter]=keep, r=redo, o=open, q=quit)" -ForegroundColor DarkGray }
        }
    }
}

# --- report -----------------------------------------------------------------
Write-Host ""
Write-Host "Done." -ForegroundColor Cyan
Write-Host ("Saved:   {0}" -f $saved.Count) -ForegroundColor Green
if ($skipped.Count) {
    Write-Host ("Skipped: {0}  ->  {1}" -f $skipped.Count, (($skipped | ForEach-Object { $_.File }) -join ", ")) -ForegroundColor Yellow
}
$missing = $shots | Where-Object { -not (Test-Path (Join-Path $imagesDir $_.File)) }
if ($missing) {
    Write-Host ("Still missing: {0}" -f (($missing | ForEach-Object { $_.File }) -join ", ")) -ForegroundColor Yellow
} else {
    Write-Host "All shots captured. Open docs/USER_GUIDE.md to verify." -ForegroundColor Green
}




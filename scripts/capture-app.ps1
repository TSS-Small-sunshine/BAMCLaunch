param([string]$ExePath, [string]$ShotName)

# Resolve repo root from script location so the script is portable across machines
$ProjectRoot = (Resolve-Path -Path (Join-Path $PSScriptRoot "..")).Path
$ScreenshotsDir = Join-Path $ProjectRoot "screenshots"

# Kill any prior bamcl
Get-Process bamcl -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500

# Start bamcl with project root cwd (so it can find dist)
$proc = Start-Process -FilePath $ExePath -WorkingDirectory $ProjectRoot -PassThru
Start-Sleep -Seconds 4

# Wait until window appears
for ($i = 0; $i -lt 10; $i++) {
    $p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
    if ($p -and $p.MainWindowHandle -ne 0) { break }
    Start-Sleep -Milliseconds 500
}

$p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
if (-not $p -or $p.MainWindowHandle -eq 0) {
    Write-Output "bamcl PID $($proc.Id): NO WINDOW"
    exit 1
}

$hwnd = $p.MainWindowHandle
Write-Output "bamcl PID $($proc.Id), hwnd $hwnd"

# Win32 imports — Rectangle defined here to avoid earlier conflict
Add-Type @"
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public static class WinAPI {
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBlt, uint nFlags);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }

    public static System.Drawing.Rectangle GetRect(IntPtr h) {
        RECT r; GetWindowRect(h, out r);
        return new System.Drawing.Rectangle(r.Left, r.Top, r.Right - r.Left, r.Bottom - r.Top);
    }

    public static void Capture(IntPtr hWnd, string path) {
        var rect = GetRect(hWnd);
        if (rect.Width <= 0 || rect.Height <= 0) {
            throw new Exception("Invalid window size: " + rect.Width + "x" + rect.Height);
        }
        using (var bmp = new Bitmap(rect.Width, rect.Height))
        using (var g = Graphics.FromImage(bmp)) {
            IntPtr hdc = g.GetHdc();
            try {
                bool ok = PrintWindow(hWnd, hdc, 2); // PW_RENDERFULLCONTENT
                if (!ok) throw new Exception("PrintWindow failed");
            } finally {
                g.ReleaseHdc(hdc);
            }
            bmp.Save(path, ImageFormat.Png);
        }
    }
}
"@ -ReferencedAssemblies System.Drawing,System.Windows.Forms

ShowWindow($hwnd, 9) | Out-Null   # SW_RESTORE
SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Milliseconds 1500

$out = Join-Path $ScreenshotsDir "$ShotName.png"
New-Item -ItemType Directory -Force -Path $ScreenshotsDir | Out-Null
[WinAPI]::Capture($hwnd, $out)
Write-Output "Saved $out"
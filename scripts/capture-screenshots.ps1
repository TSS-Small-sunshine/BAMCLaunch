# capture-screenshots.ps1 — 加虚拟时间预算让 SPA 跑完再截图
# 用 PSScriptRoot 解析项目根目录,这样脚本在哪台机器上都能跑
$ProjectRoot = (Resolve-Path -Path (Join-Path $PSScriptRoot "..")).Path
$edge = "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
$out = Join-Path $ProjectRoot "screenshots"
New-Item -ItemType Directory -Force -Path $out | Out-Null

$pages = @(
    @{Name="home"; Path="/"},
    @{Name="instances"; Path="/instances"},
    @{Name="settings"; Path="/settings"},
    @{Name="download"; Path="/download"},
    @{Name="accounts"; Path="/accounts"}
)

foreach ($page in $pages) {
    $output = "$out\$($page.Name).png"
    & $edge --headless=new --disable-gpu --no-sandbox --hide-scrollbars --window-size=1280,800 --virtual-time-budget=5000 --screenshot="$output" "http://localhost:5199$($page.Path)" 2>&1 | Out-Null
    if (Test-Path $output) {
        $size = (Get-Item $output).Length
        Write-Output "$($page.Name): OK ($size bytes)"
    } else {
        Write-Output "$($page.Name): FAIL"
    }
}
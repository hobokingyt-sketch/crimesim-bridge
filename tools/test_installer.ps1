param(
    [Parameter(Mandatory = $true)][string]$InstallerPath,
    [Parameter(Mandatory = $true)][string]$EvidenceDirectory
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
if (-not $IsWindows) { throw 'Installer acceptance requires Windows/PowerShell 7' }
$installer = (Resolve-Path -LiteralPath $InstallerPath).Path
$evidence = [System.IO.Path]::GetFullPath($EvidenceDirectory)
New-Item -ItemType Directory -Force -Path $evidence | Out-Null
$root = Join-Path ([System.IO.Path]::GetTempPath()) ('CrimeSim-InstallCheck-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$installDir = Join-Path $root 'Installed App'
$dataDir = Join-Path $root 'Project Data'

function Run-Bounded([string]$File, [string]$Arguments, [int]$Seconds, [string]$WorkingDirectory) {
    $process = Start-Process -FilePath $File -ArgumentList $Arguments -WorkingDirectory $WorkingDirectory -PassThru
    if (-not $process.WaitForExit($Seconds * 1000)) {
        $process.Kill($true)
        $process.WaitForExit(10000) | Out-Null
        throw "Process deadline exceeded: $File"
    }
    if ($process.ExitCode -ne 0) { throw "Process failed ($($process.ExitCode)): $File" }
}

try {
    # NSIS requires /D to be LAST and unquoted, including when its path contains spaces.
    Run-Bounded $installer "/S /D=$installDir" 180 $root
    $app = Join-Path $installDir 'crimesim-bridge.exe'
    if (-not (Test-Path -LiteralPath $app -PathType Leaf)) { throw 'Installed application executable missing' }
    foreach ($name in @('CRIMESIM_BRIDGE_ROOT', 'CRIMESIM_BRIDGE_DOWNLOADS', 'CRIMESIM_BRIDGE_RUNTIME_SOURCE')) {
        Remove-Item "Env:$name" -ErrorAction SilentlyContinue
    }
    # The installed executable gets only a NEW isolated test-data root. No repository runtime override.
    Run-Bounded $app "--bridge-install-check `"$dataDir`"" 600 $installDir
    $reportPath = Join-Path $dataDir 'install_check.json'
    $report = Get-Content -Raw -LiteralPath $reportPath | ConvertFrom-Json
    if ($report.ok -ne $true -or $report.frontend_ready -ne $true -or $report.runtime_verified -ne $true) {
        throw "Installed acceptance failed: $($report.detail)"
    }
    # Provenance must come from the installed app, not another executable found on PATH.
    # File identity handles Windows short-path aliases; textual path equality does not.
    python -c "import os,sys; sys.exit(0 if os.path.samefile(sys.argv[1], sys.argv[2]) else 'The tested executable was not the installed application')" $report.executable $app
    $canary = Join-Path $dataDir 'saves/install-canary.txt'
    $before = (Get-FileHash -LiteralPath $canary -Algorithm SHA256).Hash
    Run-Bounded $installer "/S /D=$installDir" 180 $root
    if ((Get-FileHash -LiteralPath $canary -Algorithm SHA256).Hash -ne $before) {
        throw 'Reinstallation changed retained project data'
    }
    $report | Add-Member -NotePropertyName reinstall_preserved_data -NotePropertyValue $true
    $uninstaller = Join-Path $installDir 'uninstall.exe'
    if (-not (Test-Path -LiteralPath $uninstaller)) { throw 'Uninstaller was not installed' }
    # _?= keeps the child from returning before the real uninstall has finished.
    Run-Bounded $uninstaller "/S _?=$installDir" 180 $root
    if (Test-Path -LiteralPath $app) { throw 'Uninstaller left the application executable behind' }
    if ((Get-FileHash -LiteralPath $canary -Algorithm SHA256).Hash -ne $before) {
        throw 'Uninstall changed retained project data'
    }
    $report | Add-Member -NotePropertyName uninstall_preserved_data -NotePropertyValue $true
    $report | Add-Member -NotePropertyName installer_sha256 -NotePropertyValue ((Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant())
    $report | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $evidence 'install_check.json') -Encoding utf8
    Write-Host 'INSTALLED APP ACCEPTANCE PASSED: native WebView IPC, bundled Godot pipeline, reinstall and uninstall data retention.'
} finally {
    # Retain failure evidence; never remove a user workspace or depend on blanket process-name kills.
    if (Test-Path (Join-Path $dataDir 'install_check.json')) {
        Copy-Item (Join-Path $dataDir 'install_check.json') (Join-Path $evidence 'app_report.json') -Force
    }
    foreach ($folder in @('logs', 'state')) {
        if (Test-Path (Join-Path $dataDir $folder)) {
            Copy-Item (Join-Path $dataDir $folder) (Join-Path $evidence $folder) -Recurse -Force
        }
    }
    Write-Host "Isolated acceptance data retained at $root"
}

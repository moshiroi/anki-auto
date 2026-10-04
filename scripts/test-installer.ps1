$ErrorActionPreference = 'Stop'
$dist = (Resolve-Path 'dist').Path
# Replace only the download boundary; run the actual install/checksum logic.
function Invoke-WebRequest {
    param([string]$Uri, [string]$OutFile, [switch]$UseBasicParsing)
    $name = $Uri.Split('/')[-1]
    Copy-Item (Join-Path $dist $name) $OutFile
}
$tempDir = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
$checksum = Join-Path $dist 'anki-auto-x86_64-pc-windows-msvc.zip.sha256'
$original = Get-Content $checksum -Raw
try {
    & ./scripts/install.ps1 -Version 'v0.1.0' -InstallDir $tempDir -NoModifyPath
    if (-not (Test-Path (Join-Path $tempDir 'basic.json'))) { throw 'Sample missing' }
    $binary = Join-Path $tempDir 'anki-auto.exe'
    $before = (Get-FileHash $binary).Hash
    Set-Content $checksum ('0' * 64 + '  archive.zip')
    $rejected = $false
    try { & ./scripts/install.ps1 -Version 'v0.1.0' -InstallDir $tempDir -NoModifyPath }
    catch { if ($_.Exception.Message -notmatch 'Checksum mismatch') { throw }; $rejected = $true }
    if (-not $rejected) { throw 'Tampered download accepted' }
    if ((Get-FileHash $binary).Hash -ne $before) { throw 'Existing binary modified' }
    Write-Host 'Installer checksum, installed sample, and tamper rejection passed'
} finally {
    Set-Content $checksum $original -NoNewline
    Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item Function:Invoke-WebRequest
}

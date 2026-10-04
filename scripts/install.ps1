param(
    [string]$Version = '',
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'anki-auto'),
    [string]$ReleaseBase = 'https://github.com/moshiroi/anki-auto/releases',
    [switch]$NoModifyPath
)
$ErrorActionPreference = 'Stop'
# GitHub downloads require TLS 1.2 on older Windows PowerShell.
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64' -and $env:PROCESSOR_ARCHITEW6432 -ne 'AMD64') {
    throw 'This release supports Windows x64. See the README for source installation.'
}
if (-not $Version) {
    $latest = Invoke-WebRequest -Uri "$ReleaseBase/latest" -UseBasicParsing
    if ($latest.BaseResponse.ResponseUri) { $uri = $latest.BaseResponse.ResponseUri }
    else { $uri = $latest.BaseResponse.RequestMessage.RequestUri }
    $Version = $uri.AbsolutePath.Split('/')[-1]
}
if ($Version -notmatch '^v[0-9][a-zA-Z0-9._-]*$') { throw 'Cannot find a valid published release.' }
$tempDir = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tempDir | Out-Null
try {
    $archive = 'anki-auto-x86_64-pc-windows-msvc.zip'
    $archivePath = Join-Path $tempDir $archive
    Invoke-WebRequest -Uri "$ReleaseBase/download/$Version/$archive" -OutFile $archivePath -UseBasicParsing
    $checksumPath = Join-Path $tempDir 'checksum'
    Invoke-WebRequest -Uri "$ReleaseBase/download/$Version/$archive.sha256" -OutFile $checksumPath -UseBasicParsing
    $expected = ((Get-Content $checksumPath -Raw).Trim() -split '\s+')[0]
    $actual = (Get-FileHash $archivePath -Algorithm SHA256).Hash
    if ($expected -ne $actual) { throw 'Checksum mismatch; nothing was installed.' }
    $unpacked = Join-Path $tempDir 'unpacked'
    Expand-Archive -Path $archivePath -DestinationPath $unpacked
    & (Join-Path $unpacked 'anki-auto.exe') --version
    if ($LASTEXITCODE -ne 0) { throw 'The downloaded binary cannot run on this computer.' }
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    foreach ($name in @('anki-auto.exe', 'basic.json', 'LICENSE')) {
        Copy-Item (Join-Path $unpacked $name) (Join-Path $InstallDir $name) -Force
    }
    if (-not $NoModifyPath) {
        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        if (($userPath -split ';') -notcontains $InstallDir) {
            [Environment]::SetEnvironmentVariable('Path', "$InstallDir;$userPath", 'User')
        }
        $env:PATH = "$InstallDir;$env:PATH"
        Write-Host 'Added the install directory to your user PATH. Reopen other terminals to use it there.'
    }
    Write-Host "Installed $Version in $InstallDir"
    Write-Host "Example JSON: $(Join-Path $InstallDir 'basic.json')"
    Write-Host 'Next: keep Anki open and run anki-auto ping.'
} finally {
    Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}

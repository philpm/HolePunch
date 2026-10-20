# Requires Python 3.10+. Download this script before running interactive setup.
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$InstallerArguments)
$ErrorActionPreference = 'Stop'
$Python = Get-Command python -ErrorAction SilentlyContinue
if (-not $Python) { throw 'Python 3.10+ is required. Install Python and add it to PATH.' }
$LocalInstaller = Join-Path $PSScriptRoot 'deploy.py'
if (Test-Path $LocalInstaller) {
    & $Python.Source $LocalInstaller install @InstallerArguments
    exit $LASTEXITCODE
}
if ($env:HOLEPUNCH_VERSION -notmatch '^v?\d+\.\d+\.\d+[-.a-zA-Z0-9]*$') {
    throw 'Set HOLEPUNCH_VERSION to an existing release tag.'
}
$Work = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $Work | Out-Null
try {
    $Installer = Join-Path $Work 'deploy.py'
    Invoke-WebRequest "https://raw.githubusercontent.com/philpm/HolePunch/$env:HOLEPUNCH_VERSION/scripts/deploy.py" -OutFile $Installer
    Invoke-WebRequest "https://raw.githubusercontent.com/philpm/HolePunch/$env:HOLEPUNCH_VERSION/scripts/access.py" -OutFile (Join-Path $Work 'access.py')
    & $Python.Source $Installer install --version $env:HOLEPUNCH_VERSION @InstallerArguments
    $Code = $LASTEXITCODE
} finally {
    Remove-Item -Recurse -Force $Work
}
exit $Code

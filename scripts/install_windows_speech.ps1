param(
    [Parameter(Mandatory = $true)]
    [string]$Archive
)

$ErrorActionPreference = 'Stop'
$file = (Resolve-Path -LiteralPath $Archive).Path
$expected = 'D431ECFA0EE9A9FAA6B7C8CB7AE520ED1354F16C799EB6F3CE1304B10BFDBB8B'
if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ne $expected) {
    throw 'This is not the published v0.2.0 speech archive. Check the download link in README.'
}
if (!(Test-Path -LiteralPath (Join-Path $PSScriptRoot 'bin\learn-swedish.exe') -PathType Leaf)) {
    throw 'Run this script from the extracted Windows app folder.'
}

$speech = Join-Path $PSScriptRoot 'speech'
if (Test-Path -LiteralPath $speech) {
    throw "Speech already exists at $speech. Move it out of the app folder before replacing it."
}

$staging = Join-Path $PSScriptRoot ('.speech-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $staging | Out-Null
try {
    # Select only the two real bank directories, not the Linux selector symlinks.
    & "$env:SystemRoot\System32\tar.exe" -xzf $file -C $staging `
        speech/azure-release-v0.2.0 speech/adventure-release-v0.2.0
    if ($LASTEXITCODE -ne 0) { throw 'Could not extract the speech banks.' }
    $generated = Join-Path $staging 'speech'
    Rename-Item -LiteralPath (Join-Path $generated 'azure-release-v0.2.0') -NewName 'azure-sv-SE-SofieNeural'
    Rename-Item -LiteralPath (Join-Path $generated 'adventure-release-v0.2.0') -NewName 'azure-adventure'
    Move-Item -LiteralPath $generated -Destination $speech
    Write-Output 'Speech installed. Reopen Swedish Keys to use it offline.'
} finally {
    Remove-Item -LiteralPath $staging -Recurse -Force
}

# Build the standalone SDK with its pinned Rust toolchain.
param(
    [ValidateSet('debug', 'release')]
    [string]$Profile = 'release',
    [Parameter(Mandatory = $true)]
    [string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
& python (Join-Path $PSScriptRoot 'build_sdk.py') --profile $Profile --output-directory $OutputDirectory
exit $LASTEXITCODE

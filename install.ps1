# Install the cogitator dbt language server on Windows from this
# project's GitHub releases:
#
#   powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/install.ps1 | iex"
#
# Overrides (set as environment variables before running):
#   COGITATOR_REPO     owner/name of the GitHub repository
#   COGITATOR_VERSION  tag to install (default: latest release)
#   COGITATOR_BIN_DIR  install directory (default: %LOCALAPPDATA%\Programs\cogitator)

$ErrorActionPreference = "Stop"

$repo = if ($env:COGITATOR_REPO) { $env:COGITATOR_REPO } else { "sirh3e/dbt-language-server" }
$version = if ($env:COGITATOR_VERSION) { $env:COGITATOR_VERSION } else { "latest" }
$binDir = if ($env:COGITATOR_BIN_DIR) { $env:COGITATOR_BIN_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\cogitator" }

$asset = "cogitator-windows-amd64.exe"
$url = if ($version -eq "latest") {
    "https://github.com/$repo/releases/latest/download/$asset"
} else {
    "https://github.com/$repo/releases/download/$version/$asset"
}

Write-Host "installing cogitator ($asset, $version) from $repo into $binDir"
New-Item -ItemType Directory -Force -Path $binDir | Out-Null
$target = Join-Path $binDir "cogitator.exe"

try {
    Invoke-WebRequest -Uri $url -OutFile $target -UseBasicParsing
} catch {
    Write-Error @"
download failed: $url
reason:   the release or asset may not exist yet
solution: check https://github.com/$repo/releases, or build from source:
          cargo build --release -p cogitator
"@
    exit 1
}

# put the install directory on the user PATH when it is not there yet
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (($userPath -split ";") -notcontains $binDir) {
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$binDir", "User")
    Write-Host "added $binDir to your user PATH (restart the terminal to pick it up)"
}

Write-Host "installed $target"
& $target --version

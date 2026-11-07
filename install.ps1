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

$name = "cogitator-windows-amd64"
$asset = "$name.zip"
$base = if ($version -eq "latest") {
    "https://github.com/$repo/releases/latest/download"
} else {
    "https://github.com/$repo/releases/download/$version"
}

Write-Host "installing cogitator ($asset, $version) from $repo into $binDir"
New-Item -ItemType Directory -Force -Path $binDir | Out-Null
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("cogitator-install-" + [System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

try {
    try {
        Invoke-WebRequest -Uri "$base/$asset" -OutFile (Join-Path $tmp $asset) -UseBasicParsing
    } catch {
        Write-Error @"
download failed: $base/$asset
reason:   the release or asset may not exist yet
solution: check https://github.com/$repo/releases, or build from source:
          cargo build --release -p cogitator
"@
        exit 1
    }

    # verify against the release's SHA256SUMS; a missing checksum file
    # downgrades to a warning, a mismatch is always fatal
    try {
        Invoke-WebRequest -Uri "$base/SHA256SUMS" -OutFile (Join-Path $tmp "SHA256SUMS") -UseBasicParsing
        $line = Select-String -Path (Join-Path $tmp "SHA256SUMS") -Pattern ([regex]::Escape($asset)) | Select-Object -First 1
        if ($line) {
            $expected = ($line.Line -split '\s+')[0]
            $actual = (Get-FileHash -Algorithm SHA256 -Path (Join-Path $tmp $asset)).Hash
            if ($expected -ne $actual.ToLower()) {
                Write-Error @"
checksum mismatch for $asset
reason:   expected $expected, got $($actual.ToLower())
solution: re-run the installer; if it persists, the download is
          corrupted or tampered with - do not use it
"@
                exit 1
            }
            Write-Host "checksum verified"
        }
    } catch {
        Write-Warning "no SHA256SUMS in this release - skipping checksum verification"
    }

    Expand-Archive -Path (Join-Path $tmp $asset) -DestinationPath $tmp -Force
    $target = Join-Path $binDir "cogitator.exe"
    Copy-Item -Path (Join-Path $tmp "$name\cogitator.exe") -Destination $target -Force
} finally {
    Remove-Item -Recurse -Force -Path $tmp -ErrorAction SilentlyContinue
}

# put the install directory on the user PATH when it is not there yet
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (($userPath -split ";") -notcontains $binDir) {
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$binDir", "User")
    Write-Host "added $binDir to your user PATH (restart the terminal to pick it up)"
}

Write-Host "installed $target"
& $target --version

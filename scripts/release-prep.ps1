$ErrorActionPreference = "Stop"

if (-not $env:NEW_VERSION) {
    throw "NEW_VERSION was not provided by cargo-release"
}

$version = $env:NEW_VERSION
$tag = "v$version"
$isDryRun = $env:DRY_RUN -eq "true"

if ($isDryRun) {
    Write-Host "cSkipping release file mutations."
    cargo check --workspace --all-targets
    exit 0
}

if (Test-Path "package.json") {
    npm version $version --no-git-tag-version
}

if (Test-Path "CHANGELOG.md") {
    git cliff --unreleased --tag $tag --prepend CHANGELOG.md

    $changelog = Get-Content "CHANGELOG.md" -Raw
    $changelog = $changelog -replace "(\S)\r?\n(##\s+\[?v?\d)", "`$1`r`n`r`n`$2"

    Set-Content "CHANGELOG.md" -Value $changelog -NoNewline
} else {
    git cliff --unreleased --tag $tag --output CHANGELOG.md

    $changelog = Get-Content "CHANGELOG.md" -Raw
    Set-Content "CHANGELOG.md" -Value ($changelog.TrimEnd() + "`r`n") -NoNewline
}

cargo check --workspace --all-targets

$files = @(
    "Cargo.toml",
    "Cargo.lock",
    "CHANGELOG.md",
    "package.json",
    "package-lock.json",
    "npm-shrinkwrap.json"
)

foreach ($file in $files) {
    if (Test-Path $file) {
        git add $file
    }
}
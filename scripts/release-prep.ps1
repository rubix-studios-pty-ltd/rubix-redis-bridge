$ErrorActionPreference = "Stop"

if (-not $env:NEW_VERSION) {
    throw "NEW_VERSION was not provided by cargo-release"
}

$version = $env:NEW_VERSION
$tag = "v$version"

if (Test-Path "package.json") {
    npm version $version --no-git-tag-version
}

if (Test-Path "CHANGELOG.md") {
    git cliff --unreleased --tag $tag --prepend CHANGELOG.md
} else {
    git cliff --unreleased --tag $tag --output CHANGELOG.md
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

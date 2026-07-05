$ErrorActionPreference = "Stop"

if (-not $env:NEW_VERSION) {
    throw "NEW_VERSION was not provided by cargo-release"
}

$version = $env:NEW_VERSION
$tag = "v$version"
$dryRun = $env:DRY_RUN -eq "true"

$releaseFiles = @(
    "Cargo.toml",
    "Cargo.lock",
    "CHANGELOG.md",
    "package.json",
    "package-lock.json"
)

function Restore-ReleaseFiles {
    foreach ($file in $releaseFiles) {
        if (Test-Path $file) {
            git restore --staged -- $file 2>$null
            git restore --worktree -- $file 2>$null
        }
    }
}

try {
    cargo check --workspace --all-targets

    if (Test-Path "package.json") {
        npm version $version --no-git-tag-version
    }

    if (Test-Path "CHANGELOG.md") {
        git cliff --unreleased --tag $tag --prepend CHANGELOG.md
    } else {
        git cliff --unreleased --tag $tag --output CHANGELOG.md
    }

    foreach ($file in $releaseFiles) {
        if (Test-Path $file) {
            git add $file
        }
    }

    if ($dryRun) {
        Restore-ReleaseFiles
    }
}
catch {
    Restore-ReleaseFiles
    throw
}
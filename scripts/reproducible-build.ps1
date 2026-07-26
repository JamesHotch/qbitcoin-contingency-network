$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$buildRoot = Join-Path $env:TEMP ("qbtc-repro-" + [guid]::NewGuid().ToString("N"))
$targetOne = Join-Path $buildRoot "target-1"
$targetTwo = Join-Path $buildRoot "target-2"
$binaries = @(
    "qbtc-node.exe",
    "qbtc-wallet.exe",
    "qbtc-wallet-service.exe",
    "qbtc-miner.exe",
    "qbtc-signer.exe",
    "qbtc-demo.exe",
    "qbtc-bench.exe"
)

New-Item -ItemType Directory -Path $buildRoot | Out-Null

try {
    Set-Location $root
    $env:CARGO_INCREMENTAL = "0"
    if (-not $env:SOURCE_DATE_EPOCH) {
        $env:SOURCE_DATE_EPOCH = git log -1 --format=%ct
    }

    foreach ($target in @($targetOne, $targetTwo)) {
        $env:RUSTFLAGS = (
            "--remap-path-prefix=$root=Q:\qbtc-source " +
            "--remap-path-prefix=$target=Q:\qbtc-target " +
            "-C link-arg=/Brepro"
        )
        $env:CARGO_TARGET_DIR = $target
        cargo build --locked --release --bins
        if ($LASTEXITCODE -ne 0) {
            throw "Release build failed for $target"
        }
    }

    foreach ($binary in $binaries) {
        $first = (Get-FileHash -Algorithm SHA256 -LiteralPath (
            Join-Path $targetOne "release\$binary"
        )).Hash.ToLowerInvariant()
        $second = (Get-FileHash -Algorithm SHA256 -LiteralPath (
            Join-Path $targetTwo "release\$binary"
        )).Hash.ToLowerInvariant()
        if ($first -ne $second) {
            throw "NON-REPRODUCIBLE $binary`nfirst:  $first`nsecond: $second"
        }
        Write-Output "$first  $binary"
    }

    $commit = git rev-parse HEAD
    $compiler = rustc --version
    Write-Output "Reproducibility smoke check passed for commit $commit with $compiler."
}
finally {
    $resolvedBuildRoot = (Resolve-Path -LiteralPath $buildRoot).Path
    $resolvedTemp = (Resolve-Path -LiteralPath $env:TEMP).Path
    if (-not $resolvedBuildRoot.StartsWith(
        $resolvedTemp,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
        throw "Refusing cleanup outside the temporary directory: $resolvedBuildRoot"
    }
    Remove-Item -LiteralPath $resolvedBuildRoot -Recurse -Force
    Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
}

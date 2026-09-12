param(
    [switch]$SkipCodegen
)

<#
    BDJ Studio Audio Analyzer - Build de la parte nativa (Windows)

    1. Regenera el puente (flutter_rust_bridge_codegen generate).
    2. Compila el engine en release (FFI, CLI).
    3. Copia bdja_ffi.dll, bdja_cli.exe y artefactos a las carpetas Debug y Release.
#>

$ErrorActionPreference = 'Stop'

$Root       = Split-Path -Parent $PSScriptRoot
$Engine     = Join-Path $Root 'engine'
$Frontend   = Join-Path $Root 'frontend'
$FFIDebug   = Join-Path $Frontend 'build\windows\x64\runner\Debug'
$FFIRelease = Join-Path $Frontend 'build\windows\x64\runner\Release'

if (-not (Test-Path (Join-Path $Engine 'Cargo.toml'))) {
    throw "No se encontro el engine en $Engine."
}

# 1. Puente FFI
if (-not $SkipCodegen) {
    Write-Host '== 1/3 Regenerando el puente flutter_rust_bridge =='
    Push-Location $Frontend
    try {
        & flutter_rust_bridge_codegen generate
        if ($LASTEXITCODE -ne 0) { throw "flutter_rust_bridge_codegen fallo (exit $LASTEXITCODE)" }
    }
    finally { Pop-Location }
} else {
    Write-Host '== 1/3 Puente FFI: omitido (-SkipCodegen) =='
}

# 2. Engine (release)
Write-Host '== 2/3 Compilando el engine en release =='
Push-Location $Engine
try {
    & cargo build --release -p bdja_ffi -p bdja_cli
    if ($LASTEXITCODE -ne 0) { throw "cargo build fallo (exit $LASTEXITCODE)" }
}
finally { Pop-Location }

# 3. Copia de DLL y binarios
Write-Host '== 3/3 Copiando bdja_ffi.dll y artefactos a Debug y Release =='
Start-Sleep -Milliseconds 200

$artifacts = @(
    (Join-Path $Engine 'target\release\bdja_ffi.dll'),
    (Join-Path $Engine 'target\release\bdja_cli.exe'),
    (Join-Path $Root 'logo.png')
)

foreach ($dir in @($FFIDebug, $FFIRelease)) {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    foreach ($bin in $artifacts) {
        if (-not (Test-Path $bin)) { throw "Falta artefacto: $bin" }
        $dst = Join-Path $dir (Split-Path $bin -Leaf)
        try {
            Copy-Item $bin $dir -Force
            Write-Host "   -> $dst"
        }
        catch {
            Write-Warning "No se pudo sobrescribir $dst : $($_.Exception.Message)"
        }
    }
}
Write-Host '== Build nativo finalizado con exito =='

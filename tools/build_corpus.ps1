<#
.SYNOPSIS
    BDJ Studio Audio Analyzer — Wrapper PowerShell para Generador de Corpus (Fase 0.2)
.DESCRIPTION
    Ejecuta tools/corpus_build/build_corpus.py para generar las 9 variantes transcode
    por máster y el manifest.csv compatible con bdja_cli validate / calibrate.
.EXAMPLE
    .\tools\build_corpus.ps1 -MastersDir "C:\Audio\Masters" -OutputDir "C:\Audio\Corpus"
#>

[CmdletBinding()]
param (
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$MastersDir,

    [Parameter(Mandatory = $true, Position = 1)]
    [string]$OutputDir,

    [Parameter(Mandatory = $false)]
    [int]$Workers = 0,

    [Parameter(Mandatory = $false)]
    [int]$Limit = 0,

    [Parameter(Mandatory = $false)]
    [string]$Ffmpeg = "ffmpeg"
)

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$PythonScript = Join-Path $ScriptDir "corpus_build\build_corpus.py"

if (-not (Test-Path $PythonScript)) {
    Write-Error "No se encontró el script en: $PythonScript"
    exit 1
}

$PyArgs = @(
    $PythonScript,
    "--masters-dir", $MastersDir,
    "--output-dir", $OutputDir,
    "--ffmpeg", $Ffmpeg
)

if ($Workers -gt 0) {
    $PyArgs += @("--workers", $Workers)
}
if ($Limit -gt 0) {
    $PyArgs += @("--limit", $Limit)
}

Write-Host "Ejecutando generador de corpus (Fase 0.2)..." -ForegroundColor Cyan
python @PyArgs
if ($LASTEXITCODE -ne 0) {
    Write-Error "Fallo en la generación de corpus (código $LASTEXITCODE)"
    exit $LASTEXITCODE
}

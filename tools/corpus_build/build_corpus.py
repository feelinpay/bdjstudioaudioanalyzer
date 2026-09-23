#!/usr/bin/env python3
"""
BDJ Studio Audio Analyzer — Generador Automatizado de Corpus Transcode (Fase 0.2)
================================================================================
Genera las 9 variantes transcode estándar por cada pista máster lossless:
  1. MP3 CBR 128k
  2. MP3 CBR 192k
  3. MP3 CBR 256k
  4. MP3 CBR 320k
  5. MP3 VBR V2 (~190 kbps)
  6. MP3 VBR V0 (~245 kbps)
  7. AAC CBR 128k
  8. AAC CBR 192k
  9. AAC CBR 256k

Invariantes metodológicos (C-1 a C-6):
  - C-1: Simetría estricta de clases: los másters auténticos se decodifican al
    mismo formato que los transcodes (PCM 16-bit a la frecuencia nativa del máster),
    garantizando que la única diferencia sistemática sea el paso lossy.
  - C-2: Escritura atómica a archivo `.part` y `os.replace` al completar.
  - C-3: Registro exhaustivo de fallos en `failures.csv` y advertencia de integridad.
  - C-5: Verificación de reducción física de tamaño en el intermedio lossy.
  - C-6: Registro de encoders, versiones y metadatos en `corpus_meta.json`.
  - Menor: `-map 0:a:0 -vn` en todas las conversiones para descartar streams de video/artworks.
"""

import argparse
import csv
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
from pathlib import Path

AUDIO_EXTENSIONS = {".wav", ".flac", ".aif", ".aiff", ".alac"}

VARIANTS = [
    {
        "id": "mp3_cbr128",
        "codec": "MP3",
        "bitrate": "128k",
        "lossy_ext": ".mp3",
        "encode_args": ["-c:a", "libmp3lame", "-b:a", "128k"],
    },
    {
        "id": "mp3_cbr192",
        "codec": "MP3",
        "bitrate": "192k",
        "lossy_ext": ".mp3",
        "encode_args": ["-c:a", "libmp3lame", "-b:a", "192k"],
    },
    {
        "id": "mp3_cbr256",
        "codec": "MP3",
        "bitrate": "256k",
        "lossy_ext": ".mp3",
        "encode_args": ["-c:a", "libmp3lame", "-b:a", "256k"],
    },
    {
        "id": "mp3_cbr320",
        "codec": "MP3",
        "bitrate": "320k",
        "lossy_ext": ".mp3",
        "encode_args": ["-c:a", "libmp3lame", "-b:a", "320k"],
    },
    {
        "id": "mp3_vbr_v2",
        "codec": "MP3",
        "bitrate": "V2",
        "lossy_ext": ".mp3",
        "encode_args": ["-c:a", "libmp3lame", "-q:a", "2"],
    },
    {
        "id": "mp3_vbr_v0",
        "codec": "MP3",
        "bitrate": "V0",
        "lossy_ext": ".mp3",
        "encode_args": ["-c:a", "libmp3lame", "-q:a", "0"],
    },
    {
        "id": "aac_cbr128",
        "codec": "AAC",
        "bitrate": "128k",
        "lossy_ext": ".m4a",
        "encode_args": ["-b:a", "128k"],
    },
    {
        "id": "aac_cbr192",
        "codec": "AAC",
        "bitrate": "192k",
        "lossy_ext": ".m4a",
        "encode_args": ["-b:a", "192k"],
    },
    {
        "id": "aac_cbr256",
        "codec": "AAC",
        "bitrate": "256k",
        "lossy_ext": ".m4a",
        "encode_args": ["-b:a", "256k"],
    },
]


def check_ffmpeg(ffmpeg_bin: str) -> tuple[bool, str, str]:
    """Verifica ffmpeg y detecta versión y encoder AAC disponible."""
    try:
        res = subprocess.run(
            [ffmpeg_bin, "-version"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=True,
        )
        version_line = res.stdout.splitlines()[0] if res.stdout else "unknown"

        # Detectar encoder AAC disponible
        res_enc = subprocess.run(
            [ffmpeg_bin, "-encoders"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=True,
        )
        aac_encoder = "aac"
        if "libfdk_aac" in res_enc.stdout:
            aac_encoder = "libfdk_aac"

        return True, version_line, aac_encoder
    except Exception as e:
        return False, str(e), "aac"


def get_audio_sample_rate(file_path: Path, ffmpeg_bin: str) -> int:
    """Extrae la frecuencia de muestreo de un archivo de audio mediante ffprobe o ffmpeg."""
    try:
        # Intentar ffprobe si está disponible en el mismo directorio
        probe_bin = "ffprobe"
        ffmpeg_dir = Path(ffmpeg_bin).parent
        if (ffmpeg_dir / "ffprobe.exe").exists():
            probe_bin = str(ffmpeg_dir / "ffprobe.exe")
        elif (ffmpeg_dir / "ffprobe").exists():
            probe_bin = str(ffmpeg_dir / "ffprobe")

        cmd = [
            probe_bin,
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=sample_rate",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            str(file_path),
        ]
        res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        if res.returncode == 0 and res.stdout.strip().isdigit():
            return int(res.stdout.strip())
    except Exception:
        pass

    # Fallback parseando salida stderr de ffmpeg -i
    try:
        cmd = [ffmpeg_bin, "-i", str(file_path)]
        res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        m = re.search(r"(\d{5,6})\s*Hz", res.stderr)
        if m:
            return int(m.group(1))
    except Exception:
        pass

    return 44100


def find_masters(masters_dir: Path, limit: int = 0) -> list[Path]:
    """Busca archivos de audio lossless en el directorio de másters."""
    masters = []
    for root, _, files in os.walk(masters_dir):
        for f in files:
            p = Path(root) / f
            if p.suffix.lower() in AUDIO_EXTENSIONS:
                masters.append(p)
                if limit > 0 and len(masters) >= limit:
                    return sorted(masters)
    return sorted(masters)


def process_lossless_master(
    master_path: Path,
    lossless_dir: Path,
    ffmpeg_bin: str,
) -> tuple[dict | None, str | None]:
    """
    C-1: Decodifica el máster original a WAV PCM 16-bit a la misma tasa nativa,
    garantizando simetría de formato con los transcodes y descartando video/artworks.
    C-2: Escritura atómica a `.part` y `os.replace`.
    """
    track_stem = master_path.stem
    out_wav = lossless_dir / f"{track_stem}.wav"
    part_wav = lossless_dir / f"{track_stem}.wav.part"

    sample_rate = get_audio_sample_rate(master_path, ffmpeg_bin)

    if out_wav.exists() and out_wav.stat().st_size > 1000:
        return {
            "path": str(out_wav.resolve()),
            "ground_truth": "lossless",
            "codec_origen": "PCM",
            "bitrate": "1411k",
            "variant_id": "lossless_master",
            "master_source": master_path.name,
            "sample_rate": str(sample_rate),
        }, None

    try:
        cmd = [
            ffmpeg_bin,
            "-y",
            "-v",
            "error",
            "-i",
            str(master_path),
            "-map",
            "0:a:0",
            "-vn",
            "-map_metadata",
            "-1",
            "-c:a",
            "pcm_s16le",
            "-f",
            "wav",
            str(part_wav),
        ]
        ret = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        if ret.returncode != 0:
            err = f"Fallo al normalizar máster {master_path.name} a PCM 16-bit: {ret.stderr.strip()}"
            return None, err

        if not part_wav.exists() or part_wav.stat().st_size < 1000:
            err = f"Archivo máster normalizado resultante corrupto o vacío: {master_path.name}"
            return None, err

        os.replace(part_wav, out_wav)

        return {
            "path": str(out_wav.resolve()),
            "ground_truth": "lossless",
            "codec_origen": "PCM",
            "bitrate": "1411k",
            "variant_id": "lossless_master",
            "master_source": master_path.name,
            "sample_rate": str(sample_rate),
        }, None
    except Exception as e:
        if part_wav.exists():
            try:
                part_wav.unlink()
            except Exception:
                pass
        return None, str(e)


def process_variant(
    master_path: Path,
    normalized_master_wav: Path,
    variant: dict,
    transcode_dir: Path,
    ffmpeg_bin: str,
    aac_encoder: str,
) -> tuple[dict | None, str | None]:
    """
    Genera una variante transcodeada y re-decodificada a WAV PCM 16-bit.
    C-1: Simetría de contenedor y descarte de video/portadas.
    C-2: Escritura atómica a `.part` y `os.replace`.
    C-5: Validación de reducción de tamaño en intermedio lossy.
    """
    track_stem = master_path.stem
    var_id = variant["id"]
    lossy_ext = variant["lossy_ext"]
    out_wav = transcode_dir / f"{track_stem}_{var_id}.wav"
    part_wav = transcode_dir / f"{track_stem}_{var_id}.wav.part"

    master_size = normalized_master_wav.stat().st_size if normalized_master_wav.exists() else 0
    sample_rate = get_audio_sample_rate(normalized_master_wav, ffmpeg_bin)

    # Si ya existe completamente decodificado, retornar
    if out_wav.exists() and out_wav.stat().st_size > 1000:
        return {
            "path": str(out_wav.resolve()),
            "ground_truth": "transcode",
            "codec_origen": variant["codec"],
            "bitrate": variant["bitrate"],
            "variant_id": var_id,
            "master_source": master_path.name,
            "sample_rate": str(sample_rate),
        }, None

    with tempfile.NamedTemporaryFile(suffix=lossy_ext, delete=False) as tmp_lossy:
        tmp_lossy_path = Path(tmp_lossy.name)

    try:
        # Configurar encoder para AAC
        encode_args = list(variant["encode_args"])
        if variant["codec"] == "AAC":
            encode_args = ["-c:a", aac_encoder] + encode_args

        # Paso 1: Codificar master normalizado a formato lossy
        cmd_enc = [
            ffmpeg_bin,
            "-y",
            "-v",
            "error",
            "-i",
            str(normalized_master_wav),
            "-map",
            "0:a:0",
            "-vn",
            "-map_metadata",
            "-1",
            *encode_args,
            str(tmp_lossy_path),
        ]
        ret = subprocess.run(
            cmd_enc, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
        )
        if ret.returncode != 0:
            err = f"Fallo al codificar {master_path.name} a {var_id}: {ret.stderr.strip()}"
            return None, err

        # C-5: Verificar que el intermedio lossy sea materialmente menor que el PCM sin comprimir
        lossy_size = tmp_lossy_path.stat().st_size if tmp_lossy_path.exists() else 0
        if master_size > 0 and lossy_size >= master_size * 0.95:
            err = f"Anomalía en {var_id}: el archivo intermedio no redujo tamaño ({lossy_size} >= {master_size} bytes)"
            return None, err

        # Paso 2: Decodificar lossy a WAV PCM 16-bit (simulando fake-lossless)
        # C-2: Escribir a `.part` primero
        cmd_dec = [
            ffmpeg_bin,
            "-y",
            "-v",
            "error",
            "-i",
            str(tmp_lossy_path),
            "-map",
            "0:a:0",
            "-vn",
            "-map_metadata",
            "-1",
            "-c:a",
            "pcm_s16le",
            "-f",
            "wav",
            str(part_wav),
        ]
        ret2 = subprocess.run(
            cmd_dec, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
        )
        if ret2.returncode != 0:
            err = f"Fallo al decodificar {var_id} a WAV: {ret2.stderr.strip()}"
            return None, err

        if not part_wav.exists() or part_wav.stat().st_size < 1000:
            err = f"Archivo WAV resultante de {var_id} corrupto o truncado"
            return None, err

        # C-2: Renombrado atómico
        os.replace(part_wav, out_wav)

        return {
            "path": str(out_wav.resolve()),
            "ground_truth": "transcode",
            "codec_origen": variant["codec"],
            "bitrate": variant["bitrate"],
            "variant_id": var_id,
            "master_source": master_path.name,
            "sample_rate": str(sample_rate),
        }, None
    except Exception as e:
        if part_wav.exists():
            try:
                part_wav.unlink()
            except Exception:
                pass
        return None, str(e)
    finally:
        if tmp_lossy_path.exists():
            try:
                tmp_lossy_path.unlink()
            except Exception:
                pass


def main():
    parser = argparse.ArgumentParser(
        description="Generador automatizado de corpus de transcodes para BDJ Studio Audio Analyzer (Fase 0.2)"
    )
    parser.add_argument(
        "--masters-dir",
        "-m",
        required=True,
        type=Path,
        help="Directorio con archivos de audio auténticos (lossless)",
    )
    parser.add_argument(
        "--output-dir",
        "-o",
        required=True,
        type=Path,
        help="Directorio destino para el corpus generado, manifest.csv y corpus_meta.json",
    )
    parser.add_argument(
        "--workers",
        "-w",
        type=int,
        default=min(os.cpu_count() or 4, 16),
        help="Hilos paralelos de ffmpeg (por defecto: CPU cores, máx 16)",
    )
    parser.add_argument(
        "--limit",
        "-l",
        type=int,
        default=0,
        help="Límite de másters a procesar (0 = todos)",
    )
    parser.add_argument(
        "--ffmpeg",
        default="ffmpeg",
        help="Ruta al binario de ffmpeg",
    )

    args = parser.parse_args()

    masters_dir = args.masters_dir.resolve()
    if not masters_dir.is_dir():
        print(f"Error: El directorio de másters '{masters_dir}' no existe o no es carpeta.")
        sys.exit(1)

    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    ok_ffmpeg, version_str, aac_enc = check_ffmpeg(args.ffmpeg)
    if not ok_ffmpeg:
        print(
            f"Error: No se encontró o no se pudo ejecutar ffmpeg ('{args.ffmpeg}'). {version_str}"
        )
        sys.exit(1)

    print("=" * 68)
    print(" BDJ STUDIO AUDIO ANALYZER — GENERADOR DE CORPUS (FASE 0.2)")
    print("=" * 68)
    print(f"Directorio másters : {masters_dir}")
    print(f"Directorio destino : {output_dir}")
    print(f"Hilos de trabajo   : {args.workers}")
    print(f"Versión FFmpeg     : {version_str}")
    print(f"Encoder AAC        : {aac_enc}")

    masters = find_masters(masters_dir, args.limit)
    if not masters:
        print(f"No se encontraron archivos lossless soportados en {masters_dir}.")
        sys.exit(1)

    print(f"Másters auténticos encontrados: {len(masters)}")
    total_expected_transcodes = len(masters) * len(VARIANTS)
    print(f"Variantes a generar           : {len(VARIANTS)} por máster ({total_expected_transcodes} en total)")

    lossless_dir = output_dir / "lossless"
    lossless_dir.mkdir(exist_ok=True)

    transcode_dir = output_dir / "transcode"
    transcode_dir.mkdir(exist_ok=True)

    manifest_rows = []
    failures = []

    # C-1: Procesar y normalizar cada máster auténtico a PCM 16-bit
    print("\nNormalizando másters lossless (C-1: simetría estricta de contenedor)...")
    normalized_masters = {}
    for m in masters:
        row, err = process_lossless_master(m, lossless_dir, args.ffmpeg)
        if row:
            manifest_rows.append(row)
            normalized_masters[m.name] = Path(row["path"])
        else:
            failures.append({"master": m.name, "variant": "lossless_master", "error": err})
            print(f"  [FALLO NORMALIZACIÓN] {m.name}: {err}")

    # C-2 / C-5: Generar transcodes en paralelo
    tasks = []
    for m in masters:
        norm_wav = normalized_masters.get(m.name)
        if not norm_wav:
            continue
        for v in VARIANTS:
            tasks.append((m, norm_wav, v))

    print(f"\nIniciando generación de {len(tasks)} transcodes...")
    start_time = time.time()
    completed = 0

    with ThreadPoolExecutor(max_workers=args.workers) as executor:
        futures = {
            executor.submit(
                process_variant, m, norm_wav, v, transcode_dir, args.ffmpeg, aac_enc
            ): (m, v)
            for m, norm_wav, v in tasks
        }

        for fut in as_completed(futures):
            m, v = futures[fut]
            res, err = fut.result()
            completed += 1
            if res:
                manifest_rows.append(res)
            else:
                failures.append({"master": m.name, "variant": v["id"], "error": err})

            if completed % 10 == 0 or completed == len(tasks):
                percent = (completed / len(tasks)) * 100.0
                elapsed = time.time() - start_time
                rate = completed / elapsed if elapsed > 0 else 0
                sys.stdout.write(
                    f"\rProgreso: {completed}/{len(tasks)} ({percent:.1f}%) — {rate:.1f} pistas/s"
                )
                sys.stdout.flush()

    print("\n")
    elapsed_total = time.time() - start_time
    print(f"Generación finalizada en {elapsed_total:.2f} s.")

    # C-3: Registrar fallos si los hubo
    failures_file = output_dir / "failures.csv"
    if failures:
        with open(failures_file, "w", newline="", encoding="utf-8") as f:
            writer = csv.DictWriter(f, fieldnames=["master", "variant", "error"])
            writer.writeheader()
            for fl in failures:
                writer.writerow(fl)
        print(
            f"\n[ADVERTENCIA: CORPUS INCOMPLETO] Se registraron {len(failures)} fallos en: {failures_file.name}"
        )
    elif failures_file.exists():
        try:
            failures_file.unlink()
        except Exception:
            pass

    # Escribir manifest.csv
    manifest_file = output_dir / "manifest.csv"
    with open(manifest_file, "w", newline="", encoding="utf-8") as f:
        fieldnames = [
            "path",
            "ground_truth",
            "codec_origen",
            "bitrate",
            "variant_id",
            "master_source",
            "sample_rate",
        ]
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()
        for row in manifest_rows:
            writer.writerow(row)

    # C-6: Escribir corpus_meta.json con trazabilidad y reproducibilidad
    meta_file = output_dir / "corpus_meta.json"
    corpus_meta = {
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "ffmpeg_version": version_str,
        "mp3_encoder": "libmp3lame",
        "aac_encoder": aac_enc,
        "total_masters_input": len(masters),
        "lossless_normalized_ok": len(normalized_masters),
        "transcodes_generated_ok": len(manifest_rows) - len(normalized_masters),
        "failures_count": len(failures),
        "is_complete": len(failures) == 0,
        "variants": [
            {"id": v["id"], "codec": v["codec"], "bitrate": v["bitrate"]}
            for v in VARIANTS
        ],
    }
    with open(meta_file, "w", encoding="utf-8") as f:
        json.dump(corpus_meta, f, indent=2)

    print(f"\nManifiesto guardado en : {manifest_file.resolve()}")
    print(f"Metadatos guardados en : {meta_file.resolve()}")
    print(f"Total registros en manifest.csv: {len(manifest_rows)}")
    print("Estructura generada:")
    print(f"  - Másters auténticos (lossless PCM 16-bit): {len(normalized_masters)}")
    print(f"  - Transcodes generados (fake-lossless WAV) : {len(manifest_rows) - len(normalized_masters)}")
    print("\nPuedes validar este corpus ejecutando:")
    print(f"  cargo run --bin bdja_cli -- validate --manifest \"{manifest_file.resolve()}\"")
    print("=" * 68)


if __name__ == "__main__":
    main()

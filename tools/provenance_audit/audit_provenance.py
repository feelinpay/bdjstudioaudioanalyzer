#!/usr/bin/env python3
"""
BDJ Studio Audio Analyzer — Auditoría de procedencia INDEPENDIENTE del motor
============================================================================
Mide magnitudes espectrales y estructurales de un archivo de audio usando
únicamente ffmpeg + numpy. NO importa, invoca ni consulta bdja_cli ni ninguna
parte del motor: es la herramienta con la que se decide si un candidato entra
o no en la clase negativa del corpus, y por eso debe ser independiente por
construcción.

Los umbrales están PRE-REGISTRADOS en UMBRALES y son deliberadamente distintos
en método y en valor a los del motor (ventana absoluta de 1 kHz en lugar de
relativa, autocorrelación de envolvente en lugar de rejilla de bloques).

Salida: una fila CSV por archivo con las magnitudes medidas y la decisión
aplicada por la regla. La regla NO usa el veredicto del motor.

Uso:
    python audit_provenance.py --input <carpeta|archivo> \
        --tier A1 --source "rip CD propio, AccurateRip OK" \
        --out auditoria.csv
"""

import argparse
import csv
import json
import os
import subprocess
import sys
from pathlib import Path

import numpy as np

AUDIO_EXT = {".wav", ".flac", ".aif", ".aiff", ".m4a", ".mp3", ".ogg", ".opus", ".wv", ".alac"}

# ----------------------------------------------------------------------------
# UMBRALES PRE-REGISTRADOS — fijar antes de auditar el primer archivo y no
# volver a tocarlos durante la recolección. Cualquier cambio invalida las
# decisiones ya tomadas y obliga a reauditar el conjunto completo.
# ----------------------------------------------------------------------------
UMBRALES = {
    # Firma de corte: caída sostenida medida entre dos ventanas ABSOLUTAS de
    # 1 kHz, una antes y otra después del candidato.
    "cliff_drop_db": 25.0,        # caída mínima para considerarla firma de códec
    "cliff_max_hz": 21000.0,      # por encima de esto es el Nyquist del máster, no un códec
    # Colapso de la banda extrema respecto a la banda media.
    "hf_collapse_db": -38.0,      # [19k..Nyq] menos [10k..16k]
    # Profundidad de bits real frente a la declarada.
    "bit_inflation": 4,           # bits declarados menos bits reales
}

# NOTA v2 — La medida de periodicidad de trama MDCT por autocorrelación de
# envolvente se RETIRÓ de esta herramienta. Validada contra controles lossless
# sintéticos resultó no fiable en las dos direcciones: un máster de club muy
# limitado con el bajo cerca de sr/576 (76,6 Hz) disparaba el retardo de 576
# muestras sin haber pasado jamás por un códec, y una variante con paso banda
# previo disparaba sobre TODO, incluidos másters limpios. Detectar la rejilla
# MDCT de verdad exige transformar con la misma ventana y solape del códec y
# probar los desplazamientos de alineación; la autocorrelación de envolvente no
# es un sustituto. Se prefiere no medirla a medirla mal: una magnitud que no se
# ha podido validar no puede decidir qué entra en la clase negativa.

# Etiquetas que SOLO puede haber escrito un codificador con pérdida: son prueba
# documental de que el archivo pasó por uno.
LOSSY_ENCODER_HINTS = ("lame", "itunnorm", "itunsmpb", "nero", "fhgaac",
                       "fraunhofer", "gogo", "xing", "fastenc", "helix", "shine")
# Etiquetas de conversores genéricos: NO prueban nada. Un WAV legítimo convertido
# con ffmpeg lleva "Lavf" igual que un fake. Mismo error que se corrigió en el
# motor al degradar E13 ante "Lavf" y ante los chunks ID3 de Serato/Rekordbox.
NEUTRAL_CONVERTER_HINTS = ("lavf", "lavc", "dbpoweramp", "xrecode", "foobar",
                           "adobe", "audition", "sox", "wavelab")


def run(cmd):
    return subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=False)


def probe(path):
    """Datos declarados del contenedor y metadatos, vía ffprobe."""
    cmd = ["ffprobe", "-v", "quiet", "-print_format", "json",
           "-show_format", "-show_streams", str(path)]
    r = run(cmd)
    if r.returncode != 0:
        return None
    try:
        info = json.loads(r.stdout.decode("utf-8", "replace"))
    except Exception:
        return None
    astream = next((s for s in info.get("streams", []) if s.get("codec_type") == "audio"), None)
    if astream is None:
        return None

    tags = {}
    for src in (info.get("format", {}).get("tags", {}), astream.get("tags", {})):
        for k, v in (src or {}).items():
            tags[k.lower()] = str(v)
    blob = " ".join(f"{k}={v}" for k, v in tags.items()).lower()
    lossy_hints = sorted({h for h in LOSSY_ENCODER_HINTS if h in blob})
    neutral_hints = sorted({h for h in NEUTRAL_CONVERTER_HINTS if h in blob})

    fmt = astream.get("sample_fmt", "")
    declared_bits = 16
    if "s32" in fmt or "flt" in fmt:
        declared_bits = 32
    elif "s24" in fmt or astream.get("bits_per_raw_sample") in ("24", 24):
        declared_bits = 24
    if astream.get("bits_per_raw_sample") not in (None, "0", 0):
        try:
            declared_bits = int(astream["bits_per_raw_sample"])
        except ValueError:
            pass

    return {
        "codec": astream.get("codec_name", "?"),
        "sample_rate": int(astream.get("sample_rate", 0) or 0),
        "channels": int(astream.get("channels", 0) or 0),
        "declared_bits": declared_bits,
        "duration_s": float(info.get("format", {}).get("duration", 0) or 0),
        "lossy_encoder_tags": ";".join(lossy_hints),
        "converter_tags": ";".join(neutral_hints),
    }


def decode_mono(path, sample_rate, max_seconds=180):
    """PCM mono float32 a la tasa nativa. Sin remuestrear: preserva el Nyquist."""
    cmd = ["ffmpeg", "-v", "quiet", "-i", str(path), "-map", "0:a:0", "-vn",
           "-t", str(max_seconds), "-ac", "1", "-f", "f32le", "-"]
    r = run(cmd)
    if r.returncode != 0 or len(r.stdout) < 4096:
        return None
    return np.frombuffer(r.stdout, dtype="<f4").astype(np.float64)


def decode_int(path, max_seconds=60):
    """PCM entero de 32 bits para estimar la profundidad de bits realmente usada."""
    cmd = ["ffmpeg", "-v", "quiet", "-i", str(path), "-map", "0:a:0", "-vn",
           "-t", str(max_seconds), "-ac", "1", "-f", "s32le", "-"]
    r = run(cmd)
    if r.returncode != 0 or len(r.stdout) < 4096:
        return None
    return np.frombuffer(r.stdout, dtype="<i4")


def average_spectrum(x, sr, nfft=8192, n_windows=48):
    """Espectro de potencia promediado sobre ventanas repartidas por toda la pista."""
    if len(x) < nfft * 2:
        return None, None
    win = np.hanning(nfft)
    starts = np.linspace(0, len(x) - nfft - 1, n_windows).astype(int)
    acc = np.zeros(nfft // 2 + 1)
    used = 0
    for s in starts:
        seg = x[s:s + nfft]
        if not np.any(seg):
            continue
        acc += np.abs(np.fft.rfft(seg * win)) ** 2
        used += 1
    if used == 0:
        return None, None
    acc /= used
    freqs = np.fft.rfftfreq(nfft, 1.0 / sr)
    db = 10.0 * np.log10(acc + 1e-20)
    return freqs, db


def band_db(freqs, db, lo, hi):
    m = (freqs >= lo) & (freqs < hi)
    return float(np.mean(db[m])) if np.any(m) else float("nan")


def smooth(v, k=9):
    if k < 3:
        return v
    ker = np.ones(k) / k
    return np.convolve(v, ker, mode="same")


def find_cliff(freqs, db, sr):
    """
    Ventana ABSOLUTA de 1 kHz antes y después del candidato — deliberadamente
    distinta de la ventana relativa del motor. Devuelve la caída sostenida
    máxima y dónde ocurre.
    """
    nyq = sr / 2.0
    sm = smooth(db, 9)
    best = (0.0, float("nan"))
    f = 3000.0
    while f < nyq - 600.0:
        pre = band_db(freqs, sm, f - 1000.0, f)
        post_near = band_db(freqs, sm, f + 200.0, min(f + 1200.0, nyq))
        post_all = band_db(freqs, sm, f + 200.0, nyq)
        if not (np.isnan(pre) or np.isnan(post_near) or np.isnan(post_all)):
            # sostenida: exige que no vuelva a subir por encima del escalón
            drop = pre - max(post_near, post_all - 3.0)
            if drop > best[0]:
                best = (float(drop), float(f))
        f += 100.0
    return best[1], best[0]


def real_bits(xi):
    """Bits realmente usados: cuenta cuántos bits bajos son siempre cero."""
    if xi is None or len(xi) == 0:
        return None
    nz = xi[xi != 0]
    if len(nz) == 0:
        return 0
    acc = np.bitwise_or.reduce(np.abs(nz).astype(np.uint32))
    zeros = 0
    while zeros < 32 and (acc >> np.uint32(zeros)) & np.uint32(1) == 0:
        zeros += 1
    return int(32 - zeros)


# ----------------------------------------------------------------------------
# REGLA DE DECISIÓN PRE-REGISTRADA
# ----------------------------------------------------------------------------
# Dos principios, y los dos existen para que la cifra de FPR no se pueda
# maquillar sin que se note:
#
# 1. La evidencia espectral solo DEGRADA una procedencia débil. Jamás excluye
#    un A1. Un A1 con firma lossy se admite igualmente como negativo y, si el
#    motor lo condena, cuenta como falso positivo.
#
# 2. En A2, una medición espectral sospechosa APARTA el archivo
#    (provenance_unknown); no lo reclasifica como positivo. Reclasificar por
#    medición sería filtrar la clase negativa hacia lo que el motor encuentra
#    fácil, que es la circularidad entrando por la puerta de atrás. Solo la
#    prueba documental (etiqueta de un codificador con pérdida) o el acuerdo
#    de dos mediciones independientes pueden afirmar lossy_confirmed en A2.
# ----------------------------------------------------------------------------
def clasificar_evidencia(m):
    """Separa la evidencia en decisiva y corroborante. Devuelve (decisivas, corroborantes)."""
    u = UMBRALES
    decisivas, corroborantes = [], []

    if (m["cliff_drop_db"] >= u["cliff_drop_db"]
            and m["cliff_hz"] == m["cliff_hz"]
            and m["cliff_hz"] <= u["cliff_max_hz"]):
        decisivas.append(
            f"corte sostenido de {m['cliff_drop_db']:.1f} dB en {m['cliff_hz']:.0f} Hz")

    if m["lossy_encoder_tags"]:
        decisivas.append(f"etiqueta de codificador con perdida: {m['lossy_encoder_tags']}")

    if m["hf_ratio_db"] <= u["hf_collapse_db"]:
        corroborantes.append(f"banda extrema colapsada {m['hf_ratio_db']:.1f} dB")

    if (m["declared_bits"] and m["real_bits"]
            and m["declared_bits"] - m["real_bits"] >= u["bit_inflation"]):
        corroborantes.append(
            f"bits inflados: declara {m['declared_bits']} y usa {m['real_bits']}")

    return decisivas, corroborantes


def decide(tier, m):
    """Devuelve (decision, motivo). No consulta el motor en ningún punto."""
    dec, cor = clasificar_evidencia(m)
    todas = dec + cor
    motivo = " | ".join(todas) if todas else "sin firmas de compresion previa"
    doc = bool(m["lossy_encoder_tags"])

    # --- A1: rip propio con AccurateRip, o entrega del creador con cadena
    #         documentada. La medición NUNCA excluye. Solo avisa.
    if tier == "A1":
        if todas:
            return "lossless_flag", f"ADMITIDO (A1 no se excluye por medicion). AVISO: {motivo}"
        return "lossless", motivo

    # --- A2: compra directa al artista, sin cadena documentada.
    if tier == "A2":
        if doc or len(cor) >= 2:
            return "lossy_confirmed", motivo
        if todas:
            return "provenance_unknown", f"apartado, no reclasificado: {motivo}"
        return "lossless", motivo

    # --- B: tienda sin cadena declarada, pool, netlabel, entrega de un amigo.
    #        Nunca entra al estrato de certificacion, gane o pierda.
    if dec or len(cor) >= 2:
        return "lossy_confirmed", motivo
    if todas:
        return "provenance_unknown", f"una sola firma corroborante: {motivo}"
    return "exploratory_clean", motivo


def measure(path):
    info = probe(path)
    if info is None:
        return None, "ffprobe no pudo leer el archivo"
    x = decode_mono(path, info["sample_rate"])
    if x is None:
        return None, "ffmpeg no pudo decodificar audio"
    sr = info["sample_rate"]
    freqs, db = average_spectrum(x, sr)
    if freqs is None:
        return None, "pista demasiado corta o en silencio para medir espectro"

    ref = band_db(freqs, smooth(db, 9), 1000.0, 6000.0)
    nyq = sr / 2.0
    cliff_hz, cliff_drop = find_cliff(freqs, db, sr)
    hf_lo = min(19000.0, nyq - 500.0)
    m = {
        "codec": info["codec"],
        "sample_rate": sr,
        "channels": info["channels"],
        "duration_s": round(info["duration_s"], 1),
        "declared_bits": info["declared_bits"],
        "real_bits": real_bits(decode_int(path)),
        "lossy_encoder_tags": info["lossy_encoder_tags"],
        "converter_tags": info["converter_tags"],
        "ref_db": round(ref, 1),
        "cliff_hz": round(cliff_hz, 0) if cliff_hz == cliff_hz else float("nan"),
        "cliff_drop_db": round(cliff_drop, 1),
        "hf_ratio_db": round(band_db(freqs, db, hf_lo, nyq) - band_db(freqs, db, 10000.0, 16000.0), 1),
    }
    return m, None


CSV_FIELDS = [
    "archivo", "tier", "origen", "decision", "motivo",
    "codec", "sample_rate", "channels", "duration_s",
    "declared_bits", "real_bits", "lossy_encoder_tags", "converter_tags",
    "ref_db", "cliff_hz", "cliff_drop_db", "hf_ratio_db",
]


def main():
    if hasattr(sys.stdout, "reconfigure"):
        try:
            sys.stdout.reconfigure(encoding="utf-8", errors="replace")
        except Exception:
            pass
    ap = argparse.ArgumentParser(description="Auditoría de procedencia independiente del motor")
    ap.add_argument("--input", "-i", required=True, type=Path, help="Archivo o carpeta a auditar")
    ap.add_argument("--tier", "-t", required=True, choices=["A1", "A2", "B"],
                    help="A1 rip propio con AccurateRip o entrega del creador documentada; "
                         "A2 compra directa al artista; B tienda/pool/netlabel/amigo")
    ap.add_argument("--source", "-s", required=True,
                    help="Descripción documental del origen (texto libre, va al CSV)")
    ap.add_argument("--out", "-o", required=True, type=Path, help="CSV de salida (se añade si existe)")
    args = ap.parse_args()

    targets = []
    if args.input.is_dir():
        for root, _, files in os.walk(args.input):
            for f in files:
                p = Path(root) / f
                if p.suffix.lower() in AUDIO_EXT:
                    targets.append(p)
        targets.sort()
    elif args.input.is_file():
        targets = [args.input]
    else:
        print(f"No existe: {args.input}")
        sys.exit(1)

    if not targets:
        print("No se encontraron archivos de audio.")
        sys.exit(1)

    print(f"Auditando {len(targets)} archivo(s) · tier {args.tier}")
    print(f"Umbrales pre-registrados: {UMBRALES}\n")

    nuevo = not args.out.exists()
    counts = {}
    with open(args.out, "a", newline="", encoding="utf-8") as fh:
        w = csv.DictWriter(fh, fieldnames=CSV_FIELDS)
        if nuevo:
            w.writeheader()
        for p in targets:
            m, err = measure(p)
            if m is None:
                row = {k: "" for k in CSV_FIELDS}
                row.update({"archivo": str(p), "tier": args.tier, "origen": args.source,
                            "decision": "unreadable", "motivo": err})
            else:
                dec, motivo = decide(args.tier, m)
                row = {k: "" for k in CSV_FIELDS}
                row.update({"archivo": str(p), "tier": args.tier, "origen": args.source,
                            "decision": dec, "motivo": motivo})
                row.update({k: v for k, v in m.items() if k in CSV_FIELDS})
            w.writerow(row)
            counts[row["decision"]] = counts.get(row["decision"], 0) + 1
            print(f"  {row['decision']:<19} {Path(row['archivo']).name}")
            if row["motivo"]:
                print(f"  {'':<19} └─ {row['motivo']}")

    print(f"\nResumen: " + " · ".join(f"{k}={v}" for k, v in sorted(counts.items())))
    limpios = counts.get("lossless", 0) + counts.get("lossless_flag", 0)
    print(f"Negativos admitidos al estrato de certificacion: {limpios}"
          + ("" if args.tier != "B" else "   (tier B nunca entra al estrato)"))
    print(f"CSV: {args.out.resolve()}")


if __name__ == "__main__":
    main()

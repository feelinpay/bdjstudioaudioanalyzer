import subprocess, json
import numpy as np
from pathlib import Path

suspects = [
    ('standard_01_riding_alone', 'Bad Panda Records (Netlabel)'),
    ('standard_04_nightwalker', 'DWK217 (Netlabel)'),
    ('standard_07_distelfink', 'MARAV005A (Netlabel)'),
    ('stress_acoustic_09_built_from_sticks', 'DDW001 (Netlabel)'),
    ('stress_ambient_07_chuzausen', 'slc36 (Netlabel)'),
    ('stress_ambient_08_paranoid', 'saw01x2 (Netlabel)'),
]

print("=== AUDITORIA INDEPENDIENTE DE PROCEDENCIA Y ESPECTRO ===\n")

for stem, src in suspects:
    p = Path("tools/corpus_test/pilot_masters") / f"{stem}.flac"
    
    probe_cmd = ["ffprobe", "-v", "error", "-show_streams", "-show_format", "-of", "json", str(p)]
    pr = json.loads(subprocess.run(probe_cmd, capture_output=True, text=True).stdout)
    fmt = pr.get("format", {})
    tags = fmt.get("tags", {})
    encoder = tags.get("encoder", tags.get("ENCODER", "None"))
    
    res = subprocess.run(["./engine/target/release/bdja_cli.exe", "analyze", str(p), "--json"], capture_output=True)
    rep = json.loads(res.stdout.decode("utf-8", errors="replace"))
    
    bw = rep["effective_bandwidth_hz"]
    slope = rep["cutoff_slope_db_oct"]
    kind = rep["cutoff_kind"]
    llr = rep["score_llr"]
    spec = rep["average_spectrum_db"]
    
    sr = rep["facts"]["sample_rate"]
    nyquist = sr / 2
    bin_hz = nyquist / 256
    
    bin_16k = int(16000 / bin_hz)
    bin_18k = int(18000 / bin_hz)
    bin_20k = int(20000 / bin_hz)
    
    p_16_18 = float(np.mean(spec[bin_16k:bin_18k])) if bin_18k < len(spec) else -99.0
    p_18_20 = float(np.mean(spec[bin_18k:bin_20k])) if bin_20k < len(spec) else -99.0
    p_above_20 = float(np.mean(spec[bin_20k:])) if bin_20k < len(spec) else -99.0
    
    print(f"Pista: {stem}")
    print(f"  Origen: {src} | Encoder tag: {encoder}")
    print(f"  Frecuencia: {sr} Hz | Bits: {rep['facts']['bit_depth']}")
    print(f"  LLR: {llr:.2f} | Tipo Corte: {kind} en {bw} Hz | Pendiente: {slope:.1f} dB/oct")
    print(f"  Potencia media dB: 16-18k: {p_16_18:.1f} dB | 18-20k: {p_18_20:.1f} dB | >20k: {p_above_20:.1f} dB")
    evs = [f"{e['code']}({e['llr']:+.1f})" for e in rep["evidences"] if e["applicable"] and e["llr"] != 0]
    print(f"  Evidencias activas: {evs}")
    print()

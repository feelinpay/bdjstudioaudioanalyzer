# BDJ Studio Audio Analyzer · Metodología R-2.3
## Protocolo de Auditoría de Procedencia

> **Propósito**: Decide, con criterio fijado de antemano y sin consultar al motor, qué archivos entran en la clase negativa del corpus. Es lo que hace que la cifra de FPR sea auditable por un tercero en lugar de una afirmación del autor.

---

## 1. El Principio Fundamental: La medición no puede elegir los negativos

Si excluyes de la clase negativa los archivos que «parecen transcodes», estás filtrando el conjunto hacia lo que el motor encuentra fácil, y el FPR que midas después no significa nada. Da igual que uses `bdja_cli` o una herramienta aparte: si la medición decide, la circularidad está ahí.

Este protocolo lo evita con dos reglas duras:

* **Regla 1 · La procedencia manda**:
  Un archivo de procedencia **A1** se admite como negativo **siempre**, incluso si la medición grita que es un transcode. Se marca con un aviso (`lossless_flag`) y se queda en la clase negativa. Si el motor luego lo condena, cuenta como un **falso positivo real**. Es la única forma de que el número siga siendo honesto.

* **Regla 2 · Apartar no es reclasificar**:
  En **A2**, una medición sospechosa **aparta** el archivo (`provenance_unknown`), dejándolo fuera de ambas clases (y se reporta cuántos). No lo convierte en positivo. Solo la prueba documental —una etiqueta escrita por un codificador con pérdida— o el acuerdo de dos mediciones independientes pueden afirmar que un A2 es un transcode (`lossy_confirmed`).

---

## 2. Paso 1: Clasificar la Procedencia antes de Medir Nada

El tier se asigna por documentación previa, no por cómo suena ni por lo que muestre el espectro. Se asigna antes de ejecutar la herramienta.

| Tier | Qué es | Riesgo Residual | ¿Certifica? |
| :--- | :--- | :--- | :---: |
| **A1** | Rip de CD hecho por ti con EAC o XLD y coincidencia AccurateRip. O entrega del creador con cadena documentada (DAW, bounce, samples). | Que el CD sea una reedición masterizada desde una fuente con pérdida. Evitar reediciones y recopilatorios; preferir prensados originales. | **Sí** |
| **A2** | Compra directa al artista en WAV o FLAC (Bandcamp y equivalentes) donde el que subió el archivo es el autor, sin cadena documentada. | Que el productor haya exportado desde un proyecto con samples comprimidos, o bounce de un MP3. Pasa de verdad. | **Sí** |
| **B** | Tienda que no declara la cadena del máster, pool de DJs, netlabel, entrega de un amigo, descarga sin recibo. | Alto. 5 de los 20 archivos del piloto inicial salieron de aquí y eran transcodes reales. | **No, nunca** |

### Consecuencia para el Sourcing
Los 300 negativos tienen que ser **A1 o A2**. El material de tier B no cuenta para la certificación por limpio que salga: sirve para encontrar fraudes reales del mercado —que son el mejor caso de estudio del producto— y para el estrato exploratorio, no para la cifra de certificación.

---

## 3. Paso 2: Magnitudes Medidas por la Herramienta (Desacopladas del Motor)

La herramienta `tools/provenance_audit/audit_provenance.py` no importa ni invoca ninguna parte del motor: es independiente por construcción (`ffmpeg` + `numpy`), y donde mide lo mismo lo mide con otro método a propósito, para que no sea una reimplementación del mismo criterio.

| Magnitud | Cómo se mide aquí | En qué se diferencia del motor |
| :--- | :--- | :--- |
| **Corte sostenido** | Ventana absoluta de 1 kHz antes del candidato contra 1 kHz después, más la media de todo lo que queda por encima. Barrido de 3 kHz a Nyquist en pasos de 100 Hz. | El motor usa una ventana relativa (±8% de la frecuencia), que hace el ancho en octavas constante. Aquí es absoluta, por lo que la cifra no es la misma magnitud. |
| **Colapso de banda extrema** | Nivel medio en [19 kHz, Nyquist] menos el nivel medio en [10, 16] kHz. | El motor extrapola una tendencia log-lineal de 10-17 kHz. Aquí es una resta directa de bandas, sin modelo. |
| **Bits reales** | OR binario de todas las muestras: cuántos bits bajos son siempre cero. | El motor usa `zero_lsb_ratio` por proporción. Aquí es exacto y determinista. |
| **Etiquetas de encoder** | Metadatos vía `ffprobe`, separados estrictamente en dos listas (Neutras vs. Lossy). | Registra conversores en su propia columna sin contarlos como evidencia. Solo son prueba documental las de códecs con pérdida. |

> **Nota v2 sobre Periodicidad de Trama**: La medida por autocorrelación de envolvente HF se retiró formalmente. Al contrastarla con controles lossless sintéticos resultó no fiable (un bajo saturado cerca de 76.6 Hz = 44100/576 generaba falsos picos sin códec alguno, y el paso banda previo saturaba sobre másters limpios). Detectar la rejilla MDCT exige proyectar la misma ventana y solape que el códec; se prefiere no medirla a medirla mal.

### Las Etiquetas Neutras no Prueban Nada
`Lavf`, `Lavc`, `dBpoweramp`, `xrecode`, `foobar`, `Audition`, `SoX`, `WaveLab` son conversores: un WAV legítimo convertido con `ffmpeg` lleva `Lavf` exactamente igual que un fake. La herramienta las registra en su propia columna y no las cuenta como evidencia. Solo son prueba documental las que únicamente puede haber escrito un codificador con pérdida: `LAME`, `iTunNORM`, `iTunSMPB`, `Nero`, `FhG`/`Fraunhofer`, `GoGo`, `Xing`, `Fastenc`, `Helix`, `Shine`.

---

## 4. Paso 3: Regla de Decisión Pre-registrada

### Umbrales Pre-registrados (v2)
* **Decisivas** (una basta):
  - Corte sostenido < 21 kHz: caída >= 25.0 dB.
  - Etiqueta de codificador con pérdida: presencia confirmada.
* **Corroborantes** (hacen falta dos):
  - Colapso de banda extrema: <= -38.0 dB.
  - Bits inflados (declarados menos reales): >= 4 bits.

### Matriz de Decisión por Tier

| Tier | Sin evidencia | Solo 1 Corroborante | Decisiva, o >= 2 Corroborantes |
| :---: | :---: | :---: | :---: |
| **A1** | `lossless` | `lossless_flag` | `lossless_flag` (admitido con aviso) |
| **A2** | `lossless` | `provenance_unknown` (apartado) | `lossy_confirmed` si hay tag o >= 2 corrob. / `provenance_unknown` si solo corte |
| **B** | `exploratory_clean` | `provenance_unknown` | `lossy_confirmed` (fraude real) |

### Significado de los Desenlaces

| Desenlace | Qué significa | Cuenta para |
| :--- | :--- | :--- |
| **`lossless`** | Negativo limpio, sin ninguna firma detectada. | **Los 300** de certificación |
| **`lossless_flag`** | Negativo A1 con firma detectada. Se admite igualmente. Si el motor lo condena, es un FP real. | **Los 300** de certificación |
| **`provenance_unknown`** | Apartado. Ni negativo ni positivo. Se reporta cuántos y por qué. | Nada. Se reporta en el informe. |
| **`lossy_confirmed`** | Transcode de primera generación con procedencia documentada. Fraude real. | Positivos · Sección [5] |
| **`exploratory_clean`** | Tier B sin firmas. No certifica porque la procedencia no lo permite. | Estrato exploratorio |

> **Estrato de Certificación**: Es la suma de `lossless` + `lossless_flag` de los tiers **A1 y A2**. Esa es la cuenta que tiene que llegar a **300**, y la única sobre la que se calcula la cota 3/n.

---

## 5. Paso 4: Ejecución Práctica

Una pasada por tier, acumulando en el mismo CSV de salida:

```bash
# 1. Rips propios verificados con AccurateRip
python tools/provenance_audit/audit_provenance.py     -i "D:\Musica\Rips_CD" -t A1     -s "rip EAC propio, AccurateRip coincidencia 2/2"     -o auditoria_procedencia.csv

# 2. Compras directas al artista (Bandcamp, etc.)
python tools/provenance_audit/audit_provenance.py     -i "D:\Musica\Bandcamp" -t A2     -s "compra Bandcamp directa al artista, FLAC original"     -o auditoria_procedencia.csv

# 3. Material de pool, tiendas sin cadena declarada o amigos (no certifica)
python tools/provenance_audit/audit_provenance.py     -i "D:\Musica\Pool" -t B     -s "descarga de pool, cadena no declarada"     -o auditoria_procedencia.csv
```

---

## 6. Límites y Reglas de Invalidación

### Límites Conocidos de la Herramienta
- **Punto ciego en AAC >= 256 kbps**: A esa tasa el encoder no deja un corte sostenido medible ni colapso de banda extrema ni periodicidad de trama. El diseño de tiers no depende de esto: un AAC 256 en A1 se admite como negativo y si el motor lo condena es un FP real.
- **Ventana temporal**: Mide los primeros 180 segundos de audio. Requiere al menos 5 s para espectro y 4.5 s para autocorrelación.
- **Retardos MDCT**: La atribución a un retardo específico (576 vs 1024) es tentativa; lo informativo es la presencia del pico periódico.

### Reglas de Invalidación (Reauditar desde cero si se viola alguna)
1. **Tocar cualquier umbral de `UMBRALES` a mitad de la recolección**.
2. **Cambiar el tier de un archivo después de ver su medición**.
3. **Borrar filas del CSV**.
4. **Usar el veredicto del motor en cualquier punto de la decisión**.

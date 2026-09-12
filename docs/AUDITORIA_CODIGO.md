# BDJ Studio Audio Analyzer — Auditoría de código v4.0

> Cuarta pasada · 12 de septiembre de 2026 · contrastada con PLAN_ARQUITECTURA.md v1.1
> El motor ya es correcto: ningún transcode sale certificado. Pendiente: la fusión de evidencias nunca condena.

BDJ Studio · Auditoría técnica · cuarta pasada

# Auditoría del Audio Analyzer

El motor ya es correcto: nada con pérdida sale certificado como auténtico, y los tres arreglos que pedí están implementados tal cual, con tests que los fijan. Lo que queda ya no es corrección, es calibración — y hay un problema estructural en la fusión de evidencias que hace que el motor *nunca condene*. Un MP3 320 con un corte vertical a 20,7 kHz sale «Inconcluso».

**Revisado** 12 sep 2026 · v4.0 **Cambios** 6 archivos · +9,7 KB · 3 tests nuevos **Corrección** resuelta **Calibración** es ahora el cuello de botella

## 0. ¿Ya quedó todo?

**El motor ya está correcto. Todo, no.** La diferencia importa, porque cambió la naturaleza de lo que falta: hasta esta pasada el problema era que el motor se equivocaba; ahora el motor mide bien y lo que falla es cuánto peso le da a lo que mide.

Certificar un transcodeYa no pasaningún archivo con pérdida alcanza «Lossless verificado» ni «Probablemente lossless»

Certificar un master realFuncionaun lossless de banda completa llega a «Lossless verificado»

Condenar un transcodeNunca ocurreel mejor caso es «Sospechoso», y solo con MP3 128

Los tres arreglos de la auditoría anterior están hechos, y hechos exactamente como correspondía:

- **`CutoffKind`** en `bdja_core` con las tres variantes, E01 decidiendo por rama y no por frecuencia, y los umbrales expresados como fracción de Nyquist — con lo que además quedó resuelto de paso el problema de los archivos de 48 y 96 kHz.
- **E02 siempre aplicable** ante `BrickwallCutoff` y `NaturalRolloff`, con +0,8 como mínimo cuando hay corte. Ya no se apaga la evidencia justo cuando hace falta.
- **El umbral de entrada del detector relajado a `ref_level − 70`**, que es el valor que validé. Reproducido: el MP3 128 sobre mezclas de −12 y −15 dB/oct ahora se detecta, y ninguno de los cinco casos lossless produce un corte falso.
- **`panic = "unwind"` + `catch_unwind`** en el bucle de escaneo. Elegiste la opción B del IPC y está bien implementada: un archivo corrupto ya no interrumpe un escaneo de 40 000 pistas.

Y tres tests nuevos que fijan precisamente los casos que fallaban: `test_dsp_mp3_320_transcode_is_convicted`, `test_dsp_dark_mix_mp3_128_detected` y `test_dsp_dark_mix_lossless_exonerated`. Eso es exactamente la práctica que pedía: un test por caso de negocio.

## 01. Estado de todo lo señalado hasta ahora

| ID        | Hallazgo                                              | Estado                                                                                                         |
|-----------|-------------------------------------------------------|----------------------------------------------------------------------------------------------------------------|
| P0-1…P0-8 | Los ocho bloqueantes originales                       | \*\*Resueltos\*\*                                                                                              |
| P0-9      | El MP3 320 salía «Lossless verificado»                | \*\*Resuelto\*\* · E01 por rama. El test lo fija con `assert_ne!` contra `LosslessVerified` y `LikelyLossless` |
| §03 v3    | Las mezclas oscuras escapaban                         | \*\*Resuelto\*\* · umbral a `ref−70`                                                                           |
| N-1       | Sin protección ante archivos malformados              | \*\*Casi\*\* · `catch_unwind` en el escaneo, pero ver §03                                                      |
| —         | Umbrales atados a 44,1 kHz                            | \*\*Resuelto\*\* · ratio sobre Nyquist                                                                         |
| N-4       | Sin `calibrate` en el CLI                             | \*\*Abierto\*\* · es ahora el cuello de botella real                                                           |
| N-5       | Recorrido en dos fases sin progreso                   | \*\*Abierto\*\*                                                                                                |
| N-6       | Cola de reportes sin tope                             | \*\*Abierto\*\*                                                                                                |
| N-8       | Sin reanudación ni `user_version`                     | \*\*Abierto\*\*                                                                                                |
| —         | LUFS sin K-weighting, métricas sobre 0,74 s y en mono | \*\*Abierto\*\* · `quality.rs` sin tocar en cuatro pasadas                                                     |
| —         | E05, E06, E09, E14                                    | \*\*Abierto\*\* · sin cambios                                                                                  |
| —         | CI, firma EV, notarización, fuzzing                   | \*\*Abierto\*\*                                                                                                |

## 02. El motor mide bien pero no condena

Reproduje la cadena completa —detector v4, E01 por rama, E02, E04 y E08— sobre las mismas señales de siempre. Esto es lo que sale hoy:

| Señal                           | Rama           | Hz     | E01  | E02  | E04  | Score | Veredicto                      |
|---------------------------------|----------------|--------|------|------|------|-------|--------------------------------|
| lossless −6 dB/oct              | FullSpectrum   | 22 050 | −1,8 | 0,0  | −1,4 | −4,0  | \*\*Lossless verificado\*\*    |
| lossless −9 dB/oct              | FullSpectrum   | 22 050 | −1,8 | 0,0  | −1,4 | −4,0  | \*\*Lossless verificado\*\*    |
| lossless −12 dB/oct             | NaturalRolloff | 14 271 | 0,0  | −1,0 | −1,4 | −3,2  | \*\*Probablemente lossless\*\* |
| lossless −15 dB/oct             | NaturalRolloff | 8 376  | 0,0  | −1,0 | −1,4 | −3,2  | \*\*Probablemente lossless\*\* |
| **MP3 128** (corte 16 kHz)      | Brickwall      | 16 290 | +2,2 | +1,5 | −1,4 | +1,5  | \*\*Sospechoso\*\*             |
| **MP3 192** (corte 19 kHz)      | Brickwall      | 19 251 | +1,2 | +1,5 | −1,4 | +0,5  | \*\*Inconcluso\*\*             |
| **MP3 320** (corte 20,5 kHz)    | Brickwall      | 20 731 | +1,2 | +1,5 | −1,4 | +0,5  | \*\*Inconcluso\*\*             |
| **AAC 256** (corte 19,5 kHz)    | Brickwall      | 19 547 | +1,2 | +1,5 | −1,4 | +0,5  | \*\*Inconcluso\*\*             |
| MP3 128 sobre mezcla oscura −12 | Brickwall      | 15 994 | +2,2 | +1,5 | −1,4 | +1,5  | \*\*Sospechoso\*\*             |
| MP3 128 sobre mezcla oscura −20 | NaturalRolloff | 4 920  | 0,0  | −1,0 | −1,4 | −3,2  | \*\*Probablemente lossless\*\* |

Mira la columna E04 y se ve el problema de golpe: **vale −1,4 en todas las filas, incluidas las de transcode**.

#### \*\*P1-1\*\* La evidencia exoneradora se aplica aunque haya una detección positiva \`bdja_dsp/src/pipeline.rs · E04 y E08\`

E04 mide huecos psicoacústicos *por debajo* del corte, entre 10 kHz y el `cutoff_bin`. En un transcode de bitrate alto esa banda está intacta por definición: el encoder no dejó huecos, solo cortó arriba. Así que E04 no encuentra nada y concluye «continuidad espectral natural», restando **−1,4**. Lo mismo hace E08 con el piso de ruido: el decodificado de un MP3 tiene un piso perfectamente coherente, así que resta **−0,8**.

Resultado: un corte vertical de 400 dB/oct a 20,7 kHz aporta +2,7 entre E01 y E02, y las dos evidencias exoneradoras le quitan 2,2. Queda +0,5, que es *Inconcluso*. **Por construcción, el motor no puede condenar nada por encima de 128 kbps**, y ni siquiera el 128 pasa de *Sospechoso*: para llegar a *Probable transcode* hace falta ≥ +4,0 y una evidencia fuerte (E04, E05, E07 o E13), y ninguna de esas cuatro se activa en un transcode limpio.

**Arreglo**La ausencia de evidencia no es evidencia de inocencia. Una evidencia solo debe exonerar cuando su medición es *informativa*: si `cutoff_kind == BrickwallCutoff`, E04 y E08 deben quedar en 0,0 y marcarse `applicable: false` con el texto «no concluyente: la banda analizada está por debajo del corte detectado». Y simétricamente, cuando no se pudo medir nada (la fila de −20 dB/oct, donde no hay energía que analizar), tampoco deben exonerar: ahí el resultado honesto es *Inconcluso*, no *Probablemente lossless*.  
  
Con ese único cambio, y sin tocar ningún peso: MP3 128 pasa a +3,7 (*Sospechoso* alto), y MP3 192/320/AAC 256 pasan a +2,7 (*Sospechoso*). Para llegar a *Probable transcode* hace falta además que la puerta de evidencia fuerte admita «corte brickwall verificado» como quinta evidencia fuerte — que es lo que es: un filtro digital de 400 dB/oct no existe en la naturaleza.

### Dos observaciones menores de calibración

- **El tramo de E01 entre 0,86 y 0,96 de Nyquist es demasiado ancho.** Cubre de 19,0 a 21,2 kHz con un único +1,2, así que MP3 192 y MP3 320 puntúan igual. Y un corte a 19,25 kHz cae en ese tramo cuando por frecuencia le correspondería el +1,6 de «192 kbps». Con el corpus esto se ajusta solo; de momento yo partiría ese tramo en dos.
- **Una mezcla oscura legítima nunca podrá ser «Lossless verificado»** (máximo −3,2 en la rama `NaturalRolloff`). Me parece la decisión correcta y conservadora —de un máster band-limited no se puede certificar el origen— pero conviene que sea deliberada y que el texto de la UI lo diga: «sin corte artificial; no verificable por falta de contenido en agudos».

## 03. Cabos sueltos de esta pasada

| Sev        | Asunto                                            | Detalle                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
|------------|---------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| \*\*P1\*\* | El `catch_unwind` solo cubre el escaneo           | Está dentro de `scan_collection`, así que protege el escaneo masivo. Pero `analyze_file` y `analyze_batch` del FFI llaman a `analyze_single_file` directamente: arrastrar **un** archivo corrupto a la ventana sigue sin protección, y un *panic* cruzando la frontera FFI es el peor sitio donde puede ocurrir. El arreglo es mover el `catch_unwind` dentro de `analyze_single_file`: una línea y quedan cubiertos todos los caminos, presentes y futuros. |
| \*\*P2\*\* | `bdja_ipc` y `bdja_worker` siguen en el workspace | Elegiste `catch_unwind`, que era una de las dos opciones válidas — pero la otra mitad de la decisión era retirar los dos crates. Siguen ahí, sin que nadie los use, y `build_native.ps1` sigue metiendo `bdja_worker.exe` en la carpeta de la app. Son ~8 KB de fuente y un binario en el instalador que no se ejecuta nunca.                                                                                                                                |
| \*\*P1\*\* | El token de capacidad, igual que antes            | El `hwid` lo sigue aportando el llamante dentro del token y el motor no lo deriva por su cuenta; el salt sigue en texto plano en los dos lados; la comparación de digests sigue sin ser en tiempo constante.                                                                                                                                                                                                                                                 |
| \*\*P2\*\* | Código muerto                                     | `analyze_file_quick` sigue siendo un alias literal. `scan_directory` (la vieja secuencial) sigue sin llamarse. El `blake3` se calcula y se guarda pero no se usa para el dedupe, que es casi gratis y resuelve un problema real de todo DJ.                                                                                                                                                                                                                  |

## 04. Qué falta para producción

T1 — Que el motor pueda condenar2-3 horas

El arreglo del §02: E04 y E08 no exoneran cuando `cutoff_kind == BrickwallCutoff` ni cuando no hubo nada que medir; el corte brickwall verificado entra como quinta evidencia fuerte; el tramo ancho de E01 partido en dos. Y el `catch_unwind` movido dentro de `analyze_single_file`.

**Criterio de aceptación**MP3 128, 192, 320 y AAC 256 convertidos a WAV salen todos como *Sospechoso* o *Probable transcode*, nunca *Inconcluso*; los cinco casos lossless mantienen su veredicto actual; y arrastrar un archivo corrupto suelto no tumba la app.

T2 — Corpus y calibración3-4 días

`bdja_cli calibrate` y `validate`, matriz de transcodes con tus masters, pesos ajustados contra datos reales y los gates en CI. Después del §02 esto pasa a ser **lo único que separa la app de estar terminada de verdad**: todos los números que quedan por afinar son pesos, y los pesos se ajustan con datos, no a mano. Es lo único que no puedo empezar sin ti.

**Criterio de aceptación**FPR \< 1 % sobre lossless genuino y recall \> 95 % en MP3 ≤ 320 kbps, medidos por un comando reproducible.

T3 — Escaneo y datos de grado producción2 días

Walk y análisis solapados con progreso desde el primer segundo, tope en la cola de reportes, dedupe con el blake3 que ya se calcula, `scan_job` con checkpoints, `user_version`, y los errores de decodificación visibles en la UI.

**Criterio de aceptación**50 000 archivos con la app matada dos veces: reanuda sin repetir ni perder, RSS por debajo de 250 MB, barra en movimiento desde el primer segundo.

T4 — Las métricas que promete la app1-2 días

`ebur128` para LUFS y true peak conformes, sobre la pista completa y por canal. E05 por bloque contiguo, E06 normalizado, E09 por entropía de LSB, E14 sobre evidencias. Limpieza del código muerto y retirada de `bdja_ipc`.

**Criterio de aceptación**El LUFS y el true peak que muestra la app coinciden con los de un DAW dentro de ±0,3 dB sobre cinco archivos de referencia. Hoy puede desviarse varios dB, y es el dato que un productor contrasta primero.

T5 — Release3-4 días

Ficha técnica exportable, i18n es/en, CI mínima (`fmt`, `clippy -D warnings`, `cargo test`, `flutter test`), firma EV de Windows, Developer ID con notarización, empaquetado de macOS, `cargo-deny`/`audit`/SBOM y `cargo-fuzz`.

**Criterio de aceptación**Instalador firmado que arranca limpio en un Windows 10 nuevo y en un macOS 12 sin Xcode, sin avisos de SmartScreen ni Gatekeeper.

**Total: 9-12 días**, y estás en torno al **90 %**. Pero el reparto sigue siendo lo importante: T1 son dos o tres horas y es lo que convierte un motor que mide bien en un motor que *sirve*.

## 05. Siguiente paso

Cuatro pasadas y el patrón ha sido consistente: cada tanda resuelve lo señalado y deja el problema una capa más adentro. Eso no es mala señal, es lo normal cuando se construye un motor de decisión — y ahora el problema está en la última capa, la de los pesos, que es donde debe estar.

Mi recomendación:

1.  **T1 ya** — dos o tres horas, cuatro cambios, con sus tests. Sin esto la app detecta transcodes y no se atreve a decirlo.
2.  **Después, el corpus.** Es el único punto donde el proyecto depende de ti y llevo cuatro pasadas pidiéndolo: 200-400 masters lossless tuyos, variados, incluyendo a propósito mezclas oscuras y material band-limited legítimo. Con eso, `calibrate` ajusta todos los pesos que hoy son criterio y no medida, y los porcentajes de confianza que ve el DJ pasan a significar algo.
3.  **Y luego T4 antes que T3**, si me dejas reordenar: el LUFS y el true peak son lo primero que un productor va a contrastar con su DAW, y si no cuadran pierde la confianza en todo el resto.

Dime si arranco con T1.


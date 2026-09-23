# BDJ Studio Audio Analyzer — Pendientes a nivel de código

> B-1 a B-16 cerrados y verificados en el código. **No queda ningún bloqueante interno.**
> 1 bloqueante externo: corpus real (100-200 másters propios) · 3 números sin medir · 7 menores · hoja de ruta competitiva en §08.

BDJ Studio Audio Analyzer · pendientes a nivel de código

# Pendientes del Audio Analyzer

Actualizado con el desglose forense tras las correcciones de B-4 (E02 proporcional a pendiente), B-5 (detector de corte desacoplado y extrapolación log-lineal), B-6 (desactivación de E02 en BrickwallCutoff para evitar doble contabilidad) y B-7 (separación en dos niveles de extensiones analizables vs reconocidas no soportadas). En la última validación forense sobre 56 pistas: **0 FP (FPR = 0.00%)**, **25 TP**, **27 TN**, **2 FN**, **Recall = 92.59%**, **Exactitud Global = 96.30%**.

**Bloqueantes internos** 0 **Bloqueante externo** 1 (corpus real) **Números sin medir** 3 **Menores** 7 **Hoja de ruta competitiva** 5 fases (§08)

El motor está cerrado. Lo que sigue no es corregir: es medir (§03) y después competir (§08).

## 0. Lo que ya está cerrado

Para no volver sobre esto: todo lo siguiente quedó verificado en el código y no requiere más trabajo.

Enum `Codec` con mapeo explícito · *seek* a 12 segmentos con respaldo · espectro real por media de potencia (B-8) de punta a punta · `CutoffKind` con las tres ramas y E01 decidiendo por rama · umbrales como fracción de Nyquist · E02 desactivado en `BrickwallCutoff` para evitar duplicar evidencia (B-6) y activo con escala gradual en `NaturalRolloff` (B-4) · eliminación de `.max(45.0)` redundante en `spectrum.rs` · detección de corte desacoplada con ventana previa y extrapolación log-lineal de agudos 10-17 kHz frente a 19-22 kHz (B-5) · soporte de biblioteca unificado en `AUDIO_EXTENSIONS` con sonda previa (B-7b) · tipado explícito `DecodeError::Unsupported` sin reportar formatos no soportados como corruptos (B-14) · `ENGINE_REV = 2` para invalidación automática de cachés previas (B-12) · creación directa de esquema v3 en bases nuevas y migraciones con propagación estricta de errores reales (B-13) · E04 y E08 no exoneran cuando hay corte · E01 como evidencia fuerte · E05 fuera de evidencias fuertes e inaplicable ante `FullSpectrum` (B-1) · regla rígida de 45 dB/oct revertida (B-2) · E11 ante cualquier corte · diagnóstico de remuestreo con candidatos de 44,1 y 48 kHz · `catch_unwind` dentro de `analyze_single_file` · clipping por canal en las dos rutas · bit depth por `zero_lsb_ratio` · E06 normalizado · E05 por ventana contigua · LUFS con K-weighting y *gating* BS.1770-4 · piso de ruido por percentil · HWID derivado nativamente, fail-closed y comparación en tiempo constante · sin fallback silencioso a base en memoria · `user_version` y migraciones · tabla `scan_job` · SQL parametrizado con `ESCAPE` · caché con `mtime` y blake3 · export paginado · tope de 250 en `pending_reports` · reportes de error por archivo · CLI con `validate` y `calibrate` en tres modalidades y reporte en dos fronteras (B-9) · `bdja_ipc` y `bdja_worker` eliminados · discriminación de errores de decodificación por el mensaje del demuxer, sin lista de extensiones cableada: contenedor irreconocible frente a códec interno no soportado, de modo que un WAV con etiqueta `WAVE_FORMAT_MPEGLAYER3` (0x0055) se reporta como códec no soportado y nunca como archivo corrupto (B-15, B-16) · guarda `table_exists` para bases preexistentes sin versionar · `ENGINE_REV` atado a un hash FNV-1a de `ENGINE_WEIGHTS_SPEC`, de modo que cambiar un peso sin subir la revisión rompe el test · CI con `fmt`, `clippy -D warnings`, `test`, `flutter analyze` y `flutter test`.

## 01. E05 contamina el veredicto del material legítimo

#### \*\*B-1\*\* Una evidencia fuerte se dispara sobre espectro continuo \`bdja_dsp/src/temporal.rs · bdja_dsp/src/pipeline.rs\`

Las 19 pistas legítimas que salen *Inconcluso* comparten la misma aritmética exacta:

    E01  -1.80   "Espectro completo hasta 22050 Hz sin corte artificial"   (FullSpectrum)
    E04  -1.40   "Continuidad espectral natural (0.0% huecos)"
    E05  +2.20   "Estructura de tramas MDCT: periodicidad de 576 muestras (MP3)"   value = 0.22065
                 ─────
    score        -1.00  →  banda muerta (-1.50, +1.50)  →  Inconclusive

El clasificador está bien: `-1.00 > -1.50`, así que no alcanza *LikelyLossless* y cae en *Inconcluso*. La banda muerta es una decisión de producto correcta. **El defecto es que E05 aporta +2,20 sobre un archivo cuyo espectro es continuo hasta Nyquist.** Una rejilla de tramas MDCT no puede coexistir con un espectro pleno sin corte: si el encoder hubiera trabajado el archivo, habría dejado un borde.

#### Arreglo, en dos partes

**Estructural — esto es lo que hay que hacer:** E05 no aplica cuando `cutoff_kind == FullSpectrum`. `applicable: false` y LLR 0,0, con el texto «no concluyente: no hay corte que acompañe una rejilla de compresión». Es la misma corrección que ya se aplicó a E04 y E08, por la misma razón lógica: una evidencia solo cuenta cuando su medición es informativa en ese contexto.

**El umbral — ojo con el número propuesto.** Subir `best_peak_mp3 > 0.08` a `> 0.40` no resuelve el problema: `genuine_lossless_24` midió **0,4612** y seguiría condenando. Y 0,40 es otro número elegido sin medir, que es exactamente cómo llegamos hasta aquí. El umbral hay que sacarlo de la distribución: `bdja_cli analyze --json` ya devuelve `E05.value`, así que con el corpus bueno se saca el histograma de las dos clases y el corte sale de ahí. Mientras no exista esa medición, lo prudente es dejar E05 fuera de las evidencias fuertes y bajarle el peso a informativo.

**Nota sobre la autocorrelación**Hay una razón por la que E05 se dispara justo en este corpus: las señales sintéticas construidas como suma de senoides en kilohercios enteros tienen periodicidades propias que pueden producir picos de autocorrelación en lags arbitrarios, y el *lag* de 576 no está exento. El 0,22 medido es compatible con eso. Con música real el fondo de autocorrelación es muy distinto, así que cualquier umbral que se fije sobre este corpus estará mal calibrado. Esto encadena con §03.

## 02. La regla de 45 dB/oct

#### \*\*B-2\*\* Forzar `BrickwallCutoff` por pendiente sola \`bdja_dsp/src/spectrum.rs\`

Se introdujo en caliente para un solo archivo: cualquier pendiente ≥ 45 dB/oct se reclasifica como corte brickwall, sin exigir la comprobación de supresión sostenida posterior que el detector sí aplica en su rama principal.

Hay que revertirla a la forma condicionada: la pendiente por sí sola no distingue un borde de encoder de una caída acústica pronunciada. Si el caso original (`latin_transcode_06`, 105,6 dB/oct que caía en `NaturalRolloff` por quedar en el límite de la ventana de 1 200 Hz) necesita atenderse, el arreglo correcto es en la **ventana de detección**, no en el umbral de pendiente: ampliar el margen de Nyquist o usar una ventana adaptativa para que el barrido no pierda cortes cerca del borde superior. Y verificar el resultado con un test propio para ese archivo.

## 03. El corpus de prueba no sirve para calibrar

#### \*\*B-3\*\* Los 4 falsos positivos son artefactos del generador \`tools/corpus_test/\`

Esto cambia la lectura de la corrida entera, y en la dirección buena para el motor:

- `genuine_lossless_23` y `27`: el generador cortó el bucle de armónicos en 8 320 Hz y escribió ceros absolutos por encima. El archivo tiene físicamente un corte de 113 dB/oct a 8,5 kHz. **El motor leyó bien lo que había en los bytes.**
- `genuine_lossless_24` y `28`: una discontinuidad de fase generó un corte vertical de 152 dB/oct a 20,6 kHz. Mismo caso.

Consecuencia: **el FPR del 50 % de esa corrida no es válido**. Cuatro de las ocho pistas «lossless» clasificadas no eran audio lossless verosímil, eran archivos con un corte digital real generado por error. El motor no tuvo un falso positivo: tuvo una lectura correcta de un archivo mal etiquetado.

Y con §01 en la mano, el corpus tampoco sirve para la otra clase: E05 se dispara sobre las señales sintéticas por su propia periodicidad, así que ni el umbral de E05 ni el `T*` se pueden fijar con este material.

**Lo que hay que hacer**Sustituir el corpus sintético por música real antes de volver a correr `calibrate`. La forma acordada: 100-200 másters lossless propios —con 30-40 de casos de estrés (dub, ambient, minimal, ripeos de vinilo con soplido, analógico de rango limitado, remuestreados a 96 kHz, WAV etiquetados con Serato)— y la clase transcode generada desde *esos mismos* másters con 9 variantes cada uno: CBR 128/192/256/320, VBR V0/V2 y AAC 128/192/256. Con 100 másters son 900 transcodes y el recall sale desglosado por bitrate.  
  
El corpus sintético conviene conservarlo, pero para lo que sí sirve: tests unitarios de regresión con respuesta conocida. Ahí es bueno; como referencia estadística no.

## 04. Confirmado: «Lossless verificado» es inalcanzable

Confirmado en la segunda corrida

La mediana lossless quedó en **−3,20 exactamente** de p0 a p95: E08 no aporta su −0,80 porque el piso de ruido sintético ronda los −55 dBFS y no cruza el umbral de −80. Sobre material continuo, el mejor veredicto posible hoy es \*\*Probablemente lossless\*\*. Decisión acordada: no mover la frontera a ciegas — con másters reales de 16 bits el dither está entre −90 y −96 dBFS, así que hay que observar si E08 dispara de forma consistente para dar el −4,00 o si la distribución aconseja bajar la cota a ≤ −3,00 o subir el peso de E01 ante `FullSpectrum`.

La aritmética del veredicto máximo:

    E01 -1.80  (FullSpectrum)
    E04 -1.40  (sin huecos)
    E08 -0.80  (piso de ruido coherente con dither)
        ─────
        -4.00  →  score_llr <= -4.0  →  LosslessVerified

Sin el −0,80 de E08 el mínimo alcanzable es −3,20, que es *Probablemente lossless* y no *Lossless verificado*. Es decir: una vez arreglado E05, esas 19 pistas pasan a −3,20 y se quedan ahí si E08 no aporta. Conviene comprobar en el JSON de un archivo real si E08 aparece y con qué valor, y si no aparece, revisar por qué (el piso de ruido de un archivo sintético sin dither no baja de −80 dB, así que puede ser otro efecto del corpus).

Dicho de otra forma: el veredicto máximo se alcanza justo en el límite, sumando exactamente −4,00 con las tres evidencias. No hay margen. Cuando haya corpus real, esa frontera es la primera que hay que mirar en la distribución.

### Coste del arreglo de E05: el recall bajó

Tras neutralizar E05 y bajarle el peso a 1,0, la segunda corrida da FPR 0,00 % y 27 de 28 lossless clasificados correctamente — pero el **recall cayó de 85,71 % a 76,92 %**: los falsos negativos pasaron de 4 a 6.

Eso es el precio de haber sacado E05 del conjunto fuerte. Un transcode cuya única evidencia fuerte era E05 ya no puede alcanzar *Probable transcode*, y además pierde 1,2 de score. La corrección de *aplicabilidad* era correcta; el **peso de 1,0 es otro número puesto a mano** y hay que sacarlo del corpus.

**Diagnóstico hecho**Los 6 falsos negativos son **todos** del primer tipo: el detector no clasificó ninguno como `BrickwallCutoff`. Cuatro cayeron en `FullSpectrum` y dos en `NaturalRolloff`. No es un problema de pesos de E05 — es el detector de borde. Ver §04b.  
  
El caso peor de todo el corpus: `latin_transcode_14`, un MP3 320 real, salió **LosslessVerified** con −4,00. La clase de fallo que costó tres iteraciones eliminar reapareció por otra vía: antes era la tabla de umbrales, ahora es que el detector no ve el corte.

## 04b. El detector de borde y el bug de E02

| Archivo (MP3 320)  | Veredicto                      | Score | CutoffKind     | BW     | Pendiente |
|--------------------|--------------------------------|-------|----------------|--------|-----------|
| latin_transcode_14 | \*\*Lossless verificado\*\*    | −4,00 | FullSpectrum   | 22 050 | 0,0       |
| latin_transcode_08 | \*\*Probablemente lossless\*\* | −3,20 | FullSpectrum   | 22 050 | 0,0       |
| latin_transcode_13 | \*\*Probablemente lossless\*\* | −3,10 | FullSpectrum   | 22 050 | 0,0       |
| latin_transcode_26 | \*\*Probablemente lossless\*\* | −2,30 | FullSpectrum   | 22 050 | 0,0       |
| latin_transcode_15 | \*\*Probablemente lossless\*\* | −3,20 | NaturalRolloff | 18 798 | 86,7      |
| latin_transcode_16 | \*\*Probablemente lossless\*\* | −1,50 | NaturalRolloff | 18 276 | 103,5     |

#### \*\*B-4\*\* E02 exonera con −1,0 aunque haya medido 103 dB/oct \`bdja_dsp/src/pipeline.rs · rama NaturalRolloff\`

En la rama `NaturalRolloff`, E02 devuelve un **−1,0 constante** con el texto «roll-off suave de {slope} dB/oct compatible con acústica natural». Con los archivos 15 y 16 eso imprime literalmente *«roll-off suave de 103,5 dB/oct»* y les regala 1,0 de exoneración. Ninguna caída acústica natural pasa de ~20 dB/oct; 86 y 103 son filtros digitales sin discusión.

Es un bug independiente del detector y se arregla sin corpus: **el LLR de E02 debe ser función de la pendiente medida en las dos ramas**, no una constante por rama. Solo por encima de ~40 dB/oct ya deja de ser exonerador. Con eso, los archivos 15 y 16 se mueven ~2,5 puntos y salen de *Probablemente lossless*.

#### \*\*B-5\*\* El detector exige que la caída ocurra dentro de una sola ventana \`bdja_dsp/src/spectrum.rs\`

Los dos modos de fallo tienen la misma raíz. La transición de un filtro de encoder a 320 kbps se extiende 1,5-2 kHz, así que la caída medida entre dos ventanas contiguas de 1 200 Hz sale de 16-17 dB y queda por debajo del `min_drop` de 18. Cuando eso pasa, el flujo cae al chequeo de energía en el extremo superior, y ahí el piso de cuantización del encoder a −65,2 dBFS cumple `top_band_db >= ref_level − 50` por **1,6 dB de margen** y se declara espectro pleno.

**Bajar `min_drop` no es el arreglo.** Es el arreglo frágil: amplía la detección y a la vez sube el riesgo de borde falso en caídas acústicas pronunciadas, y la ventana de 1 200 Hz seguiría siendo demasiado estrecha para un filtro que dura 2 kHz.

**Arreglo robusto**Comparar el nivel *antes del borde* contra *todo lo que hay bien por encima*, no contra la ventana inmediatamente adyacente: ventana previa de ~1,5 kHz que termina en el candidato, frente a la media desde candidato + 1,5 kHz hasta Nyquist. Eso desacopla «cuán empinada es» de «cuánto cae en total», que es la magnitud que discrimina. Medido sobre señales sintéticas con lowpass realista, esa formulación da caídas de 85-93 dB en material con pérdida frente a 1,6-5,3 dB en roll-off natural: dos órdenes de magnitud de margen, en lugar de los 1,6 dB actuales.  
  
Y para el chequeo del extremo superior, sustituir el umbral relativo fijo por una comparación de *forma*: el piso de cuantización de un encoder es plano, mientras que el contenido de un lossless continúa la tendencia del espectro. Comparar el nivel medido en 19-22 kHz contra el extrapolado de la tendencia de 10-17 kHz distingue las dos cosas sin un número a mano.

### Números pendientes de medir

Los tres que hoy están puestos a mano y que `calibrate` debe fijar contra la distribución empírica: el umbral de E05 (`0.08`), su peso (`1.0`) y `min_drop` (hoy `13/17 dB`, bajado desde 20 → 15/18 → 13/17 sin medida que lo respalde). Y el FPR de 0,00 % medido sobre 27 negativos tiene una resolución de 3,7 %: todavía no es un 1 % medido.

Cada bajada de `min_drop` sube el recall en el corpus sintético y no mueve el FPR, porque **ningún negativo sintético dispara el detector de borde**. Solo el corpus real puede castigar esa bajada; hasta entonces el número no está validado en ninguna dirección.

## 04c. Lo que quedó abierto al cerrar B-7

#### \*\*B-7b\*\* El filtro por extensión corta antes de intentar decodificar \`bdja_scan/src/scanner.rs\`

`is_unsupported_audio(path)` se evalúa en la línea 179, **antes** de cualquier intento de decodificación, y decide por la extensión. Dos consecuencias:

**1. Rompe el caso central del producto.** Un archivo con extensión `.wma` o `.mka` cuyos bytes son en realidad MP3 o PCM nunca llega a `forensic.rs`, así que la mentira de contenedor —precisamente lo que la app existe para detectar— no se detecta. La premisa de diseño era que el contenido decide y la extensión solo filtra el recorrido; aquí la extensión decide el veredicto.

**2. La lista es demasiado conservadora.** Symphonia 0.5 con `all-formats` + `all-codecs` sí lee varios de los que hoy están marcados como no soportados: `mp4`, `m4b` (mismo contenedor isomp4 que `m4a`), `oga` (mismo Ogg que `ogg`), `mp2` (el decodificador MPA cubre capas 1/2/3), `aifc` y `caf`, y `mka` vía Matroska. Esos archivos reciben hoy una fila «no soportado» por una predicción cableada, no por una limitación real. No soportados de verdad: `wma`, `opus`, `wv`, `ape`, `tta`, `dsf`, `dff`. Cada uno necesita una prueba con un archivo real antes de cablear nada.

**Arreglo: no predecir.** Una sola lista `AUDIO_EXTENSIONS` para el inventario y el recorrido; intentar la sonda de Symphonia; y emitir la fila «formato no soportado» solo cuando el decodificador devuelve efectivamente `Unsupported`. Así la cobertura sigue al decodificador sin mantenimiento, una predicción equivocada nunca cuesta un veredicto, y un contenedor mal etiquetado se sigue cazando.

#### \*\*B-9\*\* El recall se mide en una sola frontera \`bdja_cli/src/main.rs\`

`run_validate` cuenta como TP `ProbableTranscode | Suspicious` (línea 473). El recall de 92,59 % significa entonces «92,59 % de los transcodes alcanzaron al menos *Sospechoso*», no «fueron identificados como transcodificados». Son dos números distintos y el DJ vive en el segundo: si solo actúa ante *Probablemente transcodificado*, su recall efectivo es más bajo y hoy no está medido.

`validate` debe imprimir el recall en las dos fronteras (≥ +1,5 y ≥ +4,0) por separado. Es la métrica que decide qué veredicto puede llevar una acción en la UI, y sin ella el *gate* de producción (`FPR <= 1.0 && recall >= 95.0`) está evaluando la frontera permisiva.

## 04d. Infraestructura de caché, migraciones y tipado de decodificación (Cerrados)

#### **B-12** `ENGINE_REV` desactualizado en 1 `bdja_core/src/types.rs`
La clave de caché es `(path, file_size, mtime_utc, engine_rev)`. Al haberse introducido cambios sustanciales en el motor (B-1 a B-8, cálculo de espectro por media, adición de `cutoff_kind`), mantener `ENGINE_REV = 1` causaba que cualquier base con reportes previos devolviera filas obsoletas (con el veredicto antiguo, curva de espectro calculada con pico en vez de media, y `cutoff_kind` en `NULL`, forzando el fallback de UI).
- **Arreglo completado**: `pub const ENGINE_REV: u32 = 2;` en `types.rs`, complementado con test de regresión que exige `ENGINE_REV >= 2` y test de invalidación en `bdja_store`.

#### **B-13** Migraciones SQLite avanzaban versión ante errores reales `bdja_store/src/db.rs`
En bases nuevas (`version == 0`), el bloque `< 1` creaba la tabla completa v3, pero al continuar ejecutando los bloques `< 2` y `< 3` producía errores de "duplicate column name" silenciados por `let _ =`. Peor aún, en bases preexistentes un error real (base bloqueada, disco lleno, fallo de I/O) era silenciado y avanzaba el PRAGMA `user_version`, dejando la base en estado irrecuperable.
- **Arreglo completado**:
  1. Si `version == 0` (base nueva), se crea el esquema v3 completo directamente con sus 29 columnas e índices, se marca `PRAGMA user_version = 3;` y se retorna `Ok(())`.
  2. Para bases viejas (`version >= 1`), la función auxiliar `alter_ignore_duplicate` únicamente tolera el error `"duplicate column name"`, propagando cualquier otro error real con `?`.

#### **B-14** Formatos no soportados reportados como corruptos `bdja_decode/src/error.rs · bdja_scan/src/pipeline.rs · bdja_scan/src/scanner.rs`
Al converger en `AUDIO_EXTENSIONS` (B-7b), formatos como `.wv`, `.ape`, `.dsf`, `.wma` llegaban a Symphonia y producían fallos que eran etiquetados con `codec: "Error/Corrupted"`, guard *"Fallo al decodificar audio (archivo ilegible o corrupto)"* y resumen de archivo dañado. Un archivo íntegro pero en formato aún no integrado era denunciado falsamente al DJ como corrupto.
- **Arreglo completado**:
  1. Se introdujo la variante `DecodeError::Unsupported(String)` en `bdja_decode::error`, mapeada explícitamente desde `SymphoniaError::Unsupported` en el probe y al instanciar códecs.
  2. En `pipeline.rs` y `scanner.rs`, los errores de decodificación se desglosan con precisión técnica: `Unsupported` y `UnrecognizedFormat` generan veredicto `Inconclusive`, codec `"No soportado ({ext})"`, y guard descriptivo *"Formato de audio no soportado actualmente por el motor ({ext})"*, sin alarmar al usuario ni calificarlo de corrupto. Los errores de I/O se reportan como `"Error/I-O"` y sólo los fallos de paquetes o truncamiento inesperado son señalados como posible corrupción.

#### \*\*B-15\*\* `UnrecognizedFormat` es un cajón de sastre que tilda de «no soportado» a un archivo dañado \`bdja_decode/src/decoder.rs · bdja_scan/src/pipeline.rs\`

Residuo de B-14, en el sentido inverso. En `decoder.rs` 114-115 solo `SymphoniaError::Unsupported` se convierte en `DecodeError::Unsupported`; **todo lo demás que falle al sondear cae en `UnrecognizedFormat`**, incluidos `SymphoniaError::IoError` de un archivo truncado y `DecodeError` de una cabecera dañada. Y `bdja_scan/src/pipeline.rs` 96-105 traduce `UnrecognizedFormat` a «No soportado ({ext})».

Consecuencia: un `.wav` genuinamente corrupto se le informa al DJ como **«No soportado (wav)»**. WAV sí está soportado, así que el mensaje es falso en la otra dirección y lo que el DJ concluye es que la app no lee WAV.

Lo mismo, más estrecho, en `DecoderInit` (línea 107): dice «Códec no soportado», pero como `SymphoniaError::Unsupported` ya se captura en la línea 167, esa variante solo contiene fallos de inicialización que **no** son de códec no soportado.

Esto importa más ahora que antes: al unificar `AUDIO_EXTENSIONS` (B-7b) se eliminó a propósito toda lista de «lo que el motor puede decodificar», así que el único sitio capaz de distinguir «códec no soportado» de «archivo dañado» es el tipo de error del decodificador. La precisión de esas variantes pasó a ser estructural.

**Arreglo.** Separar en el sondeo `SymphoniaError::IoError` y `SymphoniaError::DecodeError` en variantes propias en lugar de plegarlas en `UnrecognizedFormat`, y dejar la redacción neutra en `DecoderInit`. `UnrecognizedFormat` queda entonces para lo que su nombre dice: un contenedor que Symphonia no reconoce.
- **Estado**: Cerrado y verificado con `test_corrupted_wav_is_not_reported_as_unsupported`.

#### \*\*B-16\*\* Discriminación precisa entre contenedor no soportado y códec interno no soportado en la sonda (WAV con MP3 0x0055) \`bdja_decode/src/decoder.rs\`

Cuando Symphonia devuelve `SymphoniaError::Unsupported(msg)` en el sondeo, existen dos causas con mensajes distintos:
1. Ningún lector aceptó el flujo (`msg == "unsupported format"` o `"no format reader accepted"`): el contenedor es irreconocible.
2. Un lector aceptó el contenedor pero la etiqueta/formato interno no está soportado (ej. `"wav: unsupported wave format"` para un WAV con etiqueta `0x0055` / `WAVE_FORMAT_MPEGLAYER3`).

- **Arreglo completado**:
  - En lugar de usar una lista fija de extensiones como proxy (`is_core_supported_ext`), `decoder.rs` inspecciona directamente el mensaje de Symphonia:
    - Si `msg == "unsupported format"` $\implies$ `DecodeError::UnrecognizedFormat`.
    - Si el mensaje proviene de un lector que sí reconoció el contenedor $\implies$ `DecodeError::Unsupported(msg)`.
  - Con esto, un WAV que declara MP3 (0x0055) se reporta limpiamente como formato no soportado por el decodificador (`Unsupported`) y **nunca** como cabecera corrupta (`CorruptedHeader`).
  - Se eliminó la lista cableada de extensiones, permitiendo que formatos como `mp4`, `m4b`, `oga`, `mp2`, `aifc`, `caf` y `mka` reciban el tratamiento nativo de Symphonia sin predicciones.
  - Verificado con test unitario `test_wav_with_mp3_tag_0x0055_is_unsupported_codec_not_corrupted` en `decode_tests.rs`.

#### Detalle menor de B-13 (Cerrado)

El camino `if version == 0` crea el esquema v3 y retorna. Una base preexistente con `user_version = 0` que ya tuviera tabla `file_report` —de una compilación anterior al versionado— recibiría `CREATE TABLE IF NOT EXISTS` como no-op y quedaría sellada en `user_version = 3` sin las columnas de v2/v3.
- **Arreglo completado**: Guarda `table_exists` implementada en `db.rs 162`. Si `version == 0 && !table_exists`, crea v3 directo; si la tabla ya existía, recorre las migraciones incrementales `version < 2` y `version < 3` y estampa `PRAGMA user_version = 3`. Verificado con `test_preexisting_unversioned_db_is_migrated_to_v3`.

#### Sincronización de `ENGINE_REV` con hash de pesos (Cerrado)

El test anterior `test_engine_rev_is_updated` afirmaba `ENGINE_REV >= 2`, pasando para siempre.
- **Arreglo completado**: Se formalizó `ENGINE_WEIGHTS_SPEC` y la prueba `test_engine_rev_is_synchronized_with_weights_hash` valida mediante FNV-1a de 64 bits que `ENGINE_REV == 2` y que la huella sea exactamente `0x3916e18a4b583491`. Cualquier cambio futuro en la heurística o pesos romperá la prueba hasta que se incremente deliberadamente `ENGINE_REV`.

## 05. Menores abiertos

| Ref         | Qué                                                                                                                                                                                                                                                | Esfuerzo |
|-------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|----------|
| api.rs      | `analyze_file_quick` sigue siendo alias literal de `analyze_file`, y `scan_directory_audio` sigue como segundo camino al mismo trabajo que `start_scan_job`.                                                                                       | bajo     |
| scanner.rs  | El recorrido sigue en dos fases: se descubre todo el árbol a un `Vec` antes de analizar. Ya reporta progreso durante el descubrimiento, pero `crossbeam-channel` está en las dependencias sin usar y es lo que permitiría solapar walk y análisis. | medio    |
| db.rs       | El `blake3_hash` se calcula y se guarda pero no se usa para dedupe. Es una consulta `GROUP BY` y habilita la vista de duplicados. **Absorbido por R-3.3** (§08): cruzado con el veredicto da «tienes este track tres veces y una es la transcodificada».        | bajo     |
| scan_job    | La tabla existe pero el escaneo no reanuda desde ella.                                                                                                                                                                                             | medio    |
| temporal.rs | E14 sigue midiendo varianza de energía entre ventanas y no consistencia de evidencias. Ya no hace daño (se quitó la amortiguación), pero aporta +0,5 por la variable equivocada — y aparece precisamente en uno de los cuatro FP.                  | medio    |
| UI          | Sin i18n. Los textos de las evidencias están embebidos en el motor, que es la parte más costosa de externalizar.                                                                                                                                   | alto     |
| decode      | Opus, WMA, WavPack, APE, TTA y DSD sin decodificador en Symphonia 0.5: entran al inventario con «formato no soportado» pero no reciben veredicto. **Absorbido por R-1.1** (§08): un respaldo por `ffmpeg` detectado los cubre todos de una vez, en lugar de seis integraciones nativas. | medio    |

## 06. v1.1 especificada

Aprobada de diseño, a implementar cuando el motor esté calibrado. Ninguna de estas toca archivos del usuario salvo la primera, y esa solo por selección explícita.

<table>
<colgroup>
<col style="width: 50%" />
<col style="width: 50%" />
</colgroup>
<thead>
<tr class="header">
<th>Función</th>
<th>Especificación</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td>**Re-empaquetado a FLAC**</td>
<td>Nunca a MP3: re-encodear a lossy es una segunda generación y degrada lo que el DJ tiene. FLAC comprime el PCM decodificado sin pérdida (ahorro típico 35-50 % sobre un transcode, no 80 %). Verificación obligatoria bit-exacta: decodificar el FLAC resultante, comparar blake3 del PCM contra el del origen y descartar la salida si no coincide. Archivo nuevo siempre, ruta predeterminada junto al origen y configurable, nunca borrar el original. Registro en tabla `repack_log`. Encoder: `flacenc` (Rust puro); MP3 exigiría LAME y su licencia.<br />
<br />
Al admitirse selección masiva, la cola de re-empaquetado necesita progreso, cancelación y reanudación como el job de escaneo — no es opcional. Y la verificación bit-exacta duplica el I/O (se escribe el FLAC y se vuelve a leer para comparar el hash), así que unos cientos de archivos no es una operación instantánea.</td>
</tr>
<tr class="even">
<td>**Selección**</td>
<td>Casillas desmarcadas por defecto. **«Seleccionar todo» sí existe** y aplica al *filtro activo*, no a toda la base: si el DJ filtró por «probable transcode», marca esos; si está viendo todo, marca todo. Semántica de explorador de archivos. Selección por filtro también disponible (corte ≤ 16 kHz, confianza mínima, veredicto).<br />
<br />
La seguridad no está en restringir qué se puede seleccionar, sino en mostrar qué está seleccionado antes de escribir. Pantalla de confirmación obligatoria con desglose por veredicto, tamaño actual y estimado, y destino:<br />
<br />
`412 archivos · 18,4 GB → ~9,1 GB · 380 probable transcode, 20 inconcluso, 12 lossless verificado`<br />
<br />
Los inconclusos y los lossless verificados no se excluyen: un lossless genuino también se puede pasar a FLAC sin pérdida y ahorrar espacio. Se muestran en el desglose y decide el usuario.</td>
</tr>
<tr class="odd">
<td>**Exportación sidecar**</td>
<td>CSV y M3U con los sospechosos, para importar en el software DJ. Nada de escribir en los tags: Serato guarda cue points y beatgrid en chunks propietarios (`Serato Markers2`, `Serato Overview`) y escribirlos con una librería genérica puede corromper el trabajo del DJ; Rekordbox mantiene su propia base y no vería el cambio.</td>
</tr>
<tr class="even">
<td>**Tarjeta de formato de origen**</td>
<td>Tres datos juntos: formato declarado, calidad real estimada y espacio desperdiciado. Redacción obligatoria en términos de compatibilidad — «compatible con MP3 320 kbps», nunca «es un MP3 320». Requisito previo: añadir a `validate` una matriz de confusión de familia de códec (MP3 / AAC / desconocido) y publicar la tarjeta solo si la exactitud medida la sostiene. Hoy E05 no tiene exactitud medida y E13 solo existe si sobrevivió un tag.</td>
</tr>
</tbody>
</table>

## 07. Orden de ejecución

Los tres primeros puntos de la versión anterior de esta tabla ya están hechos. Este es el orden vigente.

| \#  | Trabajo                                                                                                   | Por qué en este orden                                                                                                                               |
|-----|-----------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------|
| 1   | ~~`tools/corpus_build` — generador de las 9 variantes por máster con manifiesto CSV (R-0.2) y C-1 a C-6~~ | Cerrado y verificado: simetría PCM 16-bit en lossless/ (C-1), .part atómico (C-2), failures.csv (C-3), [4] desglose por máster en validate (C-4), validación de compresión (C-5), corpus_meta.json (C-6) y -map 0:a:0 -vn. |
| 2   | Corpus real (§03, R-0.1)                                                                                  | Todo lo cuantitativo depende de esto. Es lo único que no se puede hacer sin material propio.                                                        |
| 3   | `calibrate` y `validate` sobre el corpus; fijar `min_drop`, umbral y peso de E05, y la frontera de −4,00 | Aquí los pesos dejan de ser criterio y pasan a ser medida. Hasta este punto el FPR real es desconocido, no bajo.                                    |
| 4   | Matriz de confusión de familia de códec en `validate`                                                     | Requisito previo de la tarjeta de formato de origen y del informe de exactitud.                                                                     |
| 5   | Informe de exactitud publicado (R-2.3)                                                                    | Sale gratis del punto 3 y es el único argumento que la competencia no puede igualar. Es el activo comercial, no una tarea de QA.                     |
| 6   | Fase 2 del §08 — prueba audible, certificado forense, hi-res inflado, informe de biblioteca               | Alto impacto y esfuerzo bajo, todo sobre DSP y datos que ya existen.                                                                                |
| 7   | Fase 1 del §08 — respaldo `ffmpeg`, espectrograma, reproductor                                            | Paridad visible. Va después del foso a propósito: un espectrograma sobre un veredicto equivocado hace el error más creíble, no menos.               |
| 8   | Menores del §05                                                                                            | Independientes entre sí, se pueden intercalar en cualquier momento.                                                                                 |
| 9   | Fase 3 del §08 — Serato/Rekordbox, vigilancia de carpeta, duplicados, re-empaquetado                     | Cambia la categoría del producto. El re-empaquetado es la primera función que escribe archivos: solo con veredicto ya confiable.                    |
| 10  | Fase 4 del §08 — certificación del catálogo de BDJ LATAM                                                  | Depende de todo lo anterior y lo multiplica.                                                                                                        |

El punto 2 es el único que depende de ti y no de código.

## 08. Hoja de ruta competitiva

Referencia del sector: **Fakin' The Funk?** (€18,49 de por vida, gratis hasta 100 detecciones; Windows, macOS y Linux vía PlayOnLinux). Detecta por pico de frecuencia y bitrate real, con espectro, reproductor integrado, detección de clipping y acciones automáticas de renombrar/copiar/mover/borrar. Cubre MP3, MP4/M4A, OGG, OPUS, FLAC, WMA, AAC, ALAC, MPC, SPX, SFX, TTA y WAV.

**La tesis: no igualar su lista de funciones.** Un detector de pico de frecuencia condena todo lo que no llegue a 20 kHz, así que un dub, un rip de vinilo o un máster analógico de banda limitada salen marcados como falsos, y no existe un estado «no lo sé». Las catorce evidencias, el veredicto `Inconclusive` y las salvaguardas ya resuelven eso, y no se pueden copiar sin rehacer su detector. El eje donde se gana es **grado profesional: admitir la duda, explicar la evidencia y no destruir nada.**

### Fase 0 · Corpus (bloqueante absoluto)

| Ref     | Trabajo                                                                                                                                                               | Esfuerzo |
|---------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------|----------|
| R-0.1   | 100-200 másters propios con 30-40 casos de estrés: dub, ambient, minimal, rips de vinilo con hiss, analógico de banda limitada, *lowpass* de masterización a 20-21 kHz, 48 kHz remuestreado a 44,1, 96 kHz reales, WAV etiquetados por Serato | tuyo     |
| R-0.2   | `tools/corpus_build`: por cada máster genera CBR 128/192/256/320, VBR V0/V2 y AAC 128/192/256, decodificados de vuelta a WAV, más manifiesto `ruta,etiqueta,codec_origen,bitrate` | medio    |
| R-0.3   | Fijar `min_drop` (hoy 13/17 dB sin respaldo), umbral de E05 (0,08), peso de E05 (1,0) y la frontera de −4,00; recall y FPR en las dos fronteras con intervalo de confianza | bajo     |

### Fase 1 · Suelo de paridad

| Ref     | Trabajo                                                                                                                                                            | Esfuerzo |
|---------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------|----------|
| R-1.1   | Respaldo de decodificación por `ffmpeg` detectado en el sistema para todo lo que Symphonia no lea (Opus, WMA, WavPack, APE, TTA, DSD). No se empaqueta, así que no arrastra su licencia. Si falta, la fila mantiene «formato no soportado» y ofrece instalarlo. Opcional: Opus nativo vía `libopus` estático | bajo     |
| R-1.2   | Espectrograma 2D tiempo × frecuencia, **calculado bajo demanda al abrir la ficha forense y cacheado**, nunca durante el lote, para no perder velocidad de escaneo  | medio    |
| R-1.3   | Reproductor integrado con `cpal` alimentado por el decodificador Symphonia propio, no por un plugin de Flutter: el usuario oye exactamente el PCM que el motor midió | medio    |

**No perseguir:** MPC, SPX, SFX y TTA como integraciones propias. Son formatos que ningún DJ tiene y están en su lista porque una lista larga vende. APE y DSD entran gratis por R-1.1.

### Fase 2 · El foso

| Ref     | Trabajo                                                                                                                                                            | Esfuerzo |
|---------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------|----------|
| R-2.1   | **Prueba audible**: aislar y reproducir la banda por encima del corte detectado — aire y platos en un máster, silencio absoluto en un transcode. Requiere bajar la banda una o dos octavas con `rubato` antes de reproducirla, porque casi nadie oye por encima de 16-17 kHz y sin ese paso sale «silencio» en los dos casos; acompañar de medidor de nivel | bajo     |
| R-2.2   | Certificado forense por archivo exportable (HTML/PDF): espectro, espectrograma, las 14 evidencias, salvaguardas, datos del contenedor y hash blake3 que lo hace verificable | bajo     |
| R-2.3   | **Informe de exactitud publicado** con metodología abierta: composición del corpus, recall y FPR en las dos fronteras, matriz de confusión por familia de códec, y los casos en que el motor se declara Inconcluso a propósito | bajo     |
| R-2.4   | «Hi-res inflado» como veredicto propio con su filtro, en lugar de enterrado en una salvaguarda. Un 24/96 que por dentro es 44,1 no es transcode, pero el DJ pagó por datos que no existen | bajo     |
| R-2.5   | Informe de salud de biblioteca al terminar el escaneo: reparto por veredicto, qué carpetas y fuentes concentran los transcodes, peores casos ordenados | bajo     |
| R-2.6   | **E05/E06 como vector de condena sin corte espectral**: una vez medido el FPR de periodicidad MDCT (E05) y pre-eco temporal (E06) sobre cientos de másters limpios (demostrando FPR <= 1.0%), permitir que su concurrencia actúe como evidencia fuerte para condenar códecs de alta tasa (AAC 256k, MP3 V0/320k) que no presentan corte abrupto brickwall sino zeroing psicoacústico progresivo | medio |

### Fase 3 · Flujo de trabajo del DJ

| Ref     | Trabajo                                                                                                                                                            | Esfuerzo |
|---------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------|----------|
| R-3.1   | Leer crates de Serato y colecciones de Rekordbox, **solo lectura**: analizar por crate en lugar de por carpeta. Nunca escribir en sus bases — Serato guarda cue points y beatgrid en chunks propietarios | alto     |
| R-3.2   | Vigilancia de carpeta: avisar cuando entra un transcode en descargas o entregas, antes de que llegue a la biblioteca                                               | medio    |
| R-3.3   | Vista de duplicados por `blake3` cruzada con el veredicto: «tienes este track tres veces y una es la transcodificada»                                              | bajo     |
| R-3.4   | Re-empaquetado sin pérdida a FLAC con verificación bit-exacta (ver §06): archivo nuevo, nunca en sitio, nunca borrando, etiquetado como función de almacenamiento y no de calidad | medio    |
| R-3.5   | Sidecar CSV/M3U de sospechosos para abrirlos como playlist en el software de DJ                                                                                     | bajo     |

### Fase 4 · Ventaja estructural

| R-4.1   | **Certificación independiente y reporte para DJs/sellos**: generar reporte/insignia forense verificable para que cualquier DJ, editor o sello pueda validar compras, pendrives USB o entregas sin importar dónde consiga su música («que a mis amigos no los engañen») | alto     |

### Lo que no hay que hacer

| Descartado                              | Por qué                                                                                                                                                                                                         |
|-----------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Acciones destructivas automáticas       | Es su función más vistosa y su mayor pasivo. Con un FPR desconocido, un falso positivo que borra el máster de alguien es daño irreversible causado por el software. Convertirlo en argumento: «nunca borra nada», y cuarentena a carpeta aparte solo con confirmación explícita |
| Igualar su lista de formatos            | La mitad de los suyos no los tiene nadie. Cubrir lo que un DJ real tiene y resolver el resto con R-1.1                                                                                                          |
| Publicar cifras antes de medirlas       | Un «99 % de exactitud» sin corpus es el marketing vacío del que esta app se quiere diferenciar, y al primer contraste se pierde el único activo que tiene: que se le puede creer                                 |
| Construir la Fase 1 antes de la Fase 0  | Un espectrograma bonito sobre un veredicto equivocado hace el error más creíble, no menos — y cada función construida sobre el motor sin calibrar hay que revisarla después                                      |

### Precio

Ellos están en €18,49 de por vida. No competir ahí. Dentro de BDJ Studio para clientes propios, el precio es irrelevante y el valor es la suite. Si algún día se vende suelto, lo que justifica estar por encima de su precio es **R-2.3**, no la lista de funciones: un sello que rechaza entregas y un productor que decide si masteriza desde una fuente pagan por un número verificable, no por dos formatos más.

## 09. Invariantes del Generador de Corpus y Validación Granular (C-1 a C-6) — CERRADOS

Para garantizar que los resultados de calibración y validación midan estrictamente la presencia o ausencia del fraude lossy y no discrepancias de contenedor o artefactos de codificación, se implementaron los seis principios metodológicos:

- **C-1 · Simetría estricta de formato entre clases**: Cada máster original se decodifica con `ffmpeg` al mismo formato final que los transcodes (`pcm_s16le`, frecuencia de muestreo nativa, sin video/portadas) y se deposita en `output_dir/lossless/<stem>.wav`. De este modo se evita que bit-depths inflados (24/32 bits) disparen espuriamente $E_{09}$ (`bit_depth_inflated`) en la clase negativa, y que másters a 96 kHz distorsionen la tasa de muestreo frente a transcodes MP3 (máx 48 kHz).
- **C-2 · Escritura atómica sin archivos truncados en reanudación**: Todas las operaciones de escritura de WAV (`lossless/` y `transcode/`) se realizan sobre archivos temporales con sufijo `.part` forzando el contenedor con `-f wav`, y se renombran atómicamente con `os.replace` únicamente tras verificar que el proceso retornó código 0 y el archivo supera 1 000 bytes. La reanudación solo toma archivos consolidados, imposibilitando que un corte de luz o aborto manual contamine el corpus con un parcial.
- **C-3 · Registro de fallos y control de integridad**: En caso de fallo de codificación o decodificación de cualquier variante, se registra detalladamente en `failures.csv` con columnas `master,variant,error`, advirtiendo en consola de la condición de corpus incompleto para evitar sesgos por desbalance en el diseño de pruebas.
- **C-4 · Trazabilidad de máster y desglose granular en CLI**: El manifiesto emite la columna `master_source` y `bdja_cli validate` la consume. Se añade en el informe la sección `[4] DESGLOSE POR MÁSTER (MÁSTERS CON CASOS DIFÍCILES O FALLOS)`, desglosando exactamente qué másteres tuvieron fallos de detección y cuáles variantes fallaron (ej. `Miss perm: MP3 V0, AAC 256k`), permitiendo aislar de inmediato si una dificultad radica en el bitrate o en las características acústicas del máster (casos de estrés).
- **C-5 · Verificación de reducción física de tamaño**: El generador comprueba que el archivo intermedio comprimido (`.mp3` o `.m4a`) sea materialmente menor que el flujo PCM lineal sin comprimir (`size < raw_pcm * 0.95`), certificando que `ffmpeg` no realizó copias de flujo directo (passthrough) antes de registrarlo como transcode.
- **C-6 · Trazabilidad y reproducibilidad en `corpus_meta.json`**: El generador inspecciona y registra la versión completa de `ffmpeg`, el encoder MP3 (`libmp3lame`), el encoder AAC (`aac` nativo o `libfdk_aac`), la fecha UTC, la lista completa de variantes y los conteos de pistas en un archivo estructurado `corpus_meta.json`.
- **Menor · Descarte de streams auxiliares**: Parámetros `-map 0:a:0 -vn` agregados a todas las invocaciones para prevenir que portadas incrustadas (artworks) o streams de metadatos pesados alteren el flujo decodificado. Frecuencia de muestreo (`sample_rate`) registrada en el manifiesto CSV.

## 10. Principios Metodológicos y Correcciones Abiertas (C-7, C-8, R-2.6, Regla del 3 y Estrategia de Medición)

### Nota Metodológica · Aritmética de Negativos y la Regla del 3 (Cota 95% = 3/n)
Cada máster original incorporado al generador produce exactamente **1 negativo** (`ground_truth = lossless`). Las 9 variantes transcode producidas son positivos (`ground_truth = transcode`).
Por consiguiente, el multiplicador $\times 9$ produce positivos en abundancia (2 700 con 300 pistas), pero **los negativos crecen estrictamente 1 a 1 por cada máster conseguido**:
- 150 másters $\implies$ 150 negativos $\implies$ Cota superior al 95% con 0 fallos: $3/150 = \mathbf{2.00\%}$.
- 300 másters $\implies$ 300 negativos $\implies$ Cota superior al 95% con 0 fallos: $3/300 = \mathbf{1.00\%}$.

Para poder reclamar comercial y técnicamente un $\text{FPR} \le 1.0\%$ respaldado por la regla del 3, **se requieren obligatoriamente 300 másters independientes (300 negativos) sin un solo falso positivo**.

### Estrategia de Medición: Opción A (Medición Única con Umbrales Congelados) vs. Opción B (Holdout)
El requisito de 300 negativos entra en conflicto directo con una partición holdout (C-8): si se realiza una división 70/30 sobre 300 másters, la partición de prueba dispone únicamente de 90 negativos, lo que arroja una cota superior de $3/90 \approx 3.33\%$. Para alcanzar $\le 1.0\%$ en la partición de prueba se requerirían $\sim 1\,000$ másters.

**Decisión adoptada: Opción A (Prioritaria)**
- Los umbrales del motor quedan formalmente **congelados** en su estado actual.
- Los valores de `min_drop` (12.5 dB en alta frecuencia / 15.0 dB en estándar) quedan catalogados en el código y en la documentación como:
  `provisional-ajustado-sobre-n=14`
- Los 300 másters nuevos operarán como un **conjunto de prueba ciego y no contaminado (*out-of-sample*) en su totalidad**. Si el motor no produce un solo falso positivo sobre los 300 negativos legítimos, se certifica el activo comercial de $\text{FPR} \le 1.0\%$.

### Pre-registro Metodológico: Reglas de Parada Auditables (Stopping Rules para R-2.3)
Dado que una prueba ciega pierde su condición de incontaminada en el instante en que se modifique un parámetro tras observar los resultados, se registran formalmente las **reglas de parada previas a la ejecución**:

1. **Desenlace 0 Falsos Positivos ($k = 0$ sobre $N \ge 300$)**:
   - **Afirmación emitida**: $\text{FPR} \le 1.00\%$ certificado formalmente al 95% de confianza ($\text{cota } 3/N$).
   - **Acción**: Motor congelado para producción (Release Candidate). El Gate de Exactitud se declara superado sin recalibración.
2. **Desenlace 1 a 3 Falsos Positivos ($k \in [1, 3]$)**:
   - **Afirmación emitida**: No se reclama «FPR $\le 1.0\%$ estricto».
   - **Acción**: Se activa la **Opción B (Holdout 70/30)**. Se aíslan acústicamente los másters discordantes (determinar si son firmas legítimas no contempladas). Se ajustan salvaguardas sobre el 70% (210 másters) y se valida ciegamente sobre el 30% retenido (90 másters), publicando con transparencia la cota correspondiente ($\le 3.33\%$).
3. **Desenlace $> 3$ Falsos Positivos ($k > 3$, es decir FPR medido $> 1.0\%$)**:
   - **Afirmación emitida**: La hipótesis de umbrales congelados ha fallado.
   - **Acción**: Suspensión de la certificación comercial. Auditoría estructural profunda del discriminador brickwall vs. rolloff natural. No se emite veredicto de producción hasta re-entrenar con holdout y validar en una cohorte independiente nueva.

### Fuente del Corpus y Estrategia de Sourcing Dinámico (Condición de Parada: 300 Negativos Limpios)
El sistema está concebido para que el DJ o editor compre su música donde quiera (Beatport, Bandcamp, pools, tiendas locales, producciones propias) y pueda analizar cualquier carpeta, pendrive USB o disco duro local («que a mis amigos no los engañen»).
Para construir el corpus de referencia con másters reales entregados por artistas, productores o ripeos propios:
- **Condición de Parada Invariable**: La recolección no se detiene al alcanzar un cupo fijo de archivos (ej. 400), sino **estrictamente al alcanzar 300 negativos confirmados limpios** ($N_{\text{clean}} = 300$). Esto elimina cualquier riesgo de que la tasa real de descartes deje el conjunto de prueba por debajo de 300.
- **Pilotaje Estratificado (30 pistas)**: Las 30 pistas del piloto previo se sortearán cruzando las mismas fuentes (colección propia, compras de tiendas DJ, producciones de amigos y sellos locales) con los mismos pesos que se utilizarán para la recolección total, evitando sesgos poblacionales. El piloto sirve para estimar el orden de magnitud del esfuerzo, no como límite rígido.
- **Valor Forense de los Descartes**: Las pistas descartadas por transcode de origen no se destruyen: se catalogan formalmente como `lossy_confirmed` de primera generación (fraudes del mundo real procedentes del creador o de tiendas). Constituyen la sección `[5]` del informe y son la prueba empírica documental de las veces que engañaron al DJ en el mercado real.
- **Propósito Central**: El valor de la app es la soberanía total del usuario: análisis local offline de cualquier dispositivo o carpeta sin ataduras a ningún catálogo o plataforma externa.

### C-7 · Pendientes espectrales muertas en detector de corte (`spectrum.rs`)
En la implementación de `detected_cliff`, la pendiente se calculó sobre un intervalo de frecuencia relativo a la frecuencia candidata $f_b$:
$$\Delta f = f_b \cdot 0.08 \implies f_1 = 0.92 f_b, \quad f_2 = 1.08 f_b$$
$$\text{octavas} = \log_2\left(\frac{1.08}{0.92}\right) = \log_2(1.1739) \approx 0.23133 \quad (\text{constante})$$
$$\text{slope} = \frac{\text{drop}}{0.23133} \approx 4.3229 \cdot \text{drop}$$
Dado que la pendiente y la caída son matemáticamente la misma variable escalada por una constante:
- Para $\text{min\_drop} = 12.5\text{ dB}$, la pendiente calculada es $\ge 54.04\text{ dB/oct}$. El gate `slope >= 52.0` siempre es verdadero (código muerto).
- Para $\text{min\_drop} = 15.0\text{ dB}$, la pendiente calculada es $\ge 64.84\text{ dB/oct}$. El gate `slope >= 42.0` siempre es verdadero (código muerto).

**Solución requerida**: Desacoplar físicamente la pendiente de la caída haciendo la ventana espectral de transición absoluta en frecuencia (por ejemplo $\pm \Delta f_{\text{abs}} = \pm 400$–$500$ Hz fijos) de modo que el intervalo en octavas varíe con la frecuencia y la pendiente mida una propiedad física independiente, o bien eliminar la condición redundante documentando que el umbral efectivo está regido exclusivamente por `min_drop`.

### C-8 · Partición de reserva por máster (*Holdout Split*) en calibración y validación
Para evitar el sobreajuste (*overfitting*) en escenarios donde se requiera reajustar pesos o umbrales:
- El corpus debe particionarse en calibración y reserva **a nivel de máster completo** (ej. 70/30 por máster).
- **Invariante estricto**: Las 9 variantes de un mismo máster comparten la misma acústica y jamás deben dividirse entre calibración y prueba (filtración de datos / *data leakage*).
- Queda supeditado a la contingencia de la Opción B si la Opción A requiriese recalibración.

### R-2.6 · E05/E06 como vector de condena estructural sin corte espectral
El análisis de la distribución de LLR en el piloto reveló que la clase positiva es **bimodal**: los transcodes con corte artificial caen muy por encima de $+4.0$ LLR; los transcodes sin corte se quedan por debajo de $+1.5$.
Para códecs a bitrates altos (ej. AAC 256k, MP3 V0/320k), el detector de corte brickwall es físicamente inadecuado porque el encoder atenúa o zero-ea bandas con transiciones suaves psicoacústicas. Bajar $\text{min\_drop}$ colisiona con caídas analógicas antes de capturar AAC 256.
La vía real de recall para los 34 falsos negativos en material con agudos son las evidencias estructurales:
- **E05**: Periodicidad de bloque MDCT (tramas de 576 muestras en MP3, 1024 o 960 en AAC).
- **E06**: Pre-eco y artefactos de cuantización temporal.

Actualmente suman $+1.9$ LLR (Sospechoso) pero están vetadas para condenar sin una evidencia fuerte ($E_{01}, E_{07}, E_{13}$). Una vez medido su FPR sobre los 300 negativos auténticos de BDJ LATAM garantizando $\text{FPR} \le 1.0\%$, habilitar a $(E_{05} + E_{06})$ como vector de condena independiente sin necesidad de corte espectral.




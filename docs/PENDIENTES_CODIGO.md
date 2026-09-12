# BDJ Studio Audio Analyzer — Pendientes a nivel de código

> Actualizado con el diagnóstico de los 6 falsos negativos.
> 3 bloqueantes (E05 cerrado; abiertos: E02 constante en NaturalRolloff, detector de borde, corpus real) · 3 números sin medir · 7 menores · v1.1 especificada.

BDJ Studio Audio Analyzer · pendientes a nivel de código

# Pendientes del Audio Analyzer

Actualizado con el desglose forense de la primera corrida de corpus. La causa raíz de los 20 inconclusos está identificada y es de una sola evidencia. Los 4 falsos positivos no son un defecto del motor: son archivos del generador sintético que sí tenían un corte real en los bytes.

**Bloqueantes** 3 **Confirmado** frontera de −4,00 inalcanzable **Menores** 7 **v1.1 especificada** 4 funciones

## 0. Lo que ya está cerrado

Para no volver sobre esto: todo lo siguiente quedó verificado en el código y no requiere más trabajo.

Enum `Codec` con mapeo explícito · *seek* a 12 segmentos con respaldo · espectro real de punta a punta · `CutoffKind` con las tres ramas y E01 decidiendo por rama · umbrales como fracción de Nyquist · E02 aplicable ante corte · E04 y E08 no exoneran cuando hay corte · E01 como evidencia fuerte · E11 ante cualquier corte · diagnóstico de remuestreo con candidatos de 44,1 y 48 kHz · `catch_unwind` dentro de `analyze_single_file` · clipping por canal en las dos rutas · bit depth por `zero_lsb_ratio` · E06 normalizado · E05 por ventana contigua · LUFS con K-weighting y *gating* BS.1770-4 · piso de ruido por percentil · HWID derivado nativamente, fail-closed y comparación en tiempo constante · sin fallback silencioso a base en memoria · `user_version` y migraciones · tabla `scan_job` · SQL parametrizado con `ESCAPE` · caché con `mtime` y blake3 · export paginado · tope de 250 en `pending_reports` · reportes de error por archivo · CLI con `validate` y `calibrate` en tres modalidades · `bdja_ipc` y `bdja_worker` eliminados · CI con `fmt`, `clippy -D warnings`, `test`, `flutter analyze` y `flutter test`.

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

Los tres que hoy están puestos a mano y que `calibrate` debe fijar contra la distribución empírica: el umbral de E05 (`0.08`), su peso (`1.0`) y `min_drop` (`15/18 dB`, bajado desde 20). Y el FPR de 0,00 % medido sobre 28 pistas tiene una resolución de 3,6 %: todavía no es un 1 % medido.

## 05. Menores abiertos

| Ref         | Qué                                                                                                                                                                                                                                                | Esfuerzo |
|-------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|----------|
| api.rs      | `analyze_file_quick` sigue siendo alias literal de `analyze_file`, y `scan_directory_audio` sigue como segundo camino al mismo trabajo que `start_scan_job`.                                                                                       | bajo     |
| scanner.rs  | El recorrido sigue en dos fases: se descubre todo el árbol a un `Vec` antes de analizar. Ya reporta progreso durante el descubrimiento, pero `crossbeam-channel` está en las dependencias sin usar y es lo que permitiría solapar walk y análisis. | medio    |
| db.rs       | El `blake3_hash` se calcula y se guarda pero no se usa para dedupe. Es una consulta `GROUP BY` y habilita la vista de duplicados.                                                                                                                  | bajo     |
| scan_job    | La tabla existe pero el escaneo no reanuda desde ella.                                                                                                                                                                                             | medio    |
| temporal.rs | E14 sigue midiendo varianza de energía entre ventanas y no consistencia de evidencias. Ya no hace daño (se quitó la amortiguación), pero aporta +0,5 por la variable equivocada — y aparece precisamente en uno de los cuatro FP.                  | medio    |
| UI          | Sin i18n. Los textos de las evidencias están embebidos en el motor, que es la parte más costosa de externalizar.                                                                                                                                   | alto     |
| decode      | Opus sin soportar: Symphonia no lo decodifica, así que un Opus transcodeado a WAV no se analiza.                                                                                                                                                   | medio    |

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

| \#  | Trabajo                                                                                                        | Por qué en este orden                                                                                                                   |
|-----|----------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------|
| 1   | E05 inaplicable ante `FullSpectrum` (§01), con su test                                                         | Desbloquea el 68 % del material legítimo por sí solo. No depende de nada.                                                               |
| 2   | Revertir la regla de 45 dB/oct y arreglar la ventana de detección (§02), con un test para `latin_transcode_06` | Elimina una regla fitteada a un archivo antes de que se mezcle con la calibración.                                                      |
| 3   | Bajar E05 a informativo y fuera de las fuertes, hasta tener umbral medido                                      | Provisional y honesto: sin distribución medida, su peso actual no está justificado.                                                     |
| 4   | Corpus real (§03)                                                                                              | Todo lo cuantitativo depende de esto. Es lo único que no se puede hacer sin material propio.                                            |
| 5   | `calibrate` y `validate` sobre el corpus real; fijar umbral de E05 y revisar la frontera de −4,00 (§04)        | Aquí es donde los pesos dejan de ser criterio y pasan a ser medida.                                                                     |
| 6   | Matriz de confusión de familia de códec en `validate`                                                          | Requisito previo de la tarjeta de formato de origen.                                                                                    |
| 7   | Menores del §05                                                                                                | Independientes entre sí, se pueden intercalar.                                                                                          |
| 8   | v1.1 del §06                                                                                                   | Solo después de que el veredicto sea confiable: un veredicto equivocado que además escribe un archivo deja de ser un texto en pantalla. |

Los puntos 1 a 3 son trabajo de horas y no necesitan nada de fuera. El 4 es el único que depende de ti.


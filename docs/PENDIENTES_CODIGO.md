# BDJ Studio Audio Analyzer — Pendientes a nivel de código

> Inventario de lo que falta en el código, con archivo y línea. Distribución por instalación directa: firma, notarización y tiendas fuera de alcance.

BDJ Studio Audio Analyzer · pendientes a nivel de código

# Pendientes del Audio Analyzer

Inventario completo de lo que falta en el código, con archivo y línea, causa y arreglo. Distribución por instalación directa: firma, notarización y empaquetado de tiendas quedan fuera del alcance.

**Motor** 10 crates Rust · 22 tests **Pendientes** 1 bloqueante · 8 importantes · 12 menores **Funciones sin implementar** 5

## 0. Resumen por área

| Área                                     | Estado              | Qué falta                                                                                                               |
|------------------------------------------|---------------------|-------------------------------------------------------------------------------------------------------------------------|
| Identificación de formato y códec        | \*\*Completo\*\*    | —                                                                                                                       |
| Decodificación y muestreo                | \*\*Completo\*\*    | Opus no soportado por Symphonia                                                                                         |
| Detección de corte (E01, E02)            | \*\*Completo\*\*    | Tramo de E01 demasiado ancho entre 0,86 y 0,96 de Nyquist                                                               |
| Fusión de evidencias                     | \*\*Bloqueante\*\*  | Las evidencias exoneradoras cancelan las positivas: nunca se condena                                                    |
| Evidencias E05, E06, E09, E14            | \*\*Defectuosas\*\* | Una mide sobre datos no contiguos, una no es invariante a ganancia, una es placeholder, una mide la variable equivocada |
| Métricas de calidad (LUFS, TP, clipping) | \*\*Incorrectas\*\* | Sin K-weighting ni gating; medidas sobre 0,74 s y sobre mezcla mono                                                     |
| Calibración                              | \*\*No existe\*\*   | Pesos LLR a mano; sin `calibrate`, sin corpus, sin FPR medido                                                           |
| Robustez ante archivos hostiles          | \*\*Parcial\*\*     | `catch_unwind` solo en el escaneo                                                                                       |
| Escaneo masivo                           | \*\*Parcial\*\*     | Sin reanudación, walk en dos fases, cola sin tope, errores ocultos                                                      |
| Persistencia                             | \*\*Parcial\*\*     | Sin `user_version`, sin `scan_job`, filtros por interpolación de SQL                                                    |
| Licenciamiento SPP3                      | \*\*Completo\*\*    | —                                                                                                                       |
| Gate de capacidad del motor              | \*\*Parcial\*\*     | El HWID lo aporta el llamante; salt duplicado en Dart                                                                   |
| UI                                       | \*\*Funcional\*\*   | Sin i18n, sin A/B, sin espectrograma, sin vista de duplicados, sin errores visibles                                     |
| CI                                       | \*\*No existe\*\*   | 22 tests que nadie ejecuta automáticamente                                                                              |

## 01. Bloqueante: la fusión nunca condena

| Ref                    | Problema                                                                                                                                                                                                                                           | Efecto                            |
|------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-----------------------------------|
| pipeline.rs:181        | E04 devuelve `-1.4` «continuidad espectral natural» cuando `spectral_holes_ratio < 0.08`, sin mirar `cutoff_kind`. Los huecos se miden entre 10 kHz y el `cutoff_bin`, banda que en un transcode de bitrate alto está intacta por definición.      | Resta 1,4 a todo transcode limpio |
| pipeline.rs:247        | E08 devuelve `-0.8` cuando `noise_floor_db < -80`. El PCM decodificado de un MP3 tiene piso coherente.                                                                                                                                             | Resta 0,8 a todo transcode        |
| types.rs · is_strong() | Las cuatro evidencias fuertes son E04, E05, E07 y E13. Ninguna se activa en un transcode limpio de bitrate alto: no hay huecos, la rejilla de bloques es poco fiable, no hay colapso de joint-stereo y no queda tag LAME tras convertir en un DAW. | `ProbableTranscode` inalcanzable  |
| pipeline.rs:104-118    | El tramo de E01 `ratio <= 0.96` cubre de 19,0 a 21,2 kHz con un único `+1.2`.                                                                                                                                                                      | MP3 192 y MP3 320 puntúan igual   |

### Cadena reproducida

| Entrada                        | E01  | E02  | E04  | E08  | Score | Veredicto          |
|--------------------------------|------|------|------|------|-------|--------------------|
| MP3 128 → WAV (corte 16,0 kHz) | +2,2 | +1,5 | −1,4 | −0,8 | +1,5  | \*\*Sospechoso\*\* |
| MP3 192 → WAV (corte 19,0 kHz) | +1,2 | +1,5 | −1,4 | −0,8 | +0,5  | \*\*Inconcluso\*\* |
| MP3 320 → WAV (corte 20,5 kHz) | +1,2 | +1,5 | −1,4 | −0,8 | +0,5  | \*\*Inconcluso\*\* |
| AAC 256 → WAV (corte 19,5 kHz) | +1,2 | +1,5 | −1,4 | −0,8 | +0,5  | \*\*Inconcluso\*\* |

### Arreglo

    // pipeline.rs · E04 — la ausencia de huecos no es evidencia de inocencia
    // cuando la banda medida está por debajo de un corte ya detectado
    let cliff = spec.cutoff_kind == CutoffKind::BrickwallCutoff;
    let (e04_llr, e04_app, e04_desc) = if e04_val >= 0.15 {
        strong_evidence_present = true;
        (2.5, true, format!("Huecos psicoacústicos marcados ({:.1}%)", e04_val * 100.0))
    } else if e04_val >= 0.08 {
        (1.4, true, format!("Presencia moderada de huecos ({:.1}%)", e04_val * 100.0))
    } else if cliff {
        (0.0, false, "No concluyente: la banda analizada queda por debajo del corte detectado".into())
    } else {
        (-1.4, true, format!("Continuidad espectral natural ({:.1}% huecos)", e04_val * 100.0))
    };

    // pipeline.rs · E08 — igual: no exonerar si hay corte, ni si no se pudo medir
    let (e08_llr, e08_app, e08_desc) = if qual.has_exact_digital_silence && cliff {
        (0.8, true, "Silencio digital absoluto en pasajes de bajo nivel".into())
    } else if cliff {
        (0.0, false, "No concluyente: corte artificial detectado".into())
    } else if qual.noise_floor_db < -80.0 {
        (-0.8, true, format!("Piso de ruido coherente con dither ({:.1} dB)", qual.noise_floor_db))
    } else {
        (0.0, true, format!("Piso de ruido medido: {:.1} dB", qual.noise_floor_db))
    };

    // types.rs · un filtro digital de 400 dB/oct no existe en la naturaleza:
    // el corte brickwall verificado es evidencia fuerte
    pub fn is_strong(&self) -> bool {
        matches!(self, E01 | E04 | E05 | E07 | E13)   // E01 solo cuenta si cutoff_kind == Brickwall
    }

    // pipeline.rs · partir el tramo ancho de E01
    } else if ratio <= 0.90 { (1.6, "…compatible con MP3 192 kbps") }
    else if ratio <= 0.96 { (1.4, "…compatible con MP3 256/320 kbps") }

Resultado esperado sin tocar ningún peso: MP3 128 → +3,7; MP3 192/320 y AAC 256 → +2,7; los cinco casos lossless mantienen su veredicto. Con E01 como evidencia fuerte, el 128 alcanza \*\*Probable transcode\*\*.

## 02. Robustez

| Ref            | Problema                                                                                                                                                                                                                                               | Arreglo                                                                                                                      |
|----------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------|
| scanner.rs:124 | El `catch_unwind` envuelve la llamada dentro de `scan_collection`. `analyze_file` y `analyze_batch` del FFI llaman a `analyze_single_file` sin protección: arrastrar un archivo corrupto suelto puede propagar un *panic* a través de la frontera FFI. | Mover el `catch_unwind` dentro de `analyze_single_file` en `scan/pipeline.rs`. Cubre todos los caminos, presentes y futuros. |
| decoder.rs     | Los límites son tamaño ≤ 2 GB, duración ≤ 3 h y 4 000 paquetes. No hay presupuesto de tiempo por archivo ni tope de memoria: un FLAC de alta resolución y un MP3 consumen presupuestos muy distintos con el mismo contador de paquetes.                | Presupuesto en milisegundos por archivo, comprobado entre segmentos.                                                         |
| api.rs:216     | Si `ReportStore::open` falla, `engine_init` cae a base en memoria sin avisar. El usuario escanea toda su biblioteca y la pierde al cerrar.                                                                                                             | Devolver el error o marcar el estado como degradado y mostrarlo en la UI.                                                    |
| scanner.rs     | Sin protección de ciclos ni *symlinks*; sin prefijo `\\?\` para rutas largas en Windows.                                                                                                                                                               | Detección por `FileId` y prefijo de ruta larga.                                                                              |

## 03. Métricas de calidad

Es el bloque que la app promete como «calidad real» y el que un productor contrasta primero con su DAW. Ninguna de las cuatro cifras es correcta hoy.

| Ref            | Métrica             | Cómo está                                                                                                                                                          | Cómo debe estar                                                       |
|----------------|---------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------|
| quality.rs:85  | LUFS integrado      | `-0.691 + 10·log10(media cuadrática)`: RMS con el offset de R128 encima. Sin filtro K-weighting y sin *gating* de −70/−10 LU.                                      | `ebur128`, pista completa, canales reales                             |
| quality.rs:45  | True peak           | Interpolación lineal entre muestras vecinas, sobre la mezcla mono.                                                                                                 | Sobremuestreo 4× por canal, o `ebur128`                               |
| decoder.rs:226 | Clipping            | Contado sobre `(L+R)/2 >= 0.9999`. La mezcla atenúa picos: el recorte de un solo canal desaparece.                                                                 | Por canal, y contar rachas consecutivas, no muestras sueltas          |
| pipeline.rs:54 | Ventana de medición | `windows_8192.iter().take(4)` = 32 768 muestras ≈ 0,74 s. LUFS integrado, rango dinámico y piso de ruido de una pista entera calculados sobre menos de un segundo. | Acumular los 12 segmentos, o medir en *streaming* durante el decode   |
| quality.rs:56  | Piso de ruido       | `20·log10(min |muestra no nula|)`: depende de un único valor extremo, no es un piso.                                                                               | Percentil bajo de la energía en pasajes silenciosos                   |
| quality.rs:103 | Bit depth real      | Se deduce de ese piso de ruido: si un 24 bits tiene piso \> −96 dB se declara 16 bits.                                                                             | Entropía y distribución de los LSB: cuántos bits bajos son constantes |

## 04. Evidencias defectuosas

| Ref                   | Evidencia                   | Problema                                                                                                                                                                                                                                                                                                                                                                                   |
|-----------------------|-----------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| temporal.rs:18        | E05 · rejilla de bloques    | Concatena `windows_8192.iter().take(4)` —cuatro ventanas de tramos distintos de la canción— y autocorrela el resultado buscando periodicidad de 576/1024 muestras. Cada unión introduce una discontinuidad artificial. Con el *seek* funcionando, esos cuatro tramos están muy separados, así que es peor que antes. Debe calcularse dentro de cada bloque contiguo y promediar los picos. |
| temporal.rs:75        | E06 · pre-eco               | `energy_post > 0.05` sobre sumas de cuadrados sin normalizar. Una pista mezclada 6 dB más baja nunca dispara. Debe normalizarse por la energía de la ventana.                                                                                                                                                                                                                              |
| decoder.rs:364        | E09 · bit depth real        | `BitDepthStats` se construye con `estimated_real_bits = declared` y `zero_lsb_ratio = 0.0`, y nadie lo lee. La estimación real la improvisa `quality.rs` desde el piso de ruido. Falta la implementación por LSB.                                                                                                                                                                          |
| verdict/engine.rs:49  | E14 · consistencia temporal | Amortigua el score un 50 % cuando `temporal_variance > 0.8`, pero esa variable es el coeficiente de variación de la *energía* entre ventanas, o sea la dinámica de la música. Una pista con intro suave y drop fuerte se penaliza aunque el transcode sea evidente; un máster de loudness-war plano no se penaliza nunca. Debe medir la varianza de las *evidencias* entre los 12 tramos.  |
| pipeline.rs:288       | E11 · inflado de contenedor | Solo se activa si `e01_hz <= 16500`, o sea para MP3 128. Un WAV de 1411 kbps con corte a 19 o 20,5 kHz es el mismo caso de inflado y no lo detecta. Debe activarse ante cualquier `BrickwallCutoff`.                                                                                                                                                                                       |
| pipeline.rs:330       | E13 · valor contaminado     | `value: if strong_evidence_present { Some(1.0) } else { Some(0.0) }` usa el *flag* global, que ya puede venir activado por E04, E05 o E07. Un archivo sin hallazgo de metadata guarda `E13 = 1.0` en el reporte exportado.                                                                                                                                                                 |
| pipeline.rs / verdict | Dos fuentes de verdad       | `DspOutput.is_strong_evidence_present` y la lista que `evaluate_verdict` recalcula con `llr >= 1.5` pueden discrepar. Debe decidirlo una sola función, en `bdja_verdict`.                                                                                                                                                                                                                  |

## 05. Escaneo y datos

| Ref                 | Problema                                                                                                                                                                   | Arreglo                                                                                                                                                   |
|---------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------|
| scanner.rs:37       | Walk completo a un `Vec` antes de analizar el primer archivo, y `on_progress` no se llama durante el descubrimiento. En un disco entero son minutos con `total_found = 0`. | Solapar walk y análisis con `crossbeam-channel` acotado (ya está en las dependencias y sin usar). Reportar el contador de descubrimiento.                 |
| api.rs:308          | `pending_reports` sin tope: acumula todos los `FileReportFfi` entre *polls*, ~1,5 KB cada uno con el espectro de 256 `f32`.                                                | Tope (p. ej. 500) y, pasado el tope, que la UI lea de SQLite.                                                                                             |
| scanner.rs          | Los errores de decodificación se descartan (`Err(_) => return`). El contador `total_errors` del FFI nunca se llena.                                                        | Acumular por tipo de error y exponerlo en `poll_scan_job`.                                                                                                |
| db.rs               | No hay tabla `scan_job` ni checkpoints: no se puede reanudar un escaneo interrumpido.                                                                                      | `scan_job` con `cursor_json`, checkpoint cada 200 archivos.                                                                                               |
| db.rs:73-74         | Migraciones por `ALTER TABLE` con el error ignorado. Funciona para añadir columnas; no permite migraciones de datos ni detectar una base de versión posterior.             | `PRAGMA user_version` y migraciones numeradas.                                                                                                            |
| db.rs:168           | El filtro de veredicto y la búsqueda se concatenan en el SQL. Están escapados con `replace("'", "''")`, pero `%` y `_` en la ruta actúan como comodines de `LIKE`.         | Parámetros enlazados y `ESCAPE` en el `LIKE`.                                                                                                             |
| db.rs · save_report | `blake3_hash` se calcula y se guarda, pero la caché filtra por `(path, size, mtime, engine_rev)` y nadie usa el hash.                                                      | Dedupe por hash: agrupar copias del mismo audio en rutas distintas. En una biblioteca de DJ ahorra 15-30 % del trabajo y habilita la vista de duplicados. |
| export.rs           | `export_reports_csv` carga 100 000 reportes en RAM antes de escribir.                                                                                                      | Escritura por páginas.                                                                                                                                    |

## 06. Gate de capacidad

<table>
<colgroup>
<col style="width: 50%" />
<col style="width: 50%" />
</colgroup>
<thead>
<tr class="header">
<th class="mono">Ref</th>
<th>Problema</th>
</tr>
</thead>
<tbody>
<tr class="odd">
<td class="id">api.rs:178</td>
<td>El `hwid` se toma de `parts[0]` del propio token. El motor no deriva el identificador de la máquina por su cuenta, así que un token firmado para cualquier equipo es válido en todos.</td>
</tr>
<tr class="even">
<td class="id">license_manager.dart:234<br />
api.rs:185</td>
<td>El salt `BDJ_AUDIO_ANALYZER_CAPABILITY_SALT_2026` está en texto plano en los dos lados. En Dart es trivial de extraer del bundle.</td>
</tr>
<tr class="odd">
<td class="id">api.rs</td>
<td>La comparación de digests usa `!=` sobre `String`, no comparación en tiempo constante.</td>
</tr>
<tr class="even">
<td class="id">main.dart</td>
<td>La licencia se valida al arrancar y no se revalida durante la sesión.</td>
</tr>
</tbody>
</table>

Con distribución a clientes de confianza esto es de prioridad baja, pero el primer punto es el que convierte el gate en decorativo: el motor debe calcular el HWID él mismo y comparar.

## 07. Código muerto y duplicado

| Ref                    | Qué                                                                                                                                                                                                                                                               |
|------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| bdja_ipc · bdja_worker | Dos crates completos (protocolo, supervisor, binario worker) que nadie importa desde que se eligió `catch_unwind`. `tools/build_native.ps1` sigue compilando y copiando `bdja_worker.exe` a la carpeta de la app. Quitar de `members` del workspace y del script. |
| api.rs:267             | `analyze_file_quick` es `analyze_file(path)` literal.                                                                                                                                                                                                             |
| scanner.rs:149         | `scan_directory`, la versión secuencial anterior, sin llamadas.                                                                                                                                                                                                   |
| api.rs:375             | `scan_directory_audio` es un segundo camino al mismo trabajo que `start_scan_job`; delega en `scan_collection`, pero la UI ya no lo usa.                                                                                                                          |
| bdja_dsp/Cargo.toml    | `rubato` y `rustfft` declarados; `realfft` ya trae `rustfft`. Comprobar si `rubato` se usa.                                                                                                                                                                       |
| bdja_core/types.rs     | Seis structs espejo en `bdja_ffi` (`EngineInfoFfi`, `VolumeInfoFfi`, `FormatFactsFfi`…) y `ENGINE_REV` declarado en dos sitios; de ese número depende la invalidación de caché.                                                                                   |

## 08. Cobertura de tests

22 tests: 9 de DSP, 4 de decode, 4 de veredicto, 1 de supervisor, 4 de UI.

| Qué no está cubierto                        | Por qué importa                                                                                                                                 |
|---------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------|
| El veredicto de MP3 192, 320 y AAC 256      | El test del 320 solo comprueba `assert_ne!` contra `LosslessVerified` y `LikelyLossless`. Acepta *Inconcluso*, que es justo el defecto del §01. |
| Invariancia a ganancia y a silencio añadido | Son las propiedades que fallarían hoy con E06. Van con `proptest`.                                                                              |
| Archivos de 48 y 96 kHz                     | Los umbrales ya son relativos a Nyquist, pero E03 sigue anclado a 14/16/18 kHz.                                                                 |
| Nada de `bdja_scan` ni `bdja_store`         | Sin test de acierto de caché, de reanudación, de cancelación ni de escaneo con archivos ilegibles.                                              |
| Fuzzing de `bdja_decode`                    | Es el crate que abre bytes no confiables. `cargo-fuzz` con corpus de archivos truncados y mutados.                                              |
| Presupuestos de rendimiento                 | Ningún número de rendimiento está instrumentado. `criterion` con umbral de regresión.                                                           |
| Contratos FFI                               | Paridad de enums Rust ↔ Dart. `tools/verify_contracts.py` existe en Stems Music y aquí no.                                                      |

Y falta la CI que los ejecute: `fmt`, `clippy -D warnings`, `cargo test`, `flutter test`. En repo privado no cuesta nada y evita el patrón que se repitió en cada iteración: arreglar un módulo y romper al vecino.

## 09. Funciones sin implementar

| Función                           | Estado del código                                                                                                                                                                                                               |
|-----------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `bdja_cli calibrate` y `validate` | El CLI tiene `analyze`, `scan`, `version` y `help`. Sin estos dos no hay forma de ajustar los pesos LLR contra datos ni de medir recall y falsos positivos. Todos los números de confianza que muestra la app dependen de esto. |
| Comparación A/B                   | No existe. El motor ya devuelve todo lo necesario; es UI y una vista de diferencias.                                                                                                                                            |
| Espectrograma                     | No existe. Hay espectro promedio de 256 puntos; un espectrograma necesita guardar la matriz tiempo-frecuencia o recalcularla al abrir la ficha.                                                                                 |
| Vista de duplicados               | No existe. El `blake3` ya está en la base: es una consulta `GROUP BY` y una vista.                                                                                                                                              |
| Vigilancia de carpeta             | No existe. Requiere un *watcher* por plataforma y reutilizar el job de escaneo.                                                                                                                                                 |
| i18n                              | Todos los textos están embebidos en los widgets y en las descripciones de evidencias del motor. Los del motor son los que más cuestan de externalizar.                                                                          |
| Opus                              | Symphonia no lo decodifica. Un Opus transcodeado a WAV no se analiza; hoy sale como error de decodificación o no se detecta el origen.                                                                                          |

## 10. Orden de ejecución

| \#  | Trabajo                                                                                           | Depende de |
|-----|---------------------------------------------------------------------------------------------------|------------|
| 1   | §01 completo: E04, E08, E01 como evidencia fuerte, tramo de E01 partido, E11 ante cualquier corte | —          |
| 2   | `catch_unwind` dentro de `analyze_single_file`                                                    | —          |
| 3   | Tests de veredicto para MP3 192, 320 y AAC 256 con aserción positiva, no `assert_ne!`             | 1          |
| 4   | CI mínima                                                                                         | —          |
| 5   | §03 métricas: `ebur128`, por canal, pista completa, piso y bit depth reales                       | —          |
| 6   | §04 evidencias: E05, E06, E09, E14, E13.value, una sola fuente de «evidencia fuerte»              | 4          |
| 7   | `calibrate` y `validate` + corpus                                                                 | 1, 5, 6    |
| 8   | §05 escaneo y datos: cola acotada, progreso, errores, `scan_job`, `user_version`, dedupe          | —          |
| 9   | §07 limpieza y §06 HWID en el motor                                                               | —          |
| 10  | Funciones nuevas: duplicados, A/B, espectrograma, vigilancia de carpeta, i18n                     | 7, 8       |

Los puntos 1 a 3 son lo único que separa un motor que mide bien de un motor que dice lo que mide. El 7 es el único que necesita material tuyo.


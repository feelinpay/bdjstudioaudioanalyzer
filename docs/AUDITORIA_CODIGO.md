# BDJ Studio Audio Analyzer — Auditoría de código v2.0

> Segunda pasada · 12 de septiembre de 2026 · contrastada con PLAN_ARQUITECTURA.md v1.1 y la auditoría v1.0
> 6 de 8 bloqueantes resueltos · 2 abiertos · 10 hallazgos nuevos · no listo para producción todavía

BDJ Studio · Auditoría técnica · segunda pasada

# Auditoría del Audio Analyzer

Revisión de la actualización. Seis de los ocho bloqueantes están resueltos y bien resueltos, con tests que los cubren. Queda un defecto de medición que hace inalcanzable el mejor veredicto, una protección que existe en el repo pero no está conectada, y la calibración, que sigue sin empezar. No está listo para producción, pero ya no está lejos.

**Revisado** 12 sep 2026 · v2.0 **Cambios** +60 KB de fuente · 9 tests nuevos **Bloqueantes** 6 resueltos · 2 abiertos **Producción** no todavía

## 0. ¿Está listo para producción? ¿Quedó al 100 %?

**No, y no.** Pero la distancia cambió mucho: en la pasada anterior el motor no llegaba a ejecutarse; ahora ejecuta, detecta transcodes con precisión y ya no acusa a los archivos legítimos de tu público. Diría que estás en torno al **70 % del camino a una v1 vendible**.

Detección de lossyFuncionamide el corte de MP3 128/192/320 y AAC con error menor a 20 Hz

Certificar un losslessImposible«Lossless verificado» es inalcanzable por construcción del score

CalibraciónSin empezarlos pesos siguen escritos a mano, sin corpus ni medición de FPR

Las tres razones por las que un revisor senior no aprobaría el paso a producción hoy, en orden:

1.  **Los números que muestra al usuario no son correctos todavía.** Un WAV lossless de banda completa reporta un ancho de banda de 7-12 kHz en vez de 22 kHz. Ya no lo acusa —la guarda nueva lo evita— pero el dato que aparece en pantalla y en el reporte exportado es falso, y el mejor veredicto posible queda fuera de alcance (§02).
2.  **La confianza que muestra no está medida.** «91 % de confianza» sale de una sigmoide sobre pesos que alguien eligió a mano. Sin corpus ni tasa de falsos positivos medida, ese número no significa nada, y es el número con el que un DJ va a reclamarle a un sello.
3.  **Un archivo malformado de un USB ajeno sigue pudiendo cerrar la app** en mitad de un escaneo de 40 000 pistas. La protección está escrita en el repo pero no está conectada (§03).

Lo que sí hay que reconocer

Los seis arreglos que sí se hicieron están hechos *bien*, no a la carrera: enum de códecs con mapeo explícito y test, *seek* real a 12 segmentos con camino de respaldo, espectro real atravesando motor, base de datos y FFI, job de escaneo asíncrono con rayon y cancelación de verdad, filtro biquad real para el joint-stereo, y nueve tests nuevos que cubren precisamente los casos que fallaban. Eso es trabajo de buena calidad.

## 01. Estado de los ocho bloqueantes

| ID   | Hallazgo anterior                                                   | Estado           | Verificación                                                                                                                                                                                                                                                                                                                                                                                  |
|------|---------------------------------------------------------------------|------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| P0-1 | El veredicto nunca se calculaba: todo salía «Con pérdida declarado» | \*\*Resuelto\*\* | Nuevo `enum Codec` en `bdja_core` con mapeo explícito de las 15 constantes `CODEC_TYPE_*`, `is_lossless()` derivado del enum y `display_name()` legible. Cubierto por `test_symphonia_codec_mapping`. Bien hecho: ya no hay lógica de negocio sobre un `Debug`.                                                                                                                               |
| P0-2 | El ancho de banda efectivo no discriminaba sobre música real        | \*\*Parcial\*\*  | Sustituido por detección de borde de contenido, y para material con pérdida es **excelente**: mide 15 988 Hz donde el corte real es 16 000, y 20 489 donde es 20 500. Pero el término `hf_floor + 10` del umbral rompe el caso lossless. Ver §02 — es el defecto más importante que queda.                                                                                                    |
| P0-3 | Solo se analizaban los primeros 6 segundos                          | \*\*Resuelto\*\* | `format.seek(SeekMode::Coarse, …)` a 12 segmentos entre el 5 % y el 95 % de la duración, con respaldo secuencial si el formato no soporta *seek* o si se obtienen menos de 4 ventanas. Exactamente lo que pedía la §06 del plan.                                                                                                                                                              |
| P0-4 | El espectro de pantalla estaba inventado                            | \*\*Resuelto\*\* | `average_spectrum_db` viaja ahora en `FileReport`, se persiste como `spectrum_json` en SQLite y llega al FFI. `generate_spectrum_curve` eliminada. Lo que se dibuja es el audio del usuario.                                                                                                                                                                                                  |
| P0-5 | Escaneo de un hilo, tope de 1 000, sin caché ni cancelación         | \*\*Casi\*\*     | `start_scan_job` / `poll_scan_job` / `cancel_scan_job`, hilo en segundo plano, pool de rayon con modos turbo/normal/silent, caché por SQLite y sin tope de archivos. La UI hace *polling* a 10 Hz y cancela de verdad. Quedan cuatro cosas, todas en §04: la clave de caché sin `mtime`, el recorrido en dos fases sin progreso, la ausencia de reanudación y la cola de reportes sin límite. |
| P0-6 | Un WAV convertido con FFmpeg se marcaba como transcode              | \*\*Resuelto\*\* | `Lavf` se separó en `has_lossy_encoder_signature` vs. software de exportación legítimo; ahora aporta LLR 0,0 y se muestra como dato informativo. Cubierto por `test_forensic_lavf_ffmpeg_is_not_lossy_signature`.                                                                                                                                                                             |
| P0-7 | Los WAV etiquetados por Serato/Rekordbox se marcaban como anómalos  | \*\*Resuelto\*\* | LLR 0,0 y el texto «Metadatos estándar de software DJ presentes (Serato/Rekordbox). Totalmente legítimo.» Cubierto por `test_forensic_id3_in_wav_dj_metadata_not_transcode`. Este era el que más te iba a costar clientes.                                                                                                                                                                    |
| P0-8 | El gate de capacidad no gatea nada                                  | \*\*Abierto\*\*  | Sin cambios. `deriveCapabilityToken` sigue haciendo HMAC de la cadena literal `'::::'` ignorando `hwid` y `engineRev`, y `engine_init` en Rust sigue aceptando cualquier cadena no vacía. El `.dll` extraído del instalador se usa escribiendo `"x"`.                                                                                                                                         |

También se resolvió, sin que estuviera en la lista de bloqueantes: **E07 (joint-stereo)** ahora usa un filtro Butterworth de segundo orden real y prueba cruces candidatos a 10, 12, 14 y 16 kHz en vez de una primera diferencia sobre toda la señal con un cruce inventado. Y el **true peak** pasó de `pico × 1,05` a una interpolación entre muestras. Ambos eran P1 y están mejor.

## 02. El defecto que queda: un lossless no puede certificarse

Este es el hallazgo importante de esta pasada, y es de una línea.

El nuevo detector de borde calcula su umbral así:

    let presence_threshold = (ref_level - 45.0)
        .max(hf_floor + 10.0)     // ← el problema
        .max(-90.0);

`hf_floor` es el nivel medio del último 5 % de bins, o sea de 20,9 a 22,05 kHz. En un archivo con pérdida esa banda está muerta (−107 dB en mi reproducción), así que domina `ref_level − 45` y la medición sale perfecta. Pero en un **lossless de banda completa esa banda tiene contenido real**, así que el umbral se coloca 10 dB por encima de lo que hay cerca de Nyquist, y el barrido desciende hasta encontrar algo 10 dB más fuerte. La paradoja: cuanto mejor es el contenido de agudos, más bajo el ancho de banda que se reporta.

Reproduje la implementación tal como está, con las mismas ventanas de 8192, el mismo suavizado de 5 bins y el mismo criterio de 3 bins consecutivos:

| Señal               | hf_floor  | umbral | BW medido     | Real   |
|---------------------|-----------|--------|---------------|--------|
| lossless −6 dB/oct  | −0,2 dB   | +9,8   | 7 300 Hz      | 22 050 |
| lossless −9 dB/oct  | −12,6 dB  | −2,6   | 10 271 Hz     | 22 050 |
| lossless −12 dB/oct | −26,0 dB  | −16,0  | 12 425 Hz     | 22 050 |
| MP3 128             | −107,2 dB | −17,6  | **15 988 Hz** | 16 000 |
| MP3 192             | −106,8 dB | −17,0  | **18 987 Hz** | 19 000 |
| MP3 320             | −107,5 dB | −17,3  | **20 489 Hz** | 20 500 |
| AAC 256             | −107,5 dB | −17,7  | **19 488 Hz** | 19 500 |

### Por qué importa más de lo que parece

La guarda nueva de material band-limited evita la acusación: cuando el ancho de banda sale bajo *y* la pendiente es suave, E01 pasa a LLR 0,0 en lugar de +2,2. Eso está bien pensado y salva el caso. Pero deja tres consecuencias:

1.  **El dato en pantalla es falso.** «Ancho de banda efectivo: 10 271 Hz» sobre un master íntegro es un número que un cliente técnico va a mirar y no va a creer. Y va impreso en el reporte que el DJ manda al proveedor.
2.  **«Lossless verificado» es inalcanzable.** Esto es aritmética del código, no una estimación: en `dsp/pipeline.rs` solo existen cuatro LLR negativos —E01 −1,8, E02 −1,2, E04 −1,4 y E08 −0,8— y el veredicto máximo exige `score ≤ −4,0`. Sin el −1,8 de E01 el mínimo posible es −3,4. Y el −1,8 solo se concede si el ancho de banda medido llega a 20 500 Hz, que con este umbral no ocurre en música real. **El mejor veredicto que puede dar tu app hoy sobre un master auténtico es «Probablemente lossless».** Para un producto cuya propuesta es certificar autenticidad, y para tu idea de certificar el catálogo de BDJ LATAM, eso es un problema de negocio, no de ingeniería.
3.  **La guarda depende de una correlación afortunada.** E02 se calcula alrededor del corte que encontró el umbral equivocado; funciona porque un roll-off natural da pendiente baja. Una mezcla oscura con una caída local algo más pronunciada puede pasar de 24 dB/oct, perder la guarda y quedar con E01 = +2,2 sobre un archivo legítimo. El falso positivo está mitigado, no eliminado.

### El arreglo, ya validado

Quitar el término `hf_floor + 10.0` arregla los casos de −6 y −9 dB/oct (pasan a 22 050 Hz) y no toca la precisión en lossy. Pero deja fuera las mezclas muy oscuras (−12 y −15 dB/oct siguen midiendo bajo), porque **ningún umbral absoluto puede distinguir «caída natural pronunciada» de «corte de encoder»: el discriminante es la forma, no el nivel.**

Lo que sí funciona es convertir E01 en un *detector de borde*: buscar la caída máxima entre dos ventanas contiguas de ~1500 Hz por encima de 8 kHz, exigir una caída mínima de 30 dB y comprobar que el contenido no vuelve después. Si no hay borde, el ancho de banda *es* Nyquist y E01 exonera. Lo implementé y lo probé sobre las mismas señales:

| Señal                                   | Caída máx. | BW     | E01                        |
|-----------------------------------------|------------|--------|----------------------------|
| lossless −6 dB/oct                      | 1,6 dB     | 22 050 | \*\*sin borde → limpio\*\* |
| lossless −9 dB/oct                      | 2,6 dB     | 22 050 | \*\*sin borde → limpio\*\* |
| lossless −12 dB/oct                     | 3,2 dB     | 22 050 | \*\*sin borde → limpio\*\* |
| lossless −15 dB/oct                     | 4,0 dB     | 22 050 | \*\*sin borde → limpio\*\* |
| lossless −20 dB/oct (mezcla muy oscura) | 5,3 dB     | 22 050 | \*\*sin borde → limpio\*\* |
| MP3 128                                 | 93,0 dB    | 16 005 | \*\*corte a 16,0 kHz\*\*   |
| MP3 192                                 | 92,7 dB    | 19 003 | \*\*corte a 19,0 kHz\*\*   |
| MP3 320                                 | 92,2 dB    | 20 483 | \*\*corte a 20,5 kHz\*\*   |
| AAC 256                                 | 92,6 dB    | 19 504 | \*\*corte a 19,5 kHz\*\*   |
| MP3 128 sobre mezcla oscura −15         | 85,1 dB    | 16 005 | \*\*corte a 16,0 kHz\*\*   |

Separación total: entre 1,6 y 5,3 dB para todo lo legítimo, entre 85 y 93 dB para todo lo comprimido, y el corte localizado con un error de 5 Hz. Un aviso honesto: mi lowpass sintético es un muro a −95 dB, más abrupto que un encoder real, así que en archivos reales la caída será de 40 a 70 dB en lugar de 90. El umbral de 30 dB sigue teniendo margen de sobra, pero hay que confirmarlo con el corpus.

Y con ese cambio el ancho de banda que se muestra vuelve a ser el real, con lo que E01 puede volver a conceder su −1,8 y «Lossless verificado» pasa a ser alcanzable.

## 03. Hallazgos nuevos

#### \*\*N-1\*\* La protección contra archivos malformados existe en el repo pero no está conectada \`bdja_ipc · bdja_worker · bdja_scan · bdja_ffi\`

Esta pasada trajo `bdja_ipc/protocol.rs`, `bdja_ipc/supervisor.rs` y un `bdja_worker` funcional que lee `WorkerRequest` por `stdin`, analiza y responde. Está razonablemente escrito. Pero busqué quién lo usa y la respuesta es **nadie**: fuera de `bdja_worker` mismo, ningún crate importa `bdja_ipc`. `scan_collection` llama directamente a `analyze_single_file` en el proceso de la app.

Con `panic = "abort"` en el perfil release del workspace, eso significa que el escenario original sigue intacto: un WAV corrupto del USB de otro DJ cierra la aplicación y se pierde el escaneo. Y ahora es peor de otra manera: `build_native.ps1` copia `bdja_worker.exe` junto a la app, así que el instalador lleva un binario que no se ejecuta nunca. Infraestructura que *parece* protección es más peligrosa que no tenerla, porque nadie vuelve a mirarla.

**Arreglo**Hacer que `scan_collection` obtenga un `WorkerProcess` del supervisor por cada hilo del pool y le mande la ruta, en vez de llamar a `analyze_single_file` directamente. Añadirle al supervisor lo que le falta —**no tiene ningún timeout**: si un worker se cuelga con un archivo raro, se queda colgado para siempre— y un tope de memoria por proceso. Alternativa honesta si no quieres esa complejidad ahora: borrar `bdja_ipc` y `bdja_worker` del workspace, poner `panic = "unwind"` y envolver cada análisis en `catch_unwind`. Protege menos, pero es coherente y deja de mentir sobre lo que hay.

#### \*\*N-2\*\* La caché puede devolver veredictos viejos justo en el caso más común de tu público \`bdja_scan/src/scanner.rs · bdja_store/src/db.rs\`

La clave de caché es `(ruta, file_size, engine_rev)`. Falta el `mtime`, y la tabla no tiene columna para guardarlo. El problema concreto: cuando Serato o Rekordbox reescriben las etiquetas de un WAV, el tamaño a menudo **no cambia** porque el chunk ID3 se rellena con padding. Un archivo que el DJ reemplazó por otra versión del mismo tamaño también pasa desapercibido.

**Arreglo**Añadir `mtime_utc` y `fingerprint` (blake3 de los primeros y últimos 2 MB) a la tabla y a la clave. `blake3` ya está declarado como dependencia de `bdja_scan` y **todavía no se usa para nada** — es la pieza que falta para esto y para el dedupe de duplicados, que en una biblioteca de DJ ahorra del 15 al 30 % del trabajo.

#### \*\*N-3\*\* Los tests nuevos usan espectros de líneas, no espectros de música \`bdja_dsp/tests/dsp_tests.rs\`

Los cinco tests de DSP construyen las señales como sumas de senoides en kilohercios enteros (`for f_khz in 1..=16`). Eso es un espectro de líneas: energía en 16 bins y silencio entre ellos. La detección de bordes, los huecos espectrales y el piso de ruido se comportan de forma completamente distinta con un espectro continuo, que es lo que tiene la música.

Por eso el defecto de la §02 no lo detecta ningún test. Y `test_dsp_natural_band_limited_guard` tiene su aserción dentro de un `if output.effective_bandwidth_hz < 20000`, así que **pasa igual si el ancho de banda sale bien o sale mal**: es un test que no puede fallar por lo que lleva en el nombre.

**Arreglo**Un generador de señal con forma de música (ruido filtrado con pendiente configurable en dB/oct más piso de dither de 16 bits) y estos tres tests, que son los que faltan: lossless de banda completa debe medir ≥ 20 500 Hz; el mismo material con lowpass a 16 kHz debe medir 16 000 ± 300; y un lossless de banda completa debe alcanzar el veredicto *Lossless verificado*. Y quitar el `if` de la aserción del guard.

| ID   | Hallazgo                                                | Detalle                                                                                                                                                                                                                                                                                                                                               |
|------|---------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| N-4  | \*\*P1\*\* El CLI no tiene `calibrate`                  | `bdja_cli` pasó de 74 bytes a un binario real con `analyze`, `scan`, `version` y `help`: muy útil y bien hecho. Pero sin `calibrate` ni `validate` no hay forma de ajustar los pesos LLR contra datos ni de medir la tasa de falsos positivos. Sigue siendo la brecha más grande del proyecto.                                                        |
| N-5  | \*\*P1\*\* El recorrido sigue en dos fases              | `scan_collection` recorre *todo* el árbol a un `Vec` antes de analizar el primer archivo. En un disco completo eso son minutos con la barra en cero y `total_found` en 0, porque `on_progress` no se llama durante el descubrimiento. Falta solapar walk y análisis con una cola acotada.                                                             |
| N-6  | \*\*P1\*\* La cola de reportes pendientes no tiene tope | `pending_reports` acumula todos los `FileReportFfi` hasta que la UI hace *poll*. Cada uno lleva 256 `f32` de espectro más las cadenas: alrededor de 1,5 KB. Si el escaneo va más rápido que la UI —o si la UI se queda en segundo plano— en 100 000 archivos son cientos de megas. Hay que acotarla y, pasado el tope, dejar que la UI lea de SQLite. |
| N-7  | \*\*P2\*\* Los jobs no se eliminan nunca                | `ACTIVE_JOBS` guarda cada `ScanJob` para siempre; nada lo limpia al completarse. Fuga pequeña y acotada, pero fuga.                                                                                                                                                                                                                                   |
| N-8  | \*\*P2\*\* Sin reanudación                              | No hay tabla `scan_job` ni checkpoints. Si la app se cierra en el archivo 38 000 de 50 000, la caché evita repetir el análisis pero el recorrido completo del disco se vuelve a hacer desde cero y el usuario no recupera el trabajo «en curso».                                                                                                      |
| N-9  | \*\*P2\*\* Migraciones sin versión                      | Las migraciones son `ALTER TABLE … ADD COLUMN` con el error ignorado. Funciona para añadir columnas, pero no permite migraciones de datos ni detectar que la base viene de una versión posterior. Falta `PRAGMA user_version`.                                                                                                                        |
| N-10 | \*\*P2\*\* La UI fija `throttleMode: 'turbo'`           | Los tres modos existen en el motor y funcionan (turbo = todos los núcleos, normal = mitad acotada a 2-8, silent = 1). La UI manda siempre turbo, así que la app usa todos los núcleos del DJ sin preguntarle — justo lo contrario de tu preocupación por no desgastar el equipo. Es un selector de tres botones.                                      |

## 04. Deuda de la pasada anterior que sigue ahí

No son bloqueantes, pero son la diferencia entre «funciona» y «los números son defendibles».

| Asunto                                                                  | Estado              | Nota                                                                                                                                                                                                                                                                                                                                              |
|-------------------------------------------------------------------------|---------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| E14 mide la dinámica de la música, no la consistencia de las evidencias | \*\*Sin cambios\*\* | `bdja_verdict/src/engine.rs` no se tocó en esta actualización. Una pista dinámica sigue perdiendo el 50 % del score.                                                                                                                                                                                                                              |
| E05 autocorrela 4 ventanas concatenadas de posiciones distintas         | \*\*Sin cambios\*\* | Sigue el `windows_8192.iter().take(4)`. Ahora es más grave: con el *seek* arreglado, esas 4 ventanas vienen de tramos *muy* separados de la canción, así que las 3 uniones artificiales son peores que antes. La rejilla hay que buscarla dentro de cada bloque contiguo y promediar.                                                             |
| E06 no es invariante a ganancia                                         | \*\*Sin cambios\*\* | `energy_post > 0.05` absoluto. Una pista 6 dB más baja nunca dispara pre-eco.                                                                                                                                                                                                                                                                     |
| Lo que se llama LUFS no es LUFS                                         | \*\*Sin cambios\*\* | Sigue siendo `−0,691 + 10·log10(media cuadrática)`: sin K-weighting y sin *gating*. `ebur128` no está en las dependencias. El true peak sí mejoró.                                                                                                                                                                                                |
| Métricas de nivel sobre 0,74 s y en mono                                | \*\*Sin cambios\*\* | `mono_slice` sigue siendo `take(4)` de ventanas: 32 768 muestras. LUFS integrado y rango dinámico de una pista entera medidos sobre menos de un segundo, y el clipping sobre la mezcla `(L+R)/2`, que esconde el recorte de un solo canal.                                                                                                        |
| E09 (bit depth real) es un placeholder                                  | \*\*Sin cambios\*\* | `BitDepthStats` se sigue construyendo con `estimated_real_bits = declared` y `zero_lsb_ratio = 0.0`, y nadie lo lee. Falta la entropía de los LSB, que es lo que detecta de verdad un 24 bits inflado.                                                                                                                                            |
| Dos fuentes de verdad para «evidencia fuerte» y `E13.value` contaminado | \*\*Sin cambios\*\* | `E13.value` se sigue rellenando con el `bool` global, así que un archivo sin hallazgo de metadata puede guardar `E13 = 1.0` en el reporte exportado.                                                                                                                                                                                              |
| Código muerto y duplicado                                               | \*\*Parcial\*\*     | `analyze_file_quick` sigue siendo un alias literal. `scan_directory` (la vieja, secuencial) sigue en `scanner.rs` sin que nadie la llame. `scan_directory_audio` sobrevive como segundo camino, aunque al menos ahora delega en `scan_collection`. `rubato` se añadió a las dependencias de `bdja_dsp`; conviene comprobar que se usa o quitarlo. |
| Sin CI, sin firma, sin notarización, sin fuzzing                        | \*\*Parcial\*\*     | Apareció `distribution/installer.iss`, que es un paso real. Siguen faltando `.github/workflows/`, el certificado EV de Windows, el Developer ID y la notarización de Apple, el empaquetado de macOS, `cargo-deny`/`cargo-audit`/SBOM y `cargo-fuzz`.                                                                                              |
| Sin i18n ni reporte PDF                                                 | \*\*Sin cambios\*\* | El reporte PDF de reclamo sigue siendo, para mí, la función con mejor relación valor/esfuerzo de toda la lista.                                                                                                                                                                                                                                   |

## 05. ¿Es código a nivel senior?

El código **nuevo** de esta pasada, en su mayoría sí. Lo diría en una revisión de PR sin reservas sobre: el mapeo explícito de códecs con su test, el *seek* con camino de respaldo, el job asíncrono con `AtomicU64` para el progreso y token de cancelación consultado dentro del bucle paralelo, el biquad del joint-stereo con barrido de cruces candidatos, el protocolo IPC con marcos, y el hecho de que los seis arreglos vinieran *con tests*. Eso es oficio.

Lo que un revisor senior marcaría como bloqueante no es el estilo, son tres decisiones:

- **Una función de seguridad que finge.** `deriveCapabilityToken` ignorando sus dos parámetros no es un bug menor: es una función cuyo nombre afirma algo que no hace, y el que la lea en seis meses va a creerle. Lo mismo con `bdja_ipc` sin conectar.
- **Un test que no puede fallar.** La aserción dentro del `if` en el test del guard. Es peor que no tener el test, porque da cobertura falsa.
- **Un motor de decisión sin calibrar.** Umbrales y pesos escritos a mano en un producto que emite juicios sobre el trabajo de terceros. La ingeniería está bien; lo que falta es la evidencia de que los números son correctos.

Y una observación de proceso, que es la que más te va a ahorrar tiempo: esta actualización tocó 20 archivos a la vez, resolvió seis bloqueantes e introdujo tres hallazgos nuevos, dos de ellos dentro de los propios arreglos (el `hf_floor` del detector de borde y el IPC sin conectar). Eso pasa cuando se arregla todo en una tanda sin un criterio de aceptación por arreglo. Las fases del plan existen justamente para eso: cerrar una, comprobarla contra su criterio, y solo entonces abrir la siguiente.

## 06. Qué falta exactamente para producción

Cuatro tandas. La primera es corta y desbloquea todo lo demás.

T1 — Cerrar los dos bloqueantes abiertos1-2 días

E01 como detector de borde (§02), con los tres tests que faltan y el generador de señal con forma de música. Gate de capacidad real: HMAC sobre `hwid ‖ engine_rev ‖ ventana` en Dart y la misma verificación en Rust con comparación en tiempo constante. Y decidir el IPC: conectarlo con timeout, o retirarlo y usar `catch_unwind`.

**Criterio de aceptación**Un WAV lossless de banda completa reporta ≥ 20 500 Hz y sale *Lossless verificado*; el mismo tema en MP3 320 → WAV reporta ~20 500 y sale *Probable transcode*; `engine_init("x", …)` falla; y un archivo corrupto no cierra la app.

T2 — Corpus y calibración3-4 días

`bdja_cli calibrate` y `validate`, matriz de transcodes generada con tus masters, pesos ajustados contra datos, y los umbrales del §09 del plan como gate de CI.

**Criterio de aceptación**FPR \< 1 % sobre lossless genuino y recall \> 95 % en MP3 ≤ 256 kbps, medidos por un comando reproducible. Hasta aquí, ningún porcentaje de confianza que muestre la app significa nada.

T3 — Escaneo de grado producción2-3 días

`mtime` y huella blake3 en la clave de caché, dedupe de duplicados, walk y análisis solapados con cola acotada y progreso desde el primer segundo, tope en la cola de reportes, limpieza de jobs terminados, `scan_job` con checkpoints para reanudar, `user_version` en SQLite, selector de intensidad en la UI y recuento de errores visible.

**Criterio de aceptación**50 000 archivos con la app matada dos veces: reanuda sin repetir ni perder, RSS por debajo de 250 MB durante todo el proceso, y la barra de progreso se mueve desde el primer segundo.

T4 — Precisión, limpieza y distribución4-5 días

La deuda del §04: E14 sobre evidencias, E05 por bloque contiguo, E06 normalizado, `ebur128` para LUFS y métricas sobre la pista entera y por canal, E09 por LSB, una sola fuente para «evidencia fuerte», borrar los alias y las funciones muertas. Reporte PDF, i18n es/en, workflows de CI, firma EV, Developer ID con notarización, empaquetado de macOS, `cargo-deny`/`audit`/SBOM y `cargo-fuzz`.

**Criterio de aceptación**Instalador firmado que arranca limpio en un Windows 10 nuevo y en un macOS 12 sin Xcode, sin avisos de SmartScreen ni Gatekeeper, con cero warnings de clippy y cero code smells.

**Total: 10-14 días** hasta algo que puedas vender sin reservas. Eran 16-21 en la pasada anterior.

## 07. Funcionalidades que le veo

Ordenadas por lo que de verdad mueve la aguja para un DJ que no quiere que lo estafen.

| Funcionalidad                                  | Por qué                                                                                                                                                                                                                                                                                     | Valor / esfuerzo |
|------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|------------------|
| **Reporte PDF de reclamo**                     | Hash del archivo, veredicto, evidencias con su peso, el espectro real y la versión del motor. Es el único entregable que convierte el análisis en dinero recuperado, y cada uno que circula lleva tu marca.                                                                                 | altísimo / bajo  |
| **Comparación A/B**                            | Arrastrar el master y la versión recibida, ver las dos curvas superpuestas y la tabla de diferencias. Es la demo que se explica sola y la mejor pieza de marketing posible para este producto.                                                                                              | alto / medio     |
| **Vigilancia de carpeta**                      | Analizar automáticamente lo que entra en la carpeta de descargas del pool y avisar solo si algo sale sospechoso. Convierte una compra puntual en una herramienta que se queda abierta todos los días.                                                                                       | alto / medio     |
| **Leer las bibliotecas de Serato y Rekordbox** | Analizar «mi biblioteca» tal como el DJ la ve, devolviendo el resultado por crate o por playlist, en lugar de pedirle que elija carpetas. Nadie en este espacio lo hace, y ya tienes hecha la parte difícil: reconocer y respetar sus metadatos. Solo lectura, nunca escribir en sus bases. | altísimo / alto  |
| **Espectrograma y modo Forensic**              | Ya estaba planificado para la v1.1 y ahora que el espectro es real tiene sentido. Es lo que pide el productor, no el DJ.                                                                                                                                                                    | medio / medio    |
| **Detección de Opus y WavPack**                | Symphonia no decodifica Opus. Hoy un Opus transcodeado a WAV se te escapa. Requiere un decoder aparte o FFmpeg como plugin opcional.                                                                                                                                                        | medio / medio    |
| **Renombrado o etiquetado por veredicto**      | Mover los sospechosos a una carpeta, o escribir el veredicto en un tag. Ojo: tocar los archivos del usuario es la función más peligrosa de la lista; solo con confirmación explícita, jamás automática.                                                                                     | medio / bajo     |
| **Certificación del catálogo de BDJ LATAM**    | Sigue siendo tu ventaja estructural: analizar cada remix antes de publicarlo y mostrar el veredicto en la ficha de descarga. Ningún competidor puede replicarlo porque ninguno tiene catálogo. Y te da el corpus de calibración más realista que existe.                                    | estratégico      |

## 08. Mejoras que propongo

### Del motor

- **E01 como detector de borde** (§02). Es el cambio con más impacto de toda la lista: arregla el dato que se muestra, desbloquea el veredicto máximo y elimina la dependencia de la guarda.
- **Un umbral por sample rate.** Hoy las fronteras de E01 (16 500 / 18 500 / 19 800 / 20 500) están escritas suponiendo 44,1 kHz. En un archivo de 48 kHz el Nyquist es 24 kHz y esos números significan otra cosa. Deben expresarse como fracción de Nyquist o tener tabla propia por sample rate.
- **Guardar la evidencia, no solo el veredicto.** Ya guardas el espectro. El siguiente paso es versionar el `engine_rev` en la UI y poder decir «este archivo se analizó con el motor 3; hay motor 7 disponible, ¿reanalizar?». Es lo que hace que un veredicto viejo no envenene una reclamación.
- **Presupuesto de tiempo por archivo**, no solo de paquetes. Hoy el tope es «4 000 paquetes» y «60 paquetes por segmento», que en un FLAC de alta resolución significa algo muy distinto que en un MP3.

### Del producto

- **Que el usuario elija la intensidad** (turbo / normal / silencioso). El motor ya lo soporta; son tres botones y responde directamente a tu preocupación de no maltratar el equipo del DJ.
- **Mostrar los errores.** Hoy los archivos que fallan al decodificar se descartan en silencio (`Err(_) => return`). El DJ debería ver «38 archivos no se pudieron leer» con la lista, porque a veces *eso* es el hallazgo: un archivo corrupto que compró.
- **Publicar las cifras.** Cuando T2 esté hecho, poner el recall por códec y el FPR en la web y en la app. «En AAC detectamos el 60 %» genera más confianza que el silencio, y ningún competidor ofrece números verificables.
- **Que «Inconcluso» se vea como resultado y no como fallo.** Una herramienta que se abstiene cuando no sabe es la que un DJ enseña a su proveedor sin quedar en ridículo.

### Del proceso

- **Un arreglo, un test que falla primero.** Los dos hallazgos nuevos de esta pasada nacieron dentro de los propios arreglos. Escribir el test que falla antes de tocar el código los habría cazado en el momento.
- **Cerrar fases contra su criterio de aceptación** en vez de avanzar en paralelo. El plan tiene los criterios escritos; usarlos como puerta.
- **CI ya, aunque sea mínima.** `fmt` + `clippy -D warnings` + `cargo test` + `flutter test` en cada *push*. Con 9 tests nuevos que cubren cosas reales, ya vale la pena que corran solos.
- **Anotar las decisiones.** `docs/adr/` sigue vacío. Cuando en tres meses te preguntes por qué el IPC existe y no se usa, un ADR de tres líneas te ahorra la arqueología.

## 09. Qué hago ahora

Dime y arranco. Mi orden sería:

1.  **T1, ahora mismo.** El detector de borde de E01 con sus tres tests, el gate de capacidad real y la decisión sobre el IPC. Son uno o dos días y es lo que separa «funciona» de «los números son correctos».
2.  **El corpus.** Sigue siendo lo único que no puedo hacer yo: 200-400 masters lossless tuyos, variados, incluyendo a propósito material band-limited legítimo. Con eso monto `calibrate` y los gates de CI, y a partir de ahí los porcentajes que muestra la app son defendibles ante un sello.
3.  **El reporte PDF.** Es medio día de trabajo y es la pieza que hace que el análisis sirva para algo fuera de tu pantalla.

Y una pregunta que necesito contestada antes de T1: **¿conectamos el IPC con workers de verdad, o lo retiramos y usamos `catch_unwind`?** Lo primero es la arquitectura del plan y protege de verdad; lo segundo es media hora y deja el repo honesto. Cualquiera de las dos está bien; lo que no puede quedarse es el estado actual, con la protección a medio construir dentro del instalador.


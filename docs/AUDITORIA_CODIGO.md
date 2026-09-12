# BDJ Studio Audio Analyzer — Auditoría de código v3.0

> Tercera pasada · 12 de septiembre de 2026 · contrastada con PLAN_ARQUITECTURA.md v1.1
> Los 8 bloqueantes originales cerrados · 1 crítico nuevo: el MP3 320 convertido a WAV puede salir «Lossless verificado»

BDJ Studio · Auditoría técnica · tercera pasada

# Auditoría del Audio Analyzer

Los ocho bloqueantes originales están cerrados y el detector de borde de E01 quedó bien implementado. Pero apareció un problema nuevo y es el peor posible para este producto: el caso más común de fake-lossless del mundo DJ —un MP3 320 convertido a WAV— puede salir como «Lossless verificado». La evidencia se encuentra correctamente y se descarta en el último paso.

**Revisado** 12 sep 2026 · v3.0 **Cambios** 9 archivos · +15 KB · 1 test nuevo **Bloqueantes originales** 8 de 8 cerrados **Nuevo** 1 crítico de corrección

## 0. Dónde estás

Bloqueantes originales8 de 8cerrados, incluido el gate de capacidad con HMAC real

MP3 128-256 → WAVDetectacorte localizado con error menor a 300 Hz

MP3 320 → WAVSe escapapuede salir «Lossless verificado» por la tabla de umbrales

Estás en torno al **85 % del camino a una v1 vendible**, y el trabajo de esta pasada fue bueno: el detector de borde es la implementación correcta del problema, el token de capacidad ya se verifica de verdad, la caché ya incluye `mtime`, el selector de intensidad está en la UI y hay un test que comprueba de punta a punta que un master genuino alcanza «Lossless verificado».

Lo que impide producción hoy es una sola cosa, y es de corrección, no de arquitectura: **la tabla de umbrales de E01 en `dsp/pipeline.rs` no se actualizó cuando cambió el detector**, y esa desincronización hace que el motor tire a la basura el hallazgo justo en el caso más común que tienes que detectar. Se arregla en un archivo.

Nota de alcance

Tomo nota y lo aplico en este informe: nada de reclamos ni reembolsos. El producto le entrega al DJ o al productor **los datos y la calidad real del archivo**, y ahí termina su trabajo. He reescrito las recomendaciones con ese criterio — el informe exportable pasa a ser una ficha técnica del archivo, no un documento para reclamar.

## 01. Lo que se arregló en esta pasada

| ID   | Hallazgo                             | Estado           | Cómo quedó                                                                                                                                                                                                                                                                                                                                                                                                                                               |
|------|--------------------------------------|------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| P0-2 | El ancho de banda no discriminaba    | \*\*Resuelto\*\* | Reemplazado por un detector de borde con tres ramas bien pensadas: *corte brickwall* (ventana diferencial de 1 200 Hz, caída ≥ 20 dB y comprobación de supresión sostenida posterior), *espectro pleno* (energía real en el 88-100 % de Nyquist) y *roll-off natural*. El término `hf_floor + 10` que rompía el caso lossless desapareció. Reproducido: lossless de −6 y −9 dB/oct dan 22 050 Hz; MP3 128 da 15 994 Hz con el filtro realista de −60 dB. |
| P0-8 | El gate de capacidad no gateaba nada | \*\*Resuelto\*\* | Dart genera `hwid:HMAC-SHA256("BDJA_CAPABILITY:hwid:engine_rev")` y Rust recalcula el HMAC con `hmac` + `sha2` y rechaza el token si no coincide. Ya no se entra escribiendo `"x"`. Quedan dos matices en §04, pero el agujero está cerrado.                                                                                                                                                                                                             |
| N-2  | La caché devolvía veredictos viejos  | \*\*Resuelto\*\* | Columnas `mtime_utc` y `blake3_hash`, índice `idx_cache` sobre `(path, file_size, mtime_utc, engine_rev)` y un `get_cached_report` que filtra por la tupla completa. El caso del WAV re-etiquetado por Serato con el mismo tamaño ya no devuelve un veredicto obsoleto.                                                                                                                                                                                  |
| N-3  | Tests con espectros de líneas        | \*\*Casi\*\*     | Llegó `test_dsp_continuous_lossless_attains_verified` con densidad espectral continua de −4,5 dB/oct y piso de dither, y verifica de punta a punta que el veredicto es `LosslessVerified` con score ≤ −4,0. También se quitó el `if` que hacía infalible el test del guard. Falta el test del caso de §02.                                                                                                                                               |
| N-7  | Los jobs no se eliminaban            | \*\*Resuelto\*\* | `map.retain(|id, j| *id == job_id || !j.is_completed…)` en `poll_scan_job`.                                                                                                                                                                                                                                                                                                                                                                              |
| N-10 | La UI fijaba `turbo`                 | \*\*Resuelto\*\* | Selector de tres opciones (Turbo / Normal / Silencioso) conectado a `_throttleMode` y deshabilitado durante el escaneo. El valor por defecto sigue siendo turbo; yo pondría Normal.                                                                                                                                                                                                                                                                      |

## 02. El caso que más importa se escapa

Este es el hallazgo de esta pasada. Un DJ compra un MP3 320, lo convierte a WAV y lo vende o lo entrega como lossless. Es *el* escenario del producto. Hoy puede salir certificado como auténtico.

#### \*\*P0-9\*\* La evidencia se encuentra y se descarta en el último paso \`bdja_dsp/src/pipeline.rs (sin cambios desde la v2)\`

El detector nuevo hace su trabajo: reproduciéndolo sobre una mezcla con lowpass a 20,5 kHz —el filtro típico de LAME a 320 kbps— encuentra el corte brickwall y lo sitúa en **20 731 Hz** con una pendiente de 451 dB/oct. Tiene la respuesta correcta en la mano.

Pero la tabla de umbrales de E01 en `dsp/pipeline.rs` no se tocó cuando cambió el detector, y solo mira la frecuencia:

    } else if e01_hz >= 20500.0 {
        (-1.8, format!("Espectro completo hasta {} Hz sin corte artificial", …))

20 731 ≥ 20 500, así que el corte brickwall recién detectado recibe **−1,8 de evidencia exoneradora** y un texto que afirma exactamente lo contrario de lo que se midió. Y hay un segundo golpe: E02, que es quien lleva la pendiente de 451 dB/oct y daría +1,5, está declarada `applicable: e01_hz < 20500.0`, así que en este caso **se desactiva**.

El resultado, sumando las evidencias que quedan activas —E01 −1,8, E04 −1,4 porque por debajo del corte el espectro está intacto, E08 −0,8 por el piso de dither del decodificado— es un score de −4,0, que es justo el umbral de `LosslessVerified`. Es decir: **el MP3 320 convertido a WAV no solo se escapa, sale certificado.** Solo lo salvaría que sobreviviera un tag LAME en el WAV (E13), lo que no ocurre cuando la conversión la hace un DAW.

**Arreglo**Que E01 decida por la **rama**, no por la frecuencia. El detector ya sabe si encontró un borde; hay que hacer que esa información llegue a la fusión: `SpectrumAnalysis` debe devolver un `cliff_detected: bool` (o un `CutoffKind` con `Brickwall` / `FullSpectrum` / `NaturalRolloff`) y E01 debe leerlo: *borde detectado* → LLR positivo **en cualquier frecuencia** por debajo de Nyquist − 500 Hz, escalado por la frecuencia (a 16 kHz pesa más que a 20,5); *espectro pleno* → −1,8; *roll-off natural* → 0,0 con la guarda. Y quitar la condición `applicable: e01_hz < 20500.0` de E02, que hoy apaga la evidencia justo cuando más hace falta.

La lección de proceso, dicha una sola vez: este defecto nació de cambiar el detector sin revisar quién consume su salida. Es el mismo patrón de la pasada anterior —el arreglo introduce el hallazgo siguiente— y se evita con un test por caso de negocio antes de tocar el código. Concretamente, el test que faltaba y que lo habría cazado en el momento: *«MP3 320 con corte a 20,5 kHz debe dar Probable transcode o Sospechoso, nunca Lossless verificado»*.

## 03. Las mezclas oscuras, en los dos sentidos

Un efecto secundario del umbral de entrada del detector, con las dos caras del problema.

Para aceptar un corte, el detector exige que antes del borde haya «energía acústica real»: `db_before >= (ref_level - 45.0).max(-75.0)`, donde `ref_level` es el máximo entre 1 y 6 kHz. En una mezcla normal, el nivel a 16 kHz está unos 36-40 dB por debajo de ese máximo y pasa. En una mezcla oscura —mucha música electrónica, hip-hop, cualquier máster con los agudos contenidos— está 55-60 dB por debajo y **no pasa el umbral**, así que el corte no se detecta y el archivo cae en la rama de roll-off natural.

Lo reproduje variando la pendiente del material y aplicando siempre el mismo lowpass de MP3 128 a 16 kHz:

| Material                                 | Umbral actual (ref−45) | Propuesto (ref−70) |
|------------------------------------------|------------------------|--------------------|
| lossless −9 dB/oct *(no debe detectar)*  | sin corte ✓            | sin corte ✓        |
| lossless −12 dB/oct *(no debe detectar)* | sin corte ✓            | sin corte ✓        |
| lossless −15 dB/oct *(no debe detectar)* | sin corte ✓            | sin corte ✓        |
| lossless −20 dB/oct *(no debe detectar)* | sin corte ✓            | sin corte ✓        |
| MP3 128 sobre mezcla −9                  | 15 994 Hz ✓            | 15 994 Hz ✓        |
| MP3 128 sobre mezcla −12                 | **se escapa**          | 16 290 Hz ✓        |
| MP3 128 sobre mezcla −15                 | **se escapa**          | 15 994 Hz ✓        |
| MP3 128 sobre mezcla −20                 | se escapa              | se escapa          |

Relajar el umbral de entrada a `ref_level − 70` recupera las mezclas de −12 y −15 dB/oct **sin producir ni un corte falso en los cuatro casos lossless**. El de −20 dB/oct es genuinamente indetectable —a 16 kHz ya está por debajo del piso de dither— y eso es honesto decirlo: ahí la respuesta correcta es *Inconcluso*, no un veredicto.

La otra cara: en la rama de roll-off natural, el ancho de banda que se reporta sigue siendo el del contenido útil, no Nyquist. Una mezcla oscura legítima muestra «8 376 Hz» y, como no recibe el −1,8 de E01, tampoco puede alcanzar «Lossless verificado». Con E01 decidiendo por rama (§02) esto se resuelve solo: sin borde detectado, el archivo no tiene corte artificial y merece su exoneración, mostrando el dato como *ancho de banda de contenido útil* y el corte como *ninguno detectado*. Son dos datos distintos y hoy se están mezclando en uno.

## 04. Lo que sigue abierto

| Sev        | Asunto                                       | Estado y detalle                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
|------------|----------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| \*\*P0\*\* | El IPC sigue sin conectar                    | Tercera pasada y sigue igual: fuera de `bdja_worker`, ningún crate importa `bdja_ipc`. El decode sigue en el proceso de la app con `panic = "abort"`, así que un archivo corrupto de un USB ajeno cierra la aplicación en mitad de un escaneo. El supervisor además **no tiene timeout**. Sigue pendiente tu decisión: conectarlo de verdad, o retirarlo y usar `catch_unwind`. Cualquiera de las dos; la que no vale es dejarlo a medias dentro del instalador.                              |
| \*\*P1\*\* | El token de capacidad, dos matices           | El `hwid` lo aporta el llamante dentro del propio token y el motor no deriva el identificador de la máquina por su cuenta, así que quien conozca el salt puede firmar un token para cualquier equipo. Y el salt está en texto plano *en los dos lados* (`license_manager.dart` y `api.rs`). Además la comparación de digests usa `!=` sobre `String`, no una comparación en tiempo constante. Es una mejora real respecto a «cualquier cadena no vacía», pero es ofuscación, no verificación. |
| \*\*P1\*\* | La cola de reportes pendientes no tiene tope | `pending_reports` sigue creciendo sin límite entre *polls*. A ~1,5 KB por reporte y 100 000 archivos son cientos de megas si la UI se queda atrás o pasa a segundo plano.                                                                                                                                                                                                                                                                                                                     |
| \*\*P1\*\* | Sin reanudación                              | No hay tabla `scan_job` ni checkpoints. La caché evita repetir el análisis, pero el recorrido del disco se rehace desde cero y el trabajo «en curso» no se recupera. Tampoco hay `PRAGMA user_version`: las migraciones siguen siendo `ALTER TABLE` con el error ignorado.                                                                                                                                                                                                                    |
| \*\*P1\*\* | El recorrido sigue en dos fases              | `scan_collection` descubre todo el árbol antes de analizar el primer archivo, y no reporta progreso durante el descubrimiento. En un disco completo son minutos con la barra en cero.                                                                                                                                                                                                                                                                                                         |
| \*\*P1\*\* | Los errores no se muestran                   | Los archivos que fallan al decodificar se descartan en silencio (`Err(_) => return`). El DJ debería ver «38 archivos no se pudieron leer» con la lista, porque a veces *eso* es el dato: un archivo corrupto en su biblioteca.                                                                                                                                                                                                                                                                |
| \*\*P1\*\* | Lo que se llama LUFS no es LUFS              | `quality.rs` sin cambios: sigue siendo RMS con el offset de R128 encima, sin K-weighting ni *gating*. Y las métricas de nivel se siguen calculando sobre `take(4)` de ventanas —0,74 segundos— y sobre la mezcla mono, que esconde el clipping de un solo canal. Para una herramienta que promete «la calidad real», el LUFS y el true peak son datos que el productor va a comparar con su DAW.                                                                                              |
| \*\*P1\*\* | E05, E06, E09 y E14 sin cambios              | E05 sigue autocorrelando 4 ventanas concatenadas de tramos distintos de la canción (peor ahora que el *seek* funciona). E06 sigue con umbral absoluto, no invariante a ganancia. E09 sigue siendo un placeholder (`zero_lsb_ratio = 0.0` y nadie lo lee). E14 sigue midiendo la dinámica de la música en lugar de la consistencia de las evidencias, y penaliza un 50 % a las pistas dinámicas.                                                                                               |
| \*\*P1\*\* | Umbrales atados a 44,1 kHz                   | Las fronteras de E01 (16 500 / 18 500 / 19 800 / 20 500) y los puntos de E03 (14/16/18 kHz) están escritos suponiendo 44,1 kHz. En un archivo de 48 o 96 kHz significan otra cosa. Deben expresarse como fracción de Nyquist o tener tabla por sample rate.                                                                                                                                                                                                                                   |
| \*\*P1\*\* | Calibración                                  | `bdja_cli` sigue sin `calibrate` ni `validate`. Los pesos LLR siguen escritos a mano y la tasa de falsos positivos sigue sin medirse. Es la brecha más grande que queda después del §02.                                                                                                                                                                                                                                                                                                      |
| \*\*P2\*\* | Código muerto y duplicado                    | `analyze_file_quick` sigue siendo un alias literal de `analyze_file`. `scan_directory` (la vieja secuencial) sigue en `scanner.rs` sin que nadie la llame. `blake3` se calcula y se guarda pero no se usa para dedupe. `rubato` sigue declarado en `bdja_dsp`; conviene comprobar si se usa.                                                                                                                                                                                                  |
| \*\*P1\*\* | Nada de CI, firma, notarización ni fuzzing   | No hay `.github/workflows/`. `distribution/installer.iss` está, pero falta el certificado EV de Windows, el Developer ID y la notarización de Apple, el empaquetado de macOS, `cargo-deny`/`cargo-audit`/SBOM y `cargo-fuzz`. Con 10 tests que ya cubren cosas reales, la CI mínima se paga sola.                                                                                                                                                                                             |

## 05. ¿Es código a nivel senior?

El detector de borde de esta pasada sí, y con margen. Las tres ramas explícitas, la comprobación de supresión sostenida para no confundir un *notch* con un corte, el barrido con *stride* y el manejo del margen de Nyquist son decisiones de alguien que entiende el problema. El token con HMAC, el índice de caché sobre la tupla completa y el test de punta a punta del veredicto también.

Lo que un revisor senior seguiría marcando, y es siempre lo mismo, son **los bordes entre módulos**:

- **Se cambió un productor sin revisar a su consumidor.** El detector devuelve una frecuencia que ahora significa algo distinto —antes «hasta dónde llega la energía», ahora «dónde está el corte»— y `pipeline.rs` la sigue interpretando con la semántica vieja. El tipo de dato no cambió, así que el compilador no dijo nada. Eso es exactamente lo que un `enum CutoffKind` habría impedido: cuando el significado cambia, el tipo debe cambiar con él.
- **Infraestructura a medio conectar.** Tercera pasada con `bdja_ipc` completo y sin usar. Un revisor lo bloquearía solo por eso: el repo afirma una protección que no existe.
- **Un motor de decisión sin calibrar.** Sigue siendo cierto y sigue siendo lo que separa «funciona» de «los números son defendibles».

## 06. Qué falta exactamente para producción

T1 — La corrección del §02 y §03medio día

`CutoffKind` en `SpectrumAnalysis`, E01 decidiendo por rama, la condición `applicable` de E02 fuera, y el umbral de entrada del detector relajado a `ref−70`. Umbrales expresados como fracción de Nyquist mientras estamos ahí.

**Criterio de aceptación**Cuatro tests nuevos que hoy fallarían: MP3 320 con corte a 20,5 kHz da *Probable transcode* o *Sospechoso*; MP3 128 sobre mezcla de −15 dB/oct se detecta; una mezcla oscura legítima alcanza *Lossless verificado*; y un archivo de 48 kHz se evalúa con los umbrales correctos.

T2 — Decidir el IPC y cerrar seguridad1 día

Conectar los workers con timeout y tope de memoria, o retirar `bdja_ipc` y usar `catch_unwind` con `panic = "unwind"`. Que el motor derive el HWID por su cuenta en vez de aceptarlo del llamante, y comparación de digests en tiempo constante.

**Criterio de aceptación**Un archivo deliberadamente corrupto no interrumpe un escaneo en curso, y un token firmado para otro equipo es rechazado en este.

T3 — Corpus y calibración3-4 días

`bdja_cli calibrate` y `validate`, matriz de transcodes con tus masters, pesos ajustados contra datos y los gates en CI. Es lo único que no puedo empezar sin ti.

**Criterio de aceptación**Tasa de falsos positivos \< 1 % sobre lossless genuino y recall \> 95 % en MP3 ≤ 320 kbps, medidos por un comando reproducible.

T4 — Escaneo de grado producción2 días

Walk y análisis solapados con cola acotada y progreso desde el primer segundo, tope en la cola de reportes, dedupe con el blake3 que ya se calcula, `scan_job` con checkpoints, `user_version`, y los errores de decodificación visibles en la UI.

**Criterio de aceptación**50 000 archivos con la app matada dos veces: reanuda sin repetir ni perder, RSS por debajo de 250 MB, y la barra se mueve desde el primer segundo.

T5 — Precisión y distribución4-5 días

`ebur128` para LUFS y true peak conformes y métricas sobre la pista entera y por canal, E05 por bloque contiguo, E06 normalizado, E09 por entropía de LSB, E14 sobre evidencias, limpieza del código muerto. Ficha técnica exportable, i18n es/en, CI, firma EV, Developer ID con notarización, empaquetado de macOS, `cargo-deny`/`audit`/SBOM y `cargo-fuzz`.

**Criterio de aceptación**Instalador firmado que arranca limpio en un Windows 10 nuevo y en un macOS 12 sin Xcode, cero warnings de clippy y cero code smells.

**Total: 10-13 días.** Pero fíjate en el reparto: T1 es medio día y es lo único que separa «el motor miente en el caso más importante» de «el motor acierta». Lo haría hoy mismo, antes de cualquier otra cosa.

## 07. Funcionalidades que le veo

Reescritas con tu criterio: darle al DJ y al productor el dato y la calidad real, y nada más.

| Funcionalidad                                    | Por qué le sirve al DJ o al productor                                                                                                                                                                                                                                                                              | Valor / esfuerzo |
|--------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|------------------|
| **Ficha técnica exportable del archivo**         | Una página por pista con el formato real, el ancho de banda, el corte detectado o su ausencia, las evidencias, LUFS y true peak, y el espectro. Sirve para decidir si el archivo entra a la colección o no, y para compararlo con otra versión del mismo tema. Es un dato técnico, no un documento para nadie más. | alto / bajo      |
| **Comparación A/B**                              | Arrastrar dos versiones del mismo tema y ver las dos curvas superpuestas con la tabla de diferencias. Es lo que un productor hace mentalmente todo el día, y aquí sale en dos segundos: cuál de las dos copias que tiene es la buena.                                                                              | altísimo / medio |
| **Vigilancia de carpeta**                        | Analizar automáticamente lo que entra en la carpeta de descargas y avisar solo si algo sale por debajo de lo declarado. El DJ se entera en el momento, no tres meses después en cabina.                                                                                                                            | alto / medio     |
| **Leer las bibliotecas de Serato y Rekordbox**   | Analizar «mi biblioteca» tal como él la ve, con el resultado por crate o playlist, en lugar de pedirle que elija carpetas. Nadie en este espacio lo hace y ya tienes hecha la parte difícil: reconocer y respetar sus metadatos. Solo lectura, nunca escribir en sus bases.                                        | altísimo / alto  |
| **Espectrograma y modo Forensic**                | Ahora que el espectro es real tiene sentido: el productor quiere ver, no solo leer un veredicto. Era la v1.1 del plan.                                                                                                                                                                                             | medio / medio    |
| **Vista de duplicados**                          | Ya calculas el blake3 de cada archivo y no lo usas. Agrupar duplicados y mostrar cuál de las copias tiene mejor calidad real es una función casi gratis y que resuelve un problema que todo DJ tiene.                                                                                                              | alto / muy bajo  |
| **Detección de Opus y WavPack**                  | Symphonia no decodifica Opus, así que hoy un Opus transcodeado a WAV se te escapa entero. Requiere un decoder aparte o FFmpeg como plugin opcional.                                                                                                                                                                | medio / medio    |
| **Control de calidad del catálogo de BDJ LATAM** | Pasar cada remix por el motor antes de publicarlo, como proceso interno tuyo. Te da el dato antes de que lo tenga el cliente, y de paso el corpus de calibración más realista que existe.                                                                                                                          | estratégico      |

## 08. Mejoras que propongo

### Del motor

- **`CutoffKind` en lugar de una frecuencia suelta.** Es el arreglo del §02 y además es la lección estructural: cuando un dato cambia de significado, que cambie de tipo, para que el compilador obligue a revisar a todos sus consumidores.
- **Separar «ancho de banda de contenido» de «corte detectado».** Hoy son un solo número y eso es lo que produce el mensaje «espectro completo sin corte artificial» sobre un archivo en el que se acaba de detectar un corte. Son dos datos y el usuario entiende perfectamente la diferencia.
- **Umbrales relativos a Nyquist**, no números de 44,1 kHz.
- **LUFS y true peak de verdad** con `ebur128`, sobre la pista completa y por canal. Si la app promete «la calidad real», estas dos cifras son las que el productor va a contrastar con su DAW, y si no cuadran pierde la confianza en todo lo demás.
- **Mostrar el `engine_rev`** junto a cada veredicto, con opción de reanalizar cuando haya motor nuevo. Es lo que evita que un veredicto viejo se quede como si fuera actual.

### Del producto

- **Normal como intensidad por defecto**, no Turbo. Ya tienes el selector; el valor inicial debería ser el que no acapara la máquina del DJ.
- **Mostrar los archivos que fallaron**, con el motivo. Es información útil, no un error que haya que esconder.
- **Que «Inconcluso» se lea como un resultado.** El caso de −20 dB/oct del §03 es real: hay material donde no se puede saber, y decirlo es lo que hace creíble al resto.
- **Publicar las cifras cuando T3 esté hecho.** Recall por códec y tasa de falsos positivos, en la app y en la web. Es el tipo de dato que ningún competidor de este espacio ofrece.

### Del proceso

- **Un test de caso de negocio antes de cada arreglo.** Las tres pasadas han seguido el mismo patrón: el arreglo resuelve lo señalado e introduce el hallazgo siguiente en el borde con el módulo vecino. El test que falla primero es lo único que rompe ese ciclo.
- **CI mínima ya**: `fmt`, `clippy -D warnings`, `cargo test`, `flutter test`. Tienes 10 tests que cubren cosas reales; que corran solos en cada *push*.
- **Cerrar cada tanda contra su criterio de aceptación** antes de abrir la siguiente.

## 09. Siguiente paso

Si me dices que sí, hago **T1 ahora**: es medio día, son cuatro cambios en dos archivos, y viene con los cuatro tests que hoy fallarían. Es lo que convierte al motor en correcto en el caso que define el producto.

Y sigo necesitando dos cosas de ti:

1.  **La decisión del IPC**, que arrastra tres pasadas: ¿workers de verdad con timeout, o retirarlo y usar `catch_unwind`? Las dos son válidas.
2.  **El corpus**: 200-400 masters lossless tuyos, variados, incluyendo a propósito mezclas oscuras y material band-limited legítimo. Es lo único que no puedo hacer yo, y sin él los porcentajes de confianza que muestra la app siguen siendo una estimación en vez de una medida.


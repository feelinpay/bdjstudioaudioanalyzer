# BDJ Studio Audio Analyzer — Auditoría de código v1.0

> Auditoría del 12 de septiembre de 2026 · 10 crates Rust y 12 módulos Dart contrastados con PLAN_ARQUITECTURA.md
> 5 hallazgos bloqueantes verificados, 9 de precisión, 11 de producto. Versión navegable publicada como artifact.

BDJ Studio · Auditoría técnica de código

# Auditoría del Audio Analyzer

Revisión del código real que hay hoy en el repositorio, contrastado línea por línea con el plan de arquitectura. El esqueleto está bien construido y compila; el motor de detección, en cambio, hoy no llega a ejecutarse. Aquí está todo, ordenado por lo que rompe el producto primero.

**Revisado** 12 sep 2026 **Alcance** 10 crates Rust · 12 módulos Dart · 192 KB de fuente **Estado** compila y arranca · release dll presente **Hallazgos** 5 bloqueantes · 9 de precisión · 11 de producto

## 0. Resumen en una pantalla

AndamiajeSólidoworkspace de 10 crates, FFI generado, licencia SPP3 real, UI navegable, compila en release

Motor de veredictoNo se ejecutaun bug de una línea manda todos los archivos a «Con pérdida declarado»

Escaneo masivoTope de 1 000un hilo, sin caché, sin reanudar, sin progreso real ni cancelación

### Estado de las 21 secciones del plan

  

7 implementadas 8 parciales 6 ausentes

La forma de decirlo sin adornos: **alguien construyó muy bien la carrocería y el motor está desconectado del volante**. Todo lo estructural del plan está ahí y bien puesto — la separación en crates, el catálogo de las 14 evidencias con sus códigos, los 6 estados de veredicto, la puerta de evidencia fuerte, las guardas, el licenciamiento SPP3 con HWID V2 real. Lo que no está es lo que decide si el producto es creíble: el cálculo del ancho de banda no funciona sobre música real, el análisis mira solo los primeros 6 segundos de cada pista, el espectro que se dibuja en pantalla es inventado, y el veredicto nunca se calcula porque la detección de códec falla antes.

Lo bueno de esta noticia

Cuatro de los cinco bloqueantes son bugs puntuales, no errores de diseño: se arreglan en archivos concretos sin tocar la arquitectura. El quinto (escaneo masivo real) es trabajo de implementación que ya tiene su sitio reservado en el diseño. No hay nada que haya que rehacer desde cero.

## 01. Bloqueantes

Cinco cosas que impiden que la aplicación haga hoy aquello para lo que existe. En este orden.

#### \*\*P0-1\*\* El veredicto nunca se calcula: todos los archivos salen como «Con pérdida declarado» \`bdja_decode/src/decoder.rs\`

El códec se convierte a texto con `format!("{:?}", codec_params.codec)` y después se busca `"Pcm"`, `"Flac"` o `"Alac"` dentro de ese texto para decidir `is_lossless_declared`.

Pero `CodecType` en Symphonia es una tupla sobre un `u32` (`pub struct CodecType(u32)`, verificado en el fuente de `symphonia-core 0.5.4`), y su `Debug` no imprime un nombre: imprime el número. `CODEC_TYPE_PCM_S16LE` es `CodecType(0x108)`, así que el texto que sale es literalmente `"CodecType(264)"`, y FLAC es `"CodecType(8192)"`.

Consecuencia en cadena: `is_lossless_declared` es **siempre falso** → `evaluate_verdict` entra por su primera rama y devuelve `DeclaredLossy` con confianza 0,99 → las 14 evidencias se calculan y se tiran a la basura → y en la ficha del archivo el usuario lee *«Códec: CodecType(264)»*. Se comprueba en 30 segundos: arrastra cualquier WAV y mira el veredicto.

**Arreglo**Mapear explícitamente las constantes de Symphonia (`CODEC_TYPE_PCM_*`, `CODEC_TYPE_FLAC`, `CODEC_TYPE_ALAC`, `CODEC_TYPE_MP3`, `CODEC_TYPE_AAC`, `CODEC_TYPE_VORBIS`…) a un `enum Codec` propio en `bdja_core`, con nombre legible y un `is_lossless()` derivado del enum. Nunca decidir lógica de negocio sobre el resultado de un `Debug`: es un detalle de implementación de terceros que puede cambiar en cualquier versión. Añadir un test por cada códec soportado.

#### \*\*P0-2\*\* El ancho de banda efectivo no discrimina sobre música real \`bdja_dsp/src/spectrum.rs\`

E01 se calcula como la frecuencia donde la energía acumulada alcanza el 99,5 % del total. En música real la energía está concentrada en los graves, así que ese punto se alcanza muchísimo antes del corte verdadero. Lo reproduje con el mismo algoritmo, mismas ventanas de 8192 y mismo Hann:

| Señal de prueba                                            | Algoritmo actual | LLR E01 | Corregido |
|------------------------------------------------------------|------------------|---------|-----------|
| Ruido blanco *(el único caso que cubre el test existente)* | 21 942 Hz        | −1,8    | 22 050 Hz |
| Ruido rosa (−3 dB/oct)                                     | 21 199 Hz        | −1,8    | 22 050 Hz |
| Música típica −6 dB/oct · **lossless**                     | 13 916 Hz        | +2,2    | 22 050 Hz |
| Música típica −9 dB/oct · **lossless**                     | 4 226 Hz         | +2,2    | 22 050 Hz |
| Música típica −12 dB/oct · **lossless**                    | 2 406 Hz         | +2,2    | 22 050 Hz |
| La misma −9 dB/oct pero **MP3 128** (corte real 16 kHz)    | 4 161 Hz         | +2,2    | 15 918 Hz |
| La misma −9 dB/oct pero **MP3 192** (corte real 19 kHz)    | 4 382 Hz         | +2,2    | 18 912 Hz |

Dos lecturas de esa tabla. Primera: un master lossless legítimo y su versión MP3 128 dan **el mismo número** — E01 tiene cero poder discriminante y se queda clavado en «+2,2 acusar». Segunda: el único test que existe usa ruido blanco, que es plano, y por eso pasa. El test valida el bug.

Y esto arrastra a E04, la evidencia más fuerte del catálogo: los huecos espectrales solo se calculan `if bin_10k < cutoff_bin`, y con un cutoff de 4 kHz esa condición es falsa, así que E04 devuelve 0,0 y aporta **−1,4 exonerando**. El motor acusa y absuelve el mismo archivo por dos vías distintas.

**Arreglo (validado)**El corte no es un percentil de energía, es el borde del contenido: frecuencia más alta cuyo nivel suavizado (~200 Hz) sigue por encima de una referencia — el máximo de la banda 1-6 kHz menos ~45 dB, o mejor el piso de ruido del propio archivo más un margen — exigiendo 3 bins consecutivos para no morder un pico aislado. Esa es la columna «Corregido» de la tabla: lossless da Nyquist, MP3 128 da 15,9 kHz y MP3 192 da 18,9 kHz. Y el test hay que rehacerlo con señales con forma de música, no con ruido blanco. *Esto es también culpa del plan: la §07 decía «percentil 99,5 % de energía acumulada» y la implementación lo siguió al pie de la letra. La corrijo en el plan.*

#### \*\*P0-3\*\* Solo se analizan los primeros ~6 segundos de cada pista \`bdja_decode/src/decoder.rs\`

No hay *seek*. El decodificador arranca en el byte cero, va acumulando y para cuando `all_mono` llega a `8192 × 32 = 262 144` muestras: **5,9 segundos** a 44,1 kHz. Las «12 ventanas estratificadas» se reparten dentro de esos 6 segundos, no a lo largo de la canción.

En música de DJ eso es lo peor que se puede elegir: los primeros segundos son intro, fade-in, un filtro cerrado, a veces silencio o una voz sola. Un remix con intro filtrada se va a leer como band-limited, y un transcode cuyo primer tramo es tranquilo se va a escapar. Además convierte a E14 («consistencia temporal a lo largo de toda la pista») en una medida sobre 6 segundos, o sea en nada.

**Arreglo**`format.seek()` a 12 puntos repartidos por la duración real (p. ej. del 5 % al 95 %), decodificar solo ~16 384 muestras en cada punto y descartar los tramos silenciosos *después* de haberlos visitado, no antes. Si el formato no permite *seek* preciso, streaming con descarte y presupuesto de tiempo. Es exactamente lo que describe la §06 del plan y es lo que da sentido al muestreo estratificado.

#### \*\*P0-4\*\* El espectro que se muestra en pantalla está inventado \`bdja_ffi/src/api.rs · generate_spectrum_curve()\`

El espectro real se calcula bien en `spectrum.rs` (`average_spectrum_db`, 256 puntos) pero **nunca sale del motor**: no viaja en `FileReport`. En su lugar, la capa FFI genera la curva con una fórmula cerrada a partir del cutoff y la pendiente: `-12.0 - 18.0 * (f / cutoff)` antes del corte y una recta por octavas después.

Es decir: la gráfica que el DJ mira para «ver la evidencia» es un dibujo idealizado que no contiene ni una muestra de su audio. Dos archivos distintos con el mismo cutoff estimado producen exactamente la misma curva. Para un producto cuya credibilidad es *«mira el espectro»*, y cuyo reporte se va a usar para reclamarle a un proveedor, esto no puede existir en ninguna versión, ni como *placeholder*.

**Arreglo**Llevar `average_spectrum_db` desde `DspOutput` a `FileReport`, persistirlo en SQLite (256 `f32` son 1 KB por archivo; como BLOB o JSON comprimido) y pasarlo por FFI. Borrar `generate_spectrum_curve` por completo — y si un reporte viejo no tiene espectro guardado, la UI dibuja el hueco y dice «sin espectro almacenado», nunca una curva sintética.

#### \*\*P0-5\*\* El escaneo masivo es un hilo, con tope de 1 000 archivos, sin caché ni reanudación \`bdja_scan/src/scanner.rs · bdja_ffi/src/api.rs · home_screen.dart\`

El requisito central era discos, USB y grandes volúmenes. Lo que hay:

- `scanDirectoryAudio(rootPath, maxFiles: 1000)` — tope de mil archivos, fijado en la UI.
- El bucle de análisis es `for path in found_files`, secuencial: **un solo núcleo**. `crossbeam-channel` y `blake3` están declarados como dependencias y no se usan.
- Primero recorre *todo* el árbol a un `Vec` y luego analiza: sin backpressure, y el usuario no ve nada durante el recorrido (en un HDD completo, minutos en blanco).
- Sin consulta de caché: `analyze_single_file` nunca mira la base de datos. El objetivo de «re-análisis ≤ 2 % del tiempo» no se cumple; cuesta el 100 %.
- Sin `scan_job` ni checkpoints: no se puede reanudar. La tabla no existe en el esquema.
- El progreso de carpeta es falso: `_totalToAnalyze = 100` fijo y `_analyzedCount` nunca se incrementa en esa ruta. La barra se queda en 0 % y salta al final.
- «Cancelar» solo hace `setState(() => _isAnalyzing = false)`: el trabajo en Rust sigue hasta terminar. El `cancel_token` existe en `scan_directory`, pero esa función no es la que usa la app.
- `ScanEvent`, `ScanOptions` y los modos de throttling están definidos en `bdja_core` y son **código muerto**: nada los emite ni los consume.

**Arreglo**Sustituir la llamada bloqueante por el par `scan_start` + `scan_events(job_id) -> Stream<ScanEvent>` del contrato de la §12: walk y análisis solapados por una cola acotada, pool de `cores−1`, huella blake3 para caché y dedupe, checkpoint en SQLite cada 200 archivos, token de cancelación consultado en el walk, en la cola y entre ventanas. Sin tope de archivos.

## 02. Precisión del motor

Nueve defectos que no impiden que la app funcione, pero que hacen que sus números no signifiquen lo que dicen significar. Ordenados por cuánto ensucian el veredicto.

| Sev        | Qué                                                      | Dónde                               | Por qué importa                                                                                                                                                                                                                                                                                                                                                                                                                                           |
|------------|----------------------------------------------------------|-------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| \*\*P1\*\* | E14 mide la cosa equivocada                              | temporal.rs · verdict/engine.rs     | `temporal_variance` es el coeficiente de variación de la *energía* entre ventanas, o sea la dinámica de la música. El plan pedía la varianza de *las evidencias* entre tramos. Resultado: una pista dinámica (intro suave, drop fuerte) se penaliza un 50 % aunque el transcode sea evidente, y un master de loudness-war plano nunca se penaliza.                                                                                                        |
| \*\*P1\*\* | E05 autocorrela ventanas no contiguas                    | temporal.rs                         | Concatena 4 ventanas de 8192 de posiciones distintas y autocorrela el resultado buscando periodicidad de 576/1024. Cada unión introduce una discontinuidad artificial. La rejilla de tramas hay que buscarla **dentro** de cada bloque contiguo y promediar los picos, no en la concatenación.                                                                                                                                                            |
| \*\*P1\*\* | E06 no es invariante a ganancia                          | temporal.rs                         | El pre-eco se detecta con umbrales absolutos (`energy_post > 0.05`) sobre sumas de cuadrados sin normalizar. Una pista mezclada 6 dB más baja nunca dispara E06. El plan lo tenía como propiedad verificable con proptest («invariante a ganancia»); ese test no existe.                                                                                                                                                                                  |
| \*\*P1\*\* | E07 no separa por banda                                  | stereo.rs                           | La «correlación de banda alta» es la correlación de la primera diferencia de la señal completa, no de una banda por encima de un cruce. Y el cruce reportado es `Some(14000)` fijo, inventado. Sirve como indicio grueso; no como una de las cuatro evidencias fuertes que autorizan a acusar.                                                                                                                                                            |
| \*\*P1\*\* | Lo que se llama LUFS no es LUFS                          | quality.rs                          | `-0.691 + 10·log10(mean²)` es RMS con el offset de R128 pegado encima: sin filtro K-weighting y sin *gating* de −70/−10 LU. Puede desviarse varios dB del valor real, que es justo la cifra que un DJ compara contra el objetivo de una plataforma. El plan especificaba la crate `ebur128`; no está en las dependencias. Lo mismo con el true peak, que es `pico × 1,05` — un 0,4 dB a ojo, sin el sobremuestreo 4× que el propio comentario dice hacer. |
| \*\*P1\*\* | Métricas de nivel calculadas sobre 0,7 s y en mono       | dsp/pipeline.rs · decoder.rs        | `analyze_quality` recibe la concatenación de 4 ventanas: 32 768 muestras ≈ 0,74 s. LUFS integrado, rango dinámico y piso de ruido de una pista entera se calculan sobre menos de un segundo. Y el clipping y el pico se miden sobre la mezcla mono `(L+R)/2`, que atenúa picos y esconde clipping de un solo canal.                                                                                                                                       |
| \*\*P1\*\* | E09 (bit depth real) no está implementado                | decoder.rs · quality.rs             | `BitDepthStats` se rellena con `estimated_real_bits = declared` y `zero_lsb_ratio = 0.0`: código muerto. La estimación real la hace `quality.rs` a partir de la muestra no nula más pequeña, que es una medida de un solo valor extremo, no un piso. El plan pedía la entropía y distribución de los LSB, que es lo que de verdad detecta un 24 bits inflado.                                                                                             |
| \*\*P1\*\* | Archivo silencioso o muy bajo → «Probablemente lossless» | decoder.rs · spectrum.rs            | Las ventanas con `energy <= 0.0001` se descartan; si se descartan todas, `analyze_spectrum` devuelve sus valores por defecto, que son *ancho de banda = Nyquist* y *0 huecos*. Suma ≈ −3,2 → veredicto «Probablemente lossless» con 70-85 % de confianza sobre un archivo del que no se midió nada. Falla abriendo. Debe fallar cerrando: *Inconcluso* explícito con la guarda correspondiente.                                                           |
| \*\*P2\*\* | Dos fuentes de verdad para «evidencia fuerte»            | dsp/pipeline.rs · verdict/engine.rs | El DSP devuelve un `bool is_strong_evidence_present` y el motor de veredicto recalcula su propia lista con `llr >= 1.5`. Pueden discrepar. Además `E13.value` se rellena con ese `bool` global, así que un archivo sin ningún hallazgo de metadata puede guardar `E13 = 1.0` en el reporte exportado. Una sola función debe decidirlo, en `bdja_verdict`.                                                                                                 |

## 03. Los falsos positivos que te van a costar clientes

Esta sección la separo porque no es deuda técnica: es el escenario concreto en el que la app acusa a un DJ honesto delante de su proveedor. Vale más que cualquier optimización.

#### \*\*P0-6\*\* Un WAV convertido con FFmpeg se marca como transcode probable \`bdja_decode/src/forensic.rs\`

El análisis forense busca la cadena `"Lavf"` en los primeros 64 KB del archivo. Si la encuentra, rellena `encoder_string`. Y en `dsp/pipeline.rs`, E13 es: `if has_xing_lame || encoder_tag.is_some()` → **LLR +3,5 y evidencia fuerte**, que es la puerta que autoriza el veredicto «Probable transcode».

`Lavf` es la firma de libavformat, o sea de FFmpeg. La escribe cualquier conversión hecha con FFmpeg, incluidas las perfectamente legítimas: pasar un master de 24 bits a 16 bits, cambiar 48 kHz a 44,1 kHz, recortar un intro, normalizar. **Nada de eso implica una fuente con pérdida.** Con el bug P0-2 sumando +2,2 por E01, un WAV legítimo pasado por FFmpeg llega a «Probable transcode» con más del 90 % de confianza.

**Arreglo**Separar en tres cosas que hoy están juntas: (a) *cabecera Xing/Info de trama MP3* y *tag LAME3.x* dentro de un contenedor lossless → sí es evidencia fuerte; (b) *nombre de software de conversión* (Lavf, SoX, Audacity, iTunes) → como máximo informativo, LLR 0,0, y se muestra como dato del archivo, no como acusación; (c) *extensión que no corresponde al contenedor real* → fuerte, correcto como está. Y nunca buscar firmas por `find_subsequence` en 64 KB en bruto: hay que parsear el chunk o el bloque de metadata correspondiente, porque una cadena suelta puede aparecer dentro del título de una canción.

#### \*\*P0-7\*\* La biblioteca etiquetada con Serato o Rekordbox se marca como anómala \`bdja_decode/src/forensic.rs · dsp/pipeline.rs\`

`has_anomalous_id3_in_wav` se activa al encontrar `"id3 "` o `"ID3 "` dentro de un RIFF/WAV, y eso aporta LLR +2,0 con el texto «Chunk ID3 anómalo».

El problema: **Serato y Rekordbox escriben chunks ID3 dentro de los WAV** para guardar cue points, beatgrid y metadatos. Es el estado normal de casi cualquier WAV en el disco de un DJ que trabaja. Tu público objetivo es precisamente el que tiene toda su biblioteca así. Un ID3 en un WAV no dice nada sobre compresión previa; es la herramienta de cabina haciendo su trabajo.

**Arreglo**Quitar E13-ID3 de la fusión de procedencia. Reconocer los chunks conocidos de Serato (`Serato Markers2`, `Serato Overview`) y de Rekordbox y mostrarlos como información útil («etiquetado con Serato») — que además es un detalle que a un DJ le gusta ver. Un chunk ID3 solo merece una nota si contiene, dentro, un tag de encoder con pérdida.

#### \*\*P1\*\* Falta la guarda más importante del plan: material band-limited por origen \`dsp/pipeline.rs\`

Hay tres guardas implementadas: duración \< 20 s, nivel muy bajo y sample rate ≤ 32 kHz. Falta justo la que el plan marcaba como crítica: **contenido con poca energía de alta frecuencia por su propia naturaleza** — vinilo, cinta, grabaciones antiguas, AM, una voz sola, pads suaves, un master con lowpass intencional. Sin esa guarda, el catálogo clásico de BDJ LATAM (remixes de temas viejos, ediciones de vinilo) entra directo a la zona de acusación.

**Arreglo**Guarda por *forma* del espectro, no por nivel: si por encima del corte estimado la caída es progresiva en lugar de vertical (E02 bajo), o si la energía entre 8 y 16 kHz ya es muy baja respecto a la banda media *sin* borde definido, el techo del veredicto es Sospechoso salvo que aparezca E04, E05 o E13-fuerte.

## 04. Brechas contra el plan

Las 21 secciones del plan, una por una, con lo que hay hoy.

| §   | Sección del plan          | Estado          | Qué falta concretamente                                                                                                                                                                                                                                                                                             |
|-----|---------------------------|-----------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| 01  | Requisitos y presupuestos | \*\*Parcial\*\* | Ningún presupuesto de §01 se mide. No hay benchmarks ni test de latencia; el tope de 1 000 archivos hace inalcanzable el objetivo de 50 000.                                                                                                                                                                        |
| 02  | Decisiones (ADR)          | \*\*Ausente\*\* | No existe `docs/adr/`. Tres ADRs se incumplen en el código (06 workers aislados, 08 muestreo estratificado, 10 gate de capacidad).                                                                                                                                                                                  |
| 03  | Stack tecnológico         | \*\*Parcial\*\* | Symphonia 0.5 en vez de 0.6.1. Faltan `ebur128`, `rubato`, `memmap2`, `zeroize`, `rayon`, `pdf`. `crossbeam` y `blake3` declarados pero sin usar. `flutter_lints` en vez de `very_good_analysis`, y `deprecated_member_use: ignore` global.                                                                         |
| 04  | Topología de tres anillos | \*\*Ausente\*\* | `bdja_worker` imprime su versión (77 bytes) y `bdja_ipc` está vacío (21 bytes). Todo decodifica en el proceso de la app. Con `panic = "abort"` en release, un archivo malformado de un USB tumba la aplicación entera en mitad de un escaneo.                                                                       |
| 05  | Workspace de 10 crates    | \*\*Hecho\*\*   | Los 10 crates existen con los nombres y las dependencias correctas. `bdja_cli` es un stub de 74 bytes, así que no hay herramienta de calibración.                                                                                                                                                                   |
| 06  | Pipeline de 7 etapas      | \*\*Parcial\*\* | Etapas 1, 2, 4, 5 y 6 están. Falta la 0 (triage con caché) y la 3 está mal (sin *seek*, ver P0-3). Ninguna salida temprana por caché.                                                                                                                                                                               |
| 07  | Catálogo de 14 evidencias | \*\*Hecho\*\*   | Las 14 existen con sus códigos, pesos y textos. La calidad de cada medida es otra cosa: ver §02 y §03 de este informe.                                                                                                                                                                                              |
| 08  | Motor de veredicto        | \*\*Hecho\*\*   | Los 6 estados, los umbrales, la puerta de evidencia fuerte y el veto de guardas están implementados fielmente. Hoy es inalcanzable por P0-1.                                                                                                                                                                        |
| 09  | Calibración y dataset     | \*\*Ausente\*\* | Sin corpus, sin `bdja calibrate`, sin gates de CI. Los pesos LLR son valores escritos a mano. **Esta es la brecha más grande que queda después de arreglar los P0.**                                                                                                                                                |
| 10  | Escaneo masivo            | \*\*Parcial\*\* | Enumeración de volúmenes en Windows: bien hecha, con `GetDriveTypeW` y exclusión de CD. Todo lo demás: ver P0-5. Sin rutas largas `\\?\`, sin protección de ciclos/symlinks.                                                                                                                                        |
| 11  | Persistencia              | \*\*Parcial\*\* | Una tabla plana en vez del esquema de 5 tablas. Sin `mtime` ni huella → sin caché. Sin `scan_job` → sin reanudación. Sin migraciones ni `user_version`: el próximo cambio de esquema rompe las bases existentes. Filtros construidos por interpolación de strings en el SQL (escapados, pero deben ser parámetros). |
| 12  | Contrato FFI              | \*\*Parcial\*\* | Hay 11 funciones en vez de las 9 del contrato, y falta la clave: `scan_events` como `Stream`. `analyze_file_quick` es un alias literal de `analyze_file` — la duplicación que pediste no tener. Sin `verify_contracts.py`.                                                                                          |
| 13  | Interfaz                  | \*\*Parcial\*\* | Activación, home con arrastre, unidades listadas, tabla de resultados, ficha de evidencias y export CSV/JSON: hechos y con buen aspecto. Faltan reporte PDF «Verify Remix», i18n es/en (no hay `app_strings`), progreso real, cancelación real y los modos Simple/Analyzer/Forensic.                                |
| 14  | Licenciamiento SPP3       | \*\*Hecho\*\*   | `BdjProduct.audioAnalyzer` añadido al paquete compartido, verificación real con `Spp3Token.verify`, HWID V2, almacén seguro, control de reloj hacia atrás. Dos pegas: el gate de capacidad es falso (§05) y no hay revalidación periódica cada 6 h.                                                                 |
| 15  | Seguridad                 | \*\*Ausente\*\* | Sin fuzzing, sin `cargo-deny`/`cargo-audit`, sin SBOM, sin aislamiento de proceso, sin firma ni notarización. Ver §05 de este informe.                                                                                                                                                                              |
| 16  | Bajos recursos            | \*\*Parcial\*\* | `get_fft_processor()` se llama *dentro* de cada análisis: se crea un planner de FFT y se recalculan las tablas Hann de 8192 y 1024 por cada archivo. En un lote de 50 000, 50 000 veces. Un solo núcleo activo. Sin modos de intensidad.                                                                            |
| 17  | Testing                   | \*\*Parcial\*\* | 6 tests en total: 2 de DSP, 2 de veredicto, 2 de UI (uno de los cuales solo comprueba constantes de color). Sin proptest, sin golden/insta, sin fuzz, sin tests de decode, scan o store, sin cobertura. El test de ancho de banda valida el bug P0-2.                                                               |
| 18  | CI/CD y distribución      | \*\*Ausente\*\* | No hay `.github/workflows/` ni `distribution/`. Sin Inno Setup, sin DMG, sin notarización. `tools/build_native.ps1` está bien hecho, pero es un script local de Windows, no un pipeline.                                                                                                                            |
| 19  | Roadmap                   | \*\*Parcial\*\* | Se implementaron partes de F1 a F6 en paralelo en lugar de cerrar fases con criterios de aceptación. Por eso hay UI de F6 encima de un motor de F3 sin validar.                                                                                                                                                     |
| 20  | Riesgos                   | \*\*Ausente\*\* | El riesgo nº1 del plan («falso positivo que hace acusar injustamente») se materializó por dos vías: Lavf y ID3 de Serato. Ver §03.                                                                                                                                                                                  |

### Código muerto y duplicado

Lo separo porque pediste expresamente cero código muerto, redundante o de dobles llamadas:

- `ScanEvent`, `ScanOptions`, `VolumeInfo.id` sin uso real, y `EngineInfo` duplicado entre `bdja_core` y `bdja_ffi` (`EngineInfoFfi`, `VolumeInfoFfi`, `FormatFactsFfi`… seis structs espejo).
- `ENGINE_REV` declarado dos veces, en `bdja_core::types` y en `bdja_ffi::api`: pueden divergir, y de ese número depende la invalidación de caché.
- `analyze_file_quick` → `analyze_file`: alias sin diferencia.
- `bdja_ipc` (21 bytes), `bdja_worker` (77 bytes), `bdja_cli` (74 bytes): tres crates que solo ocupan sitio en el workspace y en el instalador — `build_native.ps1` copia los dos `.exe` a la carpeta de la app.
- `BitDepthStats` se construye con valores fijos y nadie lo lee.
- `bdja_scan::scan_directory` (con su `cancel_token` y sus callbacks de progreso) existe y está razonablemente escrita, pero la app llama a `scan_directory_audio` del FFI, que reimplementa el walk sin cancelación ni progreso. Dos caminos para lo mismo, y el bueno es el que no se usa.
- `frontend/logs/2026-09-11.log`: log de `flutter_rust_bridge_codegen` dejado en el repo (está en `.gitignore`, pero sigue en el disco).

## 05. Seguridad y licenciamiento

#### \*\*P0-8\*\* El gate de capacidad no gatea nada \`license_manager.dart · bdja_ffi/src/api.rs\`

En Dart:

    String deriveCapabilityToken(String hwid, int engineRev) {
      final key = utf8.encode('BDJ_AUDIO_ANALYZER_CAPABILITY_SALT_2026');
      final message = utf8.encode('::::');        // ← hwid y engineRev no se usan
      final hmac = Hmac(sha256, key);
      return hmac.convert(message).toString();
    }

El HMAC se calcula sobre la cadena literal `'::::'`: los parámetros se ignoran. Se ve que era una interpolación que perdió sus variables. El token resultante es **una constante idéntica en todas las máquinas y todas las versiones**.

Y en Rust, `engine_init` solo comprueba `if capability_token.trim().is_empty()`. Cualquier cadena no vacía inicializa el motor. Es decir: el ADR-10 completo — que el `.dll` no sea usable si alguien lo extrae del instalador — hoy se salta escribiendo `"x"`.

**Arreglo**Dart: `Hmac(sha256, salt).convert(utf8.encode('$hwidHash::$engineRev::$ventana'))` con una ventana de tiempo redondeada (p. ej. hora UTC). Rust: recalcular el mismo HMAC con el `deviceHash` que obtiene por su cuenta y comparar en tiempo constante, aceptando la ventana actual y la anterior. Y el salt no debería estar en texto plano en el Dart — al menos ofuscarlo y moverlo al lado nativo, que es más costoso de leer.

| Sev        | Asunto                              | Situación                                                                                                                                                                                                                                   |
|------------|-------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| \*\*P0\*\* | Aislamiento de decodificación       | No existe. Con `panic = "abort"`, un WAV malformado del USB de otro DJ cierra la app y se pierde el escaneo en curso. Es el modelo de amenaza principal de esta aplicación y está sin cubrir.                                               |
| \*\*P0\*\* | Fuzzing del decodificador           | Sin `cargo-fuzz`, sin corpus de archivos mutados. Symphonia en Rust puro reduce mucho el riesgo de ejecución de código, pero no el de pánico o bucle infinito.                                                                              |
| \*\*P1\*\* | Límites y timeouts por archivo      | Solo hay el tope de 2 GB / 3 h y un presupuesto implícito de 4 000 paquetes. Sin timeout por archivo ni tope de RAM.                                                                                                                        |
| \*\*P1\*\* | Base de datos en memoria silenciosa | Si `ReportStore::open` falla, `engine_init` cae a una base en memoria **sin avisar**. El usuario escanea 20 000 archivos, cierra la app y lo pierde todo sin explicación.                                                                   |
| \*\*P1\*\* | SQL por interpolación               | `list_reports` concatena el filtro y la búsqueda en el SQL. Está escapado con `replace("'","''")`, así que no es inyectable de forma trivial, pero deben ser parámetros enlazados; además `%` y `_` en la ruta se comportan como comodines. |
| \*\*P1\*\* | Revalidación de licencia            | Solo al arrancar. El plan pedía cada 6 h de sesión abierta.                                                                                                                                                                                 |
| \*\*P1\*\* | Firma de código                     | Sin certificado EV de Windows ni Developer ID de Apple. Un instalador sin firmar dispara SmartScreen y Gatekeeper, y para un producto de pago eso es letal en la primera impresión.                                                         |
| \*\*P2\*\* | Auditoría de dependencias           | Sin `cargo-deny`, `cargo-audit` ni SBOM. Con licencia `Proprietary` declarada, conviene verificar que ninguna dependencia arrastre GPL.                                                                                                     |
| \*\*OK\*\* | Sin red                             | Correcto: no hay ninguna dependencia de HTTP/TLS en el árbol del motor. Falta el test que lo garantice de forma permanente.                                                                                                                 |
| \*\*OK\*\* | Solo lectura                        | Correcto: el motor abre los archivos del usuario en solo lectura y escribe únicamente en el directorio de datos de la app.                                                                                                                  |

## 06. Rendimiento frente a los presupuestos

| Presupuesto del plan           | Objetivo         | Hoy                   | Causa                                                                                      |
|--------------------------------|------------------|-----------------------|--------------------------------------------------------------------------------------------|
| Lote de 50 000 archivos        | ≤ 5 h            | imposible             | tope de 1 000 archivos en la llamada                                                       |
| Rendimiento de lote            | ≈ 3 arch/s       | ≈ 0,7 arch/s          | un solo hilo (dividido por `cores−1`)                                                      |
| Re-análisis sin cambios        | ≤ 2 % del tiempo | 100 %                 | la caché no se consulta nunca                                                              |
| Latencia de UI durante escaneo | 60 fps           | probablemente OK      | FRB ejecuta en hilo aparte, pero el resultado llega en un solo `setState` con todo el lote |
| Arranque en frío               | ≤ 1,2 s          | sin medir             | —                                                                                          |
| RSS en escaneo                 | \< 250 MB        | sin medir, con riesgo | todas las rutas en un `Vec` y todos los reportes del lote en RAM antes de devolverlos      |
| Coste por archivo              | ≤ 900 ms         | sin medir             | se paga un planner de FFT nuevo por archivo; se decodifican 6 s en vez de 12 tramos        |

Ninguno de estos números está instrumentado, así que no son medidas: son consecuencias deducidas del código. El primer paso no es optimizar, es poner los benchmarks de `criterion` y el test de presupuesto de latencia para que dejen de ser deducciones.

## 07. Lo que está bien hecho

Conviene decirlo con la misma precisión, porque marca lo que no hay que tocar.

- **La estructura del workspace.** Los 10 crates con sus responsabilidades y dependencias apuntando hacia abajo, sin ciclos. Añadir el escaneo real y los workers encaja sin mover nada.
- **El licenciamiento.** Es la parte más madura de la aplicación: producto añadido al `bdj_license_core` compartido, verificación SPP3 completa con clave raíz, código de producto, versión exacta y HWID V2, almacén seguro del sistema, y hasta un control de reloj hacia atrás que no estaba en el plan. Coherente con el resto de la suite.
- **La enumeración de volúmenes en Windows.** `GetLogicalDriveStringsW` + `GetDriveTypeW` + `GetVolumeInformationW` + `GetDiskFreeSpaceExW`, con clasificación de extraíble/fijo/red y exclusión de unidades CD. Bien hecho y con UTF-16 tratado con cuidado.
- **El catálogo de evidencias como modelo de datos.** `EvidenceCode` con `is_strong()` y `label()`, `Evidence` con `llr`, `applicable` y `description` por separado: es exactamente la estructura que hace posible la explicabilidad. Muchas herramientas del sector no la tienen.
- **El motor de veredicto.** Traduce fielmente la §08: umbrales, puerta de evidencia fuerte, veto de guardas, confianza por sigmoide y texto en lenguaje natural. Cuando llegue a ejecutarse, funciona.
- **La identidad visual.** La paleta cian eléctrico sobre azul medianoche sacada del logo es una decisión mejor que mi propuesta de negro y rojo: para una herramienta de análisis espectral, el cian sobre oscuro es el lenguaje del instrumento, y distingue el producto del resto de la suite sin salirse de la marca.
- **`tools/build_native.ps1`**: codegen, build en release y copia de artefactos, con `$ErrorActionPreference = 'Stop'` y comprobación de artefactos faltantes. Es la base del futuro workflow de CI.

## 08. Plan de trabajo

Cinco tandas. La primera es la única que hay que hacer antes de enseñarle la app a nadie.

T1 — Que el motor exista de verdad2-3 días

P0-1 (enum de códecs), P0-2 (ancho de banda por borde de contenido), P0-3 (*seek* a 12 tramos), P0-4 (espectro real de punta a punta), P0-6 y P0-7 (separar metadata informativa de evidencia fuerte: Lavf y ID3 fuera de la fusión).

**Criterio de aceptación**Un WAV lossless da *Lossless verificado*; el mismo tema en MP3 320 → WAV da *Probable transcode*; un WAV convertido con FFmpeg desde un master de 24 bits sigue dando *Lossless verificado*; un WAV etiquetado con Serato sigue dando *Lossless verificado*; y el espectro en pantalla cambia entre dos archivos distintos con el mismo corte.

T2 — Corpus y calibración3-4 días

Implementar `bdja_cli` de verdad (`analyze`, `scan`, `calibrate`, `validate`), generar la matriz de transcodes con tus propios masters, ajustar los pesos LLR contra datos en lugar de a mano, y meter los gates de la §09 en CI.

**Criterio de aceptación**FPR \< 1 % sobre lossless genuino y recall \> 95 % en MP3 ≤ 256 kbps, medido por un comando reproducible y no por impresión. Hasta que esto exista, ningún número de confianza que muestre la app significa nada.

T3 — Escaneo masivo real4-5 días

P0-5 completo: `scan_start` + `Stream<ScanEvent>`, pool de `cores−1`, cola acotada, huella blake3 para caché y dedupe, `scan_job` con checkpoints, cancelación real, modos de intensidad, sin tope de archivos. Migraciones y `user_version` en SQLite. Progreso y cancelación de verdad en la UI.

**Criterio de aceptación**50 000 archivos con la app matada dos veces a mitad: al reanudar no se repite ni se pierde ninguno, el RSS se mantiene por debajo de 250 MB y el segundo pase sobre lo ya escaneado tarda menos del 5 %.

T4 — Precisión y limpieza3-4 días

Los nueve puntos de la §02: E14 sobre evidencias, E05 por bloque contiguo, E06 normalizado, E07 por banda real, `ebur128` para LUFS y true peak conformes, métricas sobre la pista entera y por canal, E09 por LSB, fallo cerrado en archivos sin medida. Planner de FFT cacheado. Una sola fuente para «evidencia fuerte». Borrar el código muerto y los alias. La guarda de material band-limited.

**Criterio de aceptación**Suite de propiedades en verde: invariancia a ganancia, invariancia a añadir silencio, simetría entre canales idénticos, nunca NaN. Y ni un *warning* de clippy ni un code smell en SonarQube.

T5 — Endurecimiento y distribución4-5 días

Workers en procesos hijo (`bdja_worker` + `bdja_ipc` de verdad), gate de capacidad real, fuzzing, `cargo-deny`/`audit`/SBOM, reporte PDF, i18n es/en, los workflows de GitHub Actions, Inno Setup, DMG con notarización y firma en las dos plataformas.

**Criterio de aceptación**Instalador firmado que arranca limpio en un Windows 10 nuevo y en un macOS 12 sin Xcode, sin avisos de SmartScreen ni Gatekeeper, y un archivo corrupto que mata a un worker no interrumpe el escaneo.

**Total: 16-21 días** hasta algo vendible. La diferencia con el plan original (39 días desde cero) es la medida de lo que ya está construido: más o menos la mitad del camino, y la mitad que suele costar más tiempo.

## 09. Cómo competir

Spectro cuesta desde 24,99 USD y está dirigido a DJs; AudioAuditor es gratis y de código abierto. Ganarles por precio no se puede y por profundidad de análisis tampoco hace falta. Hay tres cosas que ellos no pueden hacer y tú sí.

### 1. La ventaja que nadie más tiene: el catálogo

Ninguna de esas herramientas está conectada a una tienda de remixes. Tú vendes los archivos. Eso abre algo que no es una función, es una posición: **certificar el catálogo de BDJ LATAM con tu propio motor** y mostrar el resultado en cada ficha de descarga — ancho de banda real, veredicto, versión del motor y fecha. Un DJ que compara dos pools elige el que le dice la verdad sobre lo que vende. Spectro no puede replicarlo porque no tiene catálogo; un pool competidor no puede replicarlo porque no tiene motor.

Y tiene una consecuencia operativa igual de valiosa: el analizador pasa a ser tu control de calidad interno de proveedores. Cada remix que entra al pool se analiza antes de publicarse. Eso te ahorra reclamaciones y te da, gratis, el corpus de calibración más realista que existe: material del mundo real, del género real, de tus proveedores reales.

### 2. Competir en el flujo de trabajo, no en el análisis

Las herramientas del sector analizan archivos. El DJ no tiene un problema con un archivo: tiene un problema con 8 000. Ahí es donde se gana:

| Movimiento                                             | Por qué gana                                                                                                                                                                                                                                                                                                               | Esfuerzo     |
|--------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|--------------|
| **Escaneo de biblioteca entera, reanudable, sin tope** | Es el requisito que ya pusiste como central y es exactamente lo que las alternativas hacen mal o limitan. Es la función que justifica el precio.                                                                                                                                                                           | T3           |
| **Leer las bibliotecas de Serato y Rekordbox**         | En vez de pedirle al DJ que elija carpetas, leer su base de datos de Rekordbox o sus crates de Serato y analizar «mi biblioteca» tal como él la ve, devolviendo el resultado por crate o por playlist. Nadie lo hace, y es el gesto que convierte la app en parte de la cabina. Solo lectura, nunca escribir en sus bases. | alto · v1.2  |
| **Vigilancia de carpeta**                              | Analizar automáticamente lo que entra en la carpeta de descargas del pool y avisar solo cuando algo sale sospechoso. Convierte una compra puntual en una herramienta que se queda abierta.                                                                                                                                 | medio · v1.1 |
| **Reporte de reclamo en PDF**                          | Un documento con el hash del archivo, el veredicto, las evidencias, el espectro real y la versión del motor, que el DJ manda al proveedor. Es el único entregable con el que el análisis se convierte en dinero recuperado.                                                                                                | bajo · T5    |
| **Comparación A/B**                                    | Arrastrar el master y la versión recibida y ver las dos curvas superpuestas. Es la demo que se explica sola y la mejor pieza de marketing que puede tener este producto.                                                                                                                                                   | medio · v1.1 |

### 3. La honestidad como característica de producto

Este es el punto que más valor tiene a medio plazo y el más fácil de tirar por la borda. Todas las herramientas de este espacio tienen el mismo problema reputacional: la gente no sabe cuánto creerles. Dos gestos que te ponen por encima:

- **Publicar la metodología y las cifras.** Las 14 evidencias, los umbrales, y sobre todo el recall por códec y la tasa de falsos positivos medidos sobre un corpus. Decir «en AAC detectamos el 60 %» genera más confianza que no decir nada — y es verificable, lo que ninguna alternativa ofrece.
- **Que «Inconcluso» se vea como un resultado y no como un fallo.** Una herramienta que se abstiene cuando no sabe es la que un DJ enseña a su proveedor. Una que acusa siempre se desacredita con el primer vinilo ripeado.

Y el corolario incómodo: **ahora mismo no puedes salir a competir**, no por falta de funciones sino porque el producto acusaría a los WAV legítimos de tu propio público (P0-6 y P0-7) y mostraría un espectro dibujado (P0-4). Un lanzamiento en ese estado quema exactamente el activo que te haría ganar. Con las tandas T1 y T2 hechas, la conversación cambia por completo.

### 4. Precio y distribución

Con Spectro en 24,99 USD de compra única, hay dos posiciones defendibles y una mala. La mala es competir a la baja. Las defendibles:

- **Incluido para suscriptores de BDJ LATAM** y de pago suelto para el resto. El analizador deja de ser un producto y pasa a ser una razón para suscribirse al pool — que es donde está tu ingreso recurrente.
- **Análisis de archivo suelto gratis, escaneo de biblioteca de pago.** El gratuito es la demo que se comparte solo (un DJ arrastra un archivo, ve el veredicto, hace captura y lo publica en su grupo); el de pago es el que resuelve el problema real. Coincide con la asimetría técnica: lo que cuesta de verdad es el escaneo masivo.

Para distribuir, la misma vía que ya tienes: entre colegas DJs primero, como con Stems Music, pero esta vez con el reporte PDF como pieza que circula. Cada reclamo que un DJ le manda a un sello lleva tu marca y la versión de tu motor.

## 10. Qué necesito de ti

1.  **Corpus.** Sigue siendo lo que bloquea todo lo demás. 200-400 masters lossless tuyos, variados, incluyendo a propósito material band-limited legítimo (vinilo, temas viejos). Sin eso, T2 no se puede hacer y los pesos LLR seguirán siendo opinión.
2.  **¿Arranco por T1?** Son seis arreglos concretos en archivos concretos y es lo que convierte la app en funcional. Puedo hacerlos en esta sesión o en la siguiente.
3.  **Quién escribió este código.** Si fue otra sesión trabajando desde el plan, conviene que le pase el informe; si lo escribiste tú, hay decisiones (Symphonia 0.5, `flutter_lints`, `deprecated_member_use: ignore`) que quiero entender antes de cambiarlas.
4.  **Serato/Rekordbox.** ¿Te interesa la lectura de sus bibliotecas como diferencial de la v1.2? Es la función con más ventaja competitiva de toda la lista y también la más costosa.
5.  **La paleta cian queda confirmada**, salvo que me digas lo contrario. Actualizo la §13 del plan para que documente lo que hay en el código y no lo que yo había propuesto.


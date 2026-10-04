# Compromiso Arquitectónico y Mapa de Ruta — oxedi835

> Documento vivo. Es el contrato de diseño del proyecto. Se construye stage por stage:
> cada stage añade su sección de compromiso (§7) tras ser aprobada. Las decisiones del
> Norte (§2) son invariantes: si un stage choca con uno, se rediseña el stage, no el invariante.
>
> `oxedi835` = oxidación (Rust 🦀) + edi835. Nace como reinicio greenfield del POC `fast_edi835`,
> que queda solo como referencia.

Fecha de inicio: 2026-06-14
Última revisión del diseño: 2026-10-02 (modelo de procesamiento por eventos, §3 y §4)

---

## §1 · Propósito y alcance

Un core de parseo EDI 835 en Rust: **lossless, rápido, reanudable y extensible por datos**,
con bindings multi-lenguaje (Python primero). Nace de un cuello de botella real de ingesta en
producción (parser Python demasiado lento para cientos de archivos `.rmt` grandes/día, que
obligó a escalar a múltiples workers/pods y migrar de sync a async; se consideró Go pero no
existía librería del estándar fuera de Python).

**Aprender Rust a fondo es un objetivo co-igual al de resolver el problema** — no un medio.
Sin urgencia: priorizamos diseño correcto y verificable sobre velocidad de entrega.

---

## §2 · Norte — No-negociables

- **N1 · Lossless.** Jamás se descarta un segmento, aunque no se entienda. Todo lo del
  archivo es recuperable y en su orden original.
- **N2 · Fidelidad X12.** Los delimitadores se leen del segmento ISA (no se asumen `*`/`~`)
  y se respeta el carácter de escape/release. Un valor que contenga el separador no rompe
  el parseo. La misma regla aplica en sentido inverso: al escribir, un valor que contenga
  un delimitador se escapa.
- **N3 · Extensibilidad por datos; los built-ins *son* datos.** El conocimiento del estándar
  835 se expresa en el mismo formato-dato con el que un usuario lo extiende. *Extender* y
  *sobrescribir* son la misma operación: proveer datos distintos. Nunca forkear, nunca
  recompilar, idéntico en todos los bindings.
- **N4 · Perf-consciente.** Cero-copia donde sea posible (prestar del buffer, no clonar
  `String`s); memoria acotada: el camino principal no necesita materializar el archivo
  completo; el binding de Python libera el GIL para paralelismo real entre archivos.
- **N5 · Verificable por stage (aislamiento).** Cada capa tiene un contrato explícito y un
  gate de verificación objetivo que debe pasar antes de avanzar.
- **N6 · Aprendizaje explícito.** Cada stage documenta el concepto de Rust que enseña y por
  qué se resuelve así; el "porqué" idiomático es parte del entregable.
- **N7 · Verificación de integración entre capas.** Además de los gates de aislamiento (N5),
  cada *costura* entre capas tiene su prueba de integración que valida el contrato a través
  del límite (framing→tokenizer, tokenizer→motor de loops, motor→documento, motor→dominio,
  core→binding). Existe además un test end-to-end que atraviesa todas las capas sobre
  fixtures reales. Una capa no está "lista" hasta que su integración con la capa inferior
  está verificada, no solo su comportamiento aislado.

---

## §3 · Principios arquitectónicos transversales

- **P1 · Dependencia unidireccional en capas.** El framing no sabe nada de segmentos. El
  tokenizer no sabe nada del 835. El motor de loops no sabe nada del 835. El 835 es solo
  *datos* encima de un motor genérico. Las capas de abajo nunca conocen a las de arriba.
- **P2 · Motor genérico vs. conocimiento del estándar.** Separación dura: código =
  intérprete genérico; el estándar = specs-dato (la primera que shippeamos).
- **P3 · Núcleo sans-IO.** Ninguna capa del core hace entrada/salida ni depende de un
  runtime. El core recibe bytes y devuelve valores. Quien llama decide de dónde vienen los
  bytes (disco, S3, red) y cómo envuelve el core. Esto es lo que permite cambiar el origen
  sin tocar el core.
- **P4 · Procesamiento por eventos, reanudable.** El archivo se procesa como un flujo de
  segmentos que el consumidor tira (*pull*) uno a uno. Entre segmento y segmento el control
  vuelve al consumidor: el pipeline está pausado por construcción. El motor de loops no
  recorre el archivo; se le alimenta un segmento y responde con eventos. El consumidor es
  dueño del bucle.
- **P5 · Framing separado del origen del buffer.** Localizar el siguiente segmento (buscar el
  terminador respetando el escape) es una función pura sobre `(&[u8], delimitadores)`. No
  sabe de dónde vienen los bytes. El iterador en memoria la usa hoy; un tokenizer por
  trozos (D7) la reutiliza mañana sin reescribir N2.
- **P6 · La spec-dato es estructural y bidireccional.** La spec no declara solo "qué
  segmento abre qué loop" (disparadores). Declara la estructura: loops, qué segmentos los
  componen, en qué orden, con qué elementos. La misma spec dirige el parseo (interpretar) y,
  en el futuro, la escritura (generar, D8). Extender el estándar extiende ambas direcciones.
- **P7 · Sin pánico ante input.** Ni en el camino feliz ni ante un archivo malformado. Los
  fallos son `Result` explícitos por segmento o por evento; el parser es robusto, no aborta.
  Un segmento inválido se reporta y el flujo continúa (N1).
- **P8 · Testeable en aislamiento + en integración.** Doble red: cada capa se prueba sola
  (N5) y cada costura se prueba unida (N7). El modelo pull hace que el motor de loops se
  pruebe alimentándole segmentos a mano, sin fixtures ni archivo.
- **P9 · Materializar es opcional.** El documento lossless completo (árbol en memoria) es
  el resultado de *recolectar* el flujo, no el paso obligatorio. Quien necesite todo el
  archivo lo recolecta; quien procese claim a claim no paga esa memoria.
- **P10 · Los errores son declarativos: se explican solos.** Un error, un evento de fallo
  o un diagnóstico debe poder leerse sin abrir el código y responder tres preguntas:
  *qué regla no se cumplió*, *dónde* y *con qué dato*. "Dónde" significa, según el caso:
  en una spec, el loop y la clave tal como se escribió; en un archivo, el índice del
  segmento, su rango de bytes (resoluble desde el `Document`), la posición del elemento y
  del componente cuando el fallo está dentro del segmento, y la ruta de loops abiertos.
  "Con qué dato" es el valor ofensivo tal cual (el byte, el id, el texto del elemento).
  Los fallos al parsear un archivo real son el caso principal, no la excepción: un
  segmento desconocido, un elemento vacío donde se esperaba un importe, un `SE` cuyo
  conteo no cuadra, todos se reportan con esa ubicación completa. Cada tipo de error es un enum con esos datos en sus
  variantes; el texto de `Display` es parte del contrato y tiene un test por variante;
  `source()` encadena la causa. Un error que dice "inválido" sin decir qué, dónde ni por
  qué, es un defecto, no un detalle. Los fallos de datos de entrada no son errores de
  programa: se emiten como eventos o diagnósticos con la misma información, y el flujo
  continúa (P7).

---

## §4 · Modelo de procesamiento

Vista de conjunto de cómo fluye un archivo por las capas. Es el contrato que los stages 1–4
implementan; cada stage detalla su pedazo en §7.

```
bytes (&[u8])
  │
  │  P5 · framing: (bytes, delims) -> (segmento crudo, resto)        [Stage 1]
  ▼
Tokenizer<'a>: Iterator<Item = Result<Segment<'a>>>                  [Stage 1]
  │  lee delimitadores del ISA; divide en elementos/componentes;
  │  respeta escape; presta del buffer; cada segmento lleva su índice
  ▼
LoopEngine::feed(&mut self, Segment<'a>) -> Vec<Event<'a>>           [Stage 3]
  │  máquina de estados dirigida por la spec-dato (P6);
  │  eventos: LoopOpened, LoopClosed, SegmentCaptured, Unmatched, Error
  ▼
  ├─ collect()  -> Document<'a> (árbol lossless completo, P9)        [Stage 2]
  ├─ proyección -> filas de negocio + validación SNIP                [Stage 4]
  └─ binding    -> iterador Python de loops/claims, o parse completo  [Stage 5]
```

**Pausar y comunicar segmentos.** No hay un mecanismo especial. Con `Iterator` el
consumidor controla cuándo pedir el siguiente segmento; con `feed` controla cuándo se lo
entrega al motor y recibe los eventos en ese mismo instante. Parar en un CLP, inspeccionar
y seguir es simplemente no llamar a `next` todavía.

**Ordering.** Cada segmento conserva su índice de aparición. Los eventos lo heredan. Esto es
lo que hace N1 verificable: desde cualquier salida (documento, eventos, filas) se puede
señalar el segmento de origen y reconstruir el archivo en orden.

**Errores.** `Result` por segmento en el tokenizer y evento `Error` en el motor. Un error
no detiene el flujo; el consumidor decide si aborta. Nunca `panic`.

---

## §5 · Mapa de stages

Cada stage es independiente y verificable. El orden coincide con una curva de aprendizaje de
Rust que escala de lo básico a lo profundo. Desde el Stage 1 en adelante, cada stage también
carga su prueba de costura con la capa inferior (N7).

| Stage | Qué entrega | Cómo se verifica | Rust que exprimes |
|-------|-------------|------------------|-------------------|
| **0 · Andamiaje** | Workspace, CI, fmt/clippy gates, harness de tests/property/benchmarks | CI verde con smoke test, clippy limpio, bench corre | cargo, workspace, módulos, CI |
| **1 · Framing + Tokenizer** | Framing puro (P5) + lexer ciego al 835 como `Iterator` perezoso: bytes → `Segment<'a>` con elementos/componentes, delimitadores del ISA, escape simétrico (lectura y escritura) | Property test round-trip segmentos→bytes→segmentos; casos borde del `*` en valor; framing probado solo, sin tokenizer | lifetimes & borrowing, slices, `&str` vs `String`, `Result`, `Iterator` con lifetimes, funciones puras |
| **2 · Documento lossless** | `Document<'a>` como materialización opcional (P9): recolectar el flujo retiene TODO, prestado del buffer; reconstrucción exacta | Todas las fixtures: cero segmentos perdidos + reconstrucción byte a byte; `collect` del iterador == parse completo | structs con lifetimes, `Cow`, ownership, `FromIterator`, arena |
| **3 · Motor declarativo de loops** | Spec-dato estructural (P6) + motor como máquina de estados `feed(segmento) -> eventos` (P4) | Golden files; alimentar segmento a segmento == procesar entero (determinismo); test del caso real: sección propietaria capturada por spec custom; motor probado con segmentos construidos a mano | enums y pattern matching para máquinas de estado, `serde`, data-driven, ownership de estado mutable, iteradores adaptadores, recursión |
| **4 · Proyección a dominio + validación** | eventos/loop tree → filas de negocio + SNIP, también declarativo; funciona sobre el flujo (claim a claim) y sobre el documento | Fixtures → filas/validación esperadas; proyección incremental == proyección sobre documento | iteradores, `serde_json`, transformaciones |
| **5 · Binding Python** | PyO3/maturin, libera el GIL, API ergonómica: parse completo + iterador Python de loops/claims (memoria acotada), wheels | pytest sobre las mismas fixtures; benchmark vs. lib Python vieja; test de memoria: iterar un archivo grande no lo materializa | FFI, PyO3, maturin, protocolo de iterador Python |
| **6 · Distribución** | crates.io + PyPI (manylinux), CI de release, quizá WASM | Instalar desde PyPI en entorno limpio y smoke test | publishing, cross-compile, semver, deploy |
| **7 · Escritor** *(programado, D7; prerrequisito D11)* | Tablas (nuestro esquema, `Tables` o Arrow) → loops → segmentos → bytes, dirigido por la misma spec invertida (P6); calcula derivados (SE01, GE01, IEA01, controles, ISA ancho fijo, totales BPR); diagnósticos antes de escribir; ingestión desde bases relacionales vía DuckDB fuera del core | Generar y volver a parsear == mismas tablas y cero diagnósticos; `pyx12` valida lo generado | builder pattern, `io::Write`, formato de ancho fijo, inversión de una proyección declarativa |

---

## §6 · Decisiones

### §6.1 · Tomadas (con fecha y porqué)

- **T1 · Pull con `Iterator`, no push — 2026-10-02.** El tokenizer es un `Iterator`; el
  motor expone `feed`. El consumidor controla el ritmo. Descartado push con callbacks:
  invierte el control y complica pausar.
- **T2 · Sin paralelismo intra-archivo — 2026-10-02.** Se evaluó particionar por loops
  (ST o CLP) y procesar en paralelo. Descartado: exige dos pasadas, merge ordenado y contexto
  heredado por partición, y el caso real (cientos de archivos/día) ya lo cubre el paralelismo
  entre archivos al liberar el GIL. Se revisa solo si aparece un archivo individual cuyo
  tiempo de parseo sea el cuello de botella.
- **T3 · Framing como función pura separada — 2026-10-02.** Ver P5. Es lo que deja abierta
  la puerta al streaming por trozos (D6) sin reescritura.
- **T4 · Spec estructural y bidireccional — 2026-10-02.** Ver P6. Es lo que deja abierta la
  puerta al escritor (D7) sin una segunda spec.
- **T5 · Documento = buffer `Cow` + índices (resuelve D1) — 2026-10-02.** El
  `Document<'a>` no guarda `Segment`s: guarda los bytes como `Cow<'a, [u8]>` y un vector de
  *spans* (rangos de `raw` y `body` por segmento). Los `Segment` se construyen bajo demanda
  prestando del documento. Así un mismo tipo es cero-copia cuando presta del llamador
  (`Cow::Borrowed`) y dueño cuando hace falta (`Cow::Owned`, `into_owned()` da
  `Document<'static>`), que es lo que PyO3 y un tokenizer por trozos (D6) necesitarán.
  Descartado `Vec<Segment<'a>>`: no puede volverse dueño sin un struct autorreferencial.
  Descartado `Document` siempre dueño: pagaría una copia del archivo en el camino normal.
  Candidato a revisar: buffer compartido `Arc<[u8]>` + spans (ver D8), que cambia solo la
  representación del buffer y deja los spans intactos.
- **T24 · D8 resuelta: el documento sigue siendo `Cow` + spans — 2026-10-03.** Medido con
  `examples/buffer_retention.rs` sobre `edi835_test_united.rmt` (629 KB, 30 302 segmentos,
  release, AMD Ryzen 7 5700U) frente a un prototipo `Arc<[u8]>` con los mismos spans.
  Construir desde el `Vec<u8>` que el binding ya copió: 1,13 ms con `Cow` y 1,54 ms con
  `Arc` (`Arc::from(Vec)` vuelve a copiar el buffer en una asignación nueva, con sus fallos de
  página). Iterar todos los segmentos: 4,2 frente
  a 4,1 ms. Retener N documentos de N entradas ocupa lo mismo: 1,9 / 18,5 / 185 MiB para
  N = 1, 10, 100. `Arc` solo gana al clonar (35 frente a 77 µs), y nadie clona: Python
  comparte el objeto `Document` por referencia y `stream` no construye documento. Se descarta
  `Arc`. Hallazgo: los spans pesan el doble que los bytes (40 bytes por segmento frente a
  ~21 de texto), así que retener menos pasa por compactar `Span`, no por compartir el buffer
  (issue #45).

### §6.2 · Abiertas (marcadas para no olvidarlas)

- **D2 · Formato exacto de las specs** (Stage 3): esquema, fusión default+usuario, cómo
  llega a Python como `dict`. Restricción fijada por T4: debe describir estructura (loops →
  segmentos → elementos), no solo disparadores. Con T5, los eventos y el árbol de loops
  referencian segmentos por índice en el documento, no por copia.
- **D3 · Tier 2 escape hatch (WASM/Extism)**: diseño consciente pero **no se construye**;
  marcado como YAGNI hasta que un caso real lo exija.
- **D6 · Tokenizer por trozos (streaming desde S3/red)**: dos niveles. Nivel 1,
  descargar a memoria y parsear, está cubierto por P3 sin cambios. Nivel 2, parsear mientras
  llega sin tener el archivo entero, requiere que el segmento no se preste del buffer del
  llamador. Opciones a decidir cuando exista el caso: segmentos con datos propios solo en
  este modo, o buffer interno con préstamo ligado al tokenizer (patrón *lending iterator*,
  no expresable con `Iterator` estándar). El framing (T3) se reutiliza tal cual.
- **D8 · `Cow` + spans frente a `Arc<[u8]>` + spans** → resuelta por T24 (Stage 5): en el stage siguiente al que
  tenga un consumidor que comparta el documento (previsiblemente Stage 5, Python), medir
  las dos representaciones en tiempo y en memoria sobre las mismas fixtures y los samples
  grandes: construir, iterar, `into_owned` o clonar, y retener N documentos a la vez. Se
  decide con números, no antes. El cambio es local porque los spans no cambian.
- **D9 · Segundo formato de spec (YAML)**: acordado el 2026-10-02 empezar solo con JSON.
  YAML se añade después como *otro deserializador* sobre el mismo `Spec` (un crate más,
  detrás de un feature), sin tocar el motor ni el formato en memoria. Se decide cuándo
  cuando exista un usuario que escriba specs a mano.
- **D10 · Proyección columnar / Arrow** (resuelta en Stage 4 por T14 y T15): el core
  construye columnas con la disposición de Arrow sin depender del crate; Stage 5 las expone
  por la interfaz C sin copiar.
- **D11 · Cardinalidad de segmentos por loop y reglas de cuadre declarativas**: SNIP 2
  completo exige saber qué segmentos son obligatorios y cuántas veces se repiten dentro de
  un loop, y SNIP 3 exige declarar qué columnas se suman contra cuáles. Ambas piden una
  extensión de la spec que Stage 4 no abre; se decide cuando las tablas proyectadas estén
  en uso y se vea si una regla "suma por grupo" basta. Desde el 2026-10-03 es prerrequisito
  del escritor (D7): la escritura necesita orden y cardinalidad por loop y los cuadres
  (`BPR02` contra claims y `PLB`; `CLP03`−`CLP04` contra `CAS`; `SVC02`−`SVC03` contra `CAS`
  de servicio) para calcular o comprobar los totales. Las medidas del planificador de 4b (el
  cuadre de `BPR02` necesita sumas con signos opuestos) acotan el formato de reglas.
- **D12 · Compatibilidad con `edi-835-parser`** (Python, keiron-stoddart; `parse(path)
  → TransactionSets.to_dataframe()`): tras el Stage 5, escribir una spec de `tables` (y el
  parche que haga falta) cuya salida coincida fila a fila con el DataFrame de esa librería
  sobre los archivos que ambos pueden leer. Si se consigue solo con datos, N3 queda probada
  frente a un parser real y sus usuarios tienen camino de migración; si exige código, la
  diferencia dice qué le falta a la spec. La comparación es un test reproducible. Nota
  2026-10-03: `edi-835-parser` falla en cinco de los seis samples anonimizados; la
  comparativa se hace sobre los originales (fuera del repo), y antes hay que identificar
  qué campo altera `scripts/anonymize_835.py` de forma que rompe a ese parser y corregir
  el anonimizador, regenerando los samples por el camino previsto (nunca a mano).
  Diagnóstico del mismo día (#46): el anonimizador no es la causa; los originales fallan
  igual porque la librería hace `int()` sobre `N104` y los payers usan ids `XV`
  alfanuméricos, válidos en X12. La comparativa de 5b corre sobre los originales con un
  parche mínimo en el script de comparación que acepte `N104` no numérico, documentado como
  la única divergencia conocida, y la corrección se ofrece aguas arriba.
  Alcance fijado por el dueño el 2026-10-03, en tres partes y en este orden: (1) una spec de
  `tables` (y parche) cuyo DataFrame coincide con `TransactionSets.to_dataframe()` de
  `edi-835-parser` fila a fila y columna a columna sobre los originales; la velocidad es un
  dato más, no el objetivo; (2) con esa paridad probada, buscar en los archivos las secciones
  que esa librería pierde (por ejemplo los ajustes a nivel de claim, "other claim
  adjustments", los `PLB` o los `REF`/`AMT` que no mapea) y demostrar con los mismos
  archivos que `oxedi835` sí los reconoce y los entrega; (3) una API Python compatible
  sobre el paquete `oxedi835` que cubra toda la superficie pública de esa librería (1.8.0),
  no solo `to_dataframe()`: `parse(path | dir) -> TransactionSets`; en `TransactionSets`,
  `__iter__`, `__len__`, `count_claims()`, `count_patients()`, `sum_payments()`,
  `sort_columns(df)` y `to_dataframe()`; en `TransactionSet`, `payer`, `payee`,
  `to_dataframe()` y `serialize_service(...)`, con los objetos que expone (`interchange`,
  `financial_information`, `claims`, `organizations` y los loops `Claim`/`Service` con sus
  segmentos), con la misma forma de columnas y los mismos tipos. Así un usuario de la
  librería vieja migra sin tocar su código y gana lo que aquella pierde. Forma acordada el
  2026-10-03: la capa vive dentro del paquete `oxedi835` como subpaquete con el nombre de
  importación de la librería que imita (`oxedi835.edi_835_parser`), activada por un extra de
  pip igual de explícito (`oxedi835[edi-835-parser]`, que arrastra `pandas` solo para quien lo
  pide); el nombre genérico "compat" se descarta porque no dice con qué es compatible. Otras
  librerías seguirían el mismo patrón (`oxedi835.<nombre>`, extra `oxedi835[<nombre>]`), cada
  una con su spec de `tables` y sus vistas; antes de abrir 5b se hace un sondeo de los parsers
  835 en Python existentes y de lo que exponen, para decidir con datos si alguna otra merece
  una capa (hecho el 2026-10-03: solo `edi-835-parser` la merece). (4) **Equivalentes nativos
  en la API propia** (acordado 2026-10-03): la API nativa de `oxedi835` (sobre Arrow y Polars)
  ofrece sus propios métodos para los mismos conceptos que esa librería expone, con nombres
  nuestros y sin `pandas`: en `Result`, `count_claims()`, `count_patients()`,
  `sum_payments()`, `payer` y `payee` (desde las tablas `payments`/`claims`, con `Decimal`
  exacto para los importes), y en `Tables`/`Table`, `to_polars()` y `to_pandas()` como
  conveniencias. Dependencias: el wheel base de `oxedi835` no declara ninguna dependencia
  Python (Arrow sale por PyCapsule); `polars`, `pandas` (+ `pyarrow`, que pandas necesita como
  puente) y `edi-835-parser` (que arrastra `pandas` por su contrato) se declaran solo como
  extras (`oxedi835[polars]`, `oxedi835[pandas]`, `oxedi835[edi-835-parser]`); `to_polars()` y
  `to_pandas()` importan la librería al llamarse y, si falta, lanzan un error que nombra el
  extra a instalar (P10). Nunca como dependencia directa ni transitiva del paquete base.
  DuckDB (comprobado el 2026-10-03 con la 1.5.6): consume nuestras tablas por el mismo
  protocolo sin dependencia alguna (*replacement scan* de la variable Python, `register`,
  joins por los ordinales de padre, `Decimal128` exacto); 5b añade un test con `duckdb` como
  dependencia de desarrollo, y no hace falta extra para DuckDB. Así hay dos caminos de
  migración: el rápido, instalar `oxedi835[edi-835-parser]` y cambiar un `import`; y el
  definitivo, pasar con tiempo a los métodos nativos, con una tabla "método viejo → método
  nuevo" en la documentación (Stage 8).
- **D13 · Estructura de módulos** → resuelta por T37–T45 (Stage 5c, 2026-10-04): `spec.rs` supera las 2.500 líneas tras el Stage 4a y
  `diagnostic.rs`, `engine.rs` y `check.rs` crecen. Tras el Stage 5, planificar la división
  en submódulos (por ejemplo `spec/{load,shape,segments,tables,control,patch}.rs`) con
  reglas de descubrimiento: archivos cortos, un sustantivo por archivo, `lib.rs` como índice
  comentado, tests junto al código que prueban. Sin cambio de comportamiento; se verifica
  con la suite y los goldens intactos.
- **D14 · Documentación perdurable** (al cerrar el roadmap): un libro para humanos con las
  ideas, los conceptos y los patrones que rigen el proyecto (lossless por construcción,
  motor genérico y estándar como datos, pull, sans-IO, errores que se explican solos,
  columnas con disposición Arrow), sin fragmentos de código ni referencias a líneas, para
  que no exija mantenimiento continuo; más guías de uso de la librería Python y del binario.
  Lo que sí cambia con el código (firmas, ejemplos) se queda en rustdoc y en los planes.
- **D15 · Toolkit para la familia X12** (835 primero, 837 después): el motor, el formato
  de spec, la proyección y el binario no saben nada del 835 más allá de la spec built-in, así
  que otro conjunto de transacciones con la misma lógica de loops y otras definiciones de
  segmentos debería entrar como una spec más. A decidir: nombres (crate y binario dejan de
  ser "835"), una spec por conjunto de transacciones, qué expone el CLI, y qué suposiciones
  del 835 se colaron en código (auditar antes de abrir la 837).
- **D16 · Interoperabilidad con `pyx12`** (acordada 2026-10-03). `pyx12` es el validador
  HIPAA X12 de referencia en Python (BSD, activo), con mapas XML por guía de implementación
  para toda la familia. Tres piezas: (1) **parte del Stage 9**: usar sus mapas como oráculo y
  generador de nuestras specs, con un script que coteje el mapa del 835 contra
  `specs/835.json` (segmentos, obligatoriedad, códigos) y que genere el borrador de la spec de
  la 837 y siguientes, respetando la atribución BSD al derivar datos; (2) **opcional**:
  `oxedi835.pyx12.validate(data)` que corre la validación de `pyx12` sobre los mismos bytes y
  traduce sus errores a `Diagnostic` (índice, elemento, ruta, dato), una sola lista con los
  dos orígenes marcados, para cubrir los niveles SNIP 3–7 sin escribirlos; (3) **opcional,
  bajo demanda**: `oxedi835.pyx12.ContextReader(document)`, vista de solo lectura con la forma
  de `X12ContextReader` (`iter_segments`, `select`, `get_value`, `exists`) sobre nuestro
  `Document` y `LoopTree`. Las piezas 2 y 3 viven en el subpaquete `oxedi835.pyx12` tras el
  extra `oxedi835[pyx12]`, siguiendo la regla de nombres de D12; el script de la pieza 1 vive
  en `scripts/`.
- **D7 · Stage 7, Escritor** → programado el 2026-10-03, tras 5b y Stage 6, con un caso real:
  generar un 835 (`.RMT`) a partir de datos en bases relacionales. Contrato acordado: la
  entrada es nuestro esquema de tablas (`Tables` del core o Arrow por el mismo protocolo
  PyCapsule en sentido inverso: `payments`, `claims`, `services`, `adjustments`,
  `provider_adjustments` con sus ordinales de padre); la spec dirige la escritura invirtiendo
  la sección `tables` (columna → loop, segmento, elemento) y usando `segments` para saber qué
  es obligatorio (P6, la spec es bidireccional); el escritor calcula los derivados (`SE01`,
  `GE01`, `IEA01`, números de control, anchos fijos del `ISA`, totales del `BPR`) y, si falta
  un elemento obligatorio, emite un `Diagnostic` antes de escribir, con la misma forma que los
  de lectura (P10 en sentido inverso); el core sigue sans-IO (devuelve bytes o escribe en un
  `io::Write`); la ingestión desde bases de datos vive fuera del core: DuckDB con sus
  conectores (`postgres`, `mysql`, `sqlite`, ODBC, Parquet) produce Arrow con nuestras
  columnas mediante SQL del usuario, y `oxedi835.write(tables, spec=None) -> bytes` en Python.
  Gate: generar y volver a parsear con nuestro `Processor` devuelve tablas idénticas y cero
  diagnósticos, y `pyx12` (D16) valida lo generado contra la guía. Prerrequisito dentro del
  mismo stage: D11 (orden y cardinalidad de segmentos por loop, reglas de cuadre), porque la
  lectura toma "el primer segmento que cumple `where`" y la escritura necesita orden y número
  de repeticiones; y elegir versión (4010 o 5010) por salida, fijando `ISA12`/`GS08`.
  **Exportación a SQLite (acordada el 2026-10-04).** Una función que vuelca un resultado a una
  base SQLite con `sqlite3` de la biblioteca estándar (sin dependencias nuevas): las cinco
  tablas con claves foráneas por los ordinales de padre, importes como decimal exacto en `TEXT`
  (nunca `REAL`), fechas ISO 8601, versión del esquema en `PRAGMA user_version`; y, con
  `include_raw=True`, una tabla `files` (delimitadores, versión, nombre) y una tabla `segments`
  con cada segmento crudo, su índice, su loop y su archivo, de modo que la base conserva el
  archivo sin pérdida. Su valor hoy es interno: oráculo de ida y vuelta del escritor (archivos
  → SQLite → adaptador → `.RMT` → `parse` → mismas tablas y cero diagnósticos; con la capa
  cruda, idéntico byte a byte) y forma cómoda de explorar los samples con SQL. Nace como
  herramienta de desarrollo dentro del Stage 7 (o antes si la queremos para análisis) y solo
  pasa a API pública documentada si aparece demanda de usuarios. La mitad
  del trabajo ya la paga el round-trip de Stage 1 (serializar segmentos con escape) y la otra
  mitad T4.

---

## §7 · Compromisos por stage

_(Se completan stage por stage conforme se aprueban.)_

### Stage 0 · Andamiaje y arnés de verificación — APROBADO 2026-06-14

Antes de una sola línea de lógica, montamos la infraestructura de verificación que N5/N7
exigen. El "proceso verificable" existe desde el commit cero.

**Propósito.** Crear el esqueleto del workspace y el harness de pruebas/benchmarks. Cero
lógica de dominio.

**Entregable / contrato.**
- Workspace Cargo en la raíz `oxedi835/`, con **solo** el crate núcleo `edi835_core`. Los
  crates `edi835_python` / `edi835_cli` se añaden en sus stages (decisión tomada: no crear
  crates vacíos que no se usan).
- Convención de pruebas fijada: unit tests inline (`#[cfg(test)]`), integration tests en
  `tests/`, fixtures en `tests/fixtures/` (reusamos las del POC: blue_cross, united,
  trizetto, emedny, multi_claim).
- Harness de property testing listo (`proptest`) — se exprime en Stage 1.
- Esqueleto de benchmarks (`criterion`), aunque mida nada, para que medir perf (N4) sea
  hábito desde el inicio.
- CI con gates obligatorios: `cargo build`, `cargo test`, `cargo clippy -- -D warnings`,
  `cargo fmt --check`, con la acción correcta `dtolnay/rust-toolchain`.

**Gate de verificación (salida del Stage 0).**
- CI verde sobre un commit con solo un smoke test trivial.
- `cargo clippy -- -D warnings` limpio.
- `cargo bench` corre.
- Un integration test "placeholder" que carga una fixture y comprueba que el harness de
  fixtures funciona (sin parsear todavía) — deja lista la tubería de N7.

**Dependencias.** Ninguna. Es la base.

**Rust que exprimes.** `cargo`, workspace multi-crate, organización de módulos,
`#[cfg(test)]`, integration vs unit tests, `dev-dependencies`, configuración de CI,
clippy/fmt.

**Fuera de alcance.** Nada de parseo, tipos de dominio, ni binding Python.

**Nota tras la revisión del 2026-10-02.** El cambio al modelo por eventos (§3, §4) no
altera este stage. El plan `plans/stage-0-scaffolding.md` sigue vigente tal cual.

### Stage 1 · Framing + Tokenizer — APROBADO 2026-10-02

Primera capa con lógica. Dos unidades con una costura entre ellas (N7): el *framing*, que
no sabe qué es un segmento, y el *tokenizer*, que no sabe qué es un 835.

**Propósito.** Convertir bytes en un flujo perezoso de segmentos genéricos, sin perder un
byte (N1), con los delimitadores que declara el archivo (N2), prestando del buffer (N4).

**Hechos de las fixtures que condicionan el diseño** (verificados el 2026-10-02).
- Cuatro de cinco fixtures empiezan por ISA. `blue_cross_nc_sample.txt` empieza por ST:
  es un *fragmento* sin sobre. Existen en producción (extractos, pruebas), así que el
  tokenizer debe poder arrancar con delimitadores dados por el llamador.
- El ISA es nominalmente de 106 bytes de ancho fijo, pero `multi_claim` mide 105 y
  `trizetto` 102 (ISA06/ISA08 mal rellenados). **Nunca se leen los delimitadores por
  offset**: se cuentan separadores. El separador tras `ISA` es el #1; ISA16 (componente)
  es el byte tras el separador #16 y el terminador es el byte siguiente.
- Separador `*` y terminador `~` en todas. Componente `:` (emedny) y `>` (las demás).
  ISA11 vale `U` en 4010 (identificador de estándar, no separador) y `^` en 5010
  (separador de repetición). Regla: ISA11 es separador de repetición si ISA12 ≥ `00402`.
- Dos fixtures tienen `\n` tras cada `~`, incluido el último; tres no tienen ningún salto
  de línea. Los bytes entre un terminador y el siguiente segmento son *trivia* que N1
  obliga a conservar. Ningún archivo tiene `\r`; se tolera igual como trivia.
- `trizetto_sample.rmt` tiene `~XX*654321~` donde el POC seguramente quiso `*`: produce un
  segmento `XX` espurio y el conteo de SE no cuadra. Se conserva tal cual, byte a byte,
  como caso deliberado de segmento desconocido (N1) e input malformado (P7).
- Sobre el "escape": X12 no define carácter de release (eso es EDIFACT/UNA) y el POC lo
  ignoraba. Lectura de N2 para este stage: (a) los delimitadores salen del ISA; (b) un
  byte que *parece* delimitador pero no es el del archivo (un `*` cuando el separador es
  `|`) es dato; (c) un carácter de release es **opcional y configurable** por el llamador,
  nunca inferido del archivo. Si existe, framing, división de elementos y escritura lo
  respetan.

**Entregable / contrato.**
- `Delimiters { element, component, segment: u8, repetition: Option<u8>, release:
  Option<u8> }`. `Delimiters::from_isa(&[u8]) -> Result<Delimiters, IsaError>` con
  `IsaError::{NotIsa, Truncated { len }}`. Builders `new(element, component, segment)`,
  `with_repetition`, `with_release`.
- **Framing (P5), módulo `frame`.** Función pura
  `next_frame(input: &[u8], &Delimiters) -> Option<(Frame<'_>, &[u8])>` con
  `Frame { raw, body, terminated }`. `raw` va desde el primer byte de `input` hasta el
  terminador inclusive (incluye la trivia inicial); `body` es `raw` sin trivia inicial ni
  terminador; el segundo valor es el resto. Respeta `release` al buscar el terminador. Sin
  terminador, el último frame es todo lo que queda con `terminated: false` (puede tener
  `body` vacío: es la trivia final del archivo). Invariante: concatenar todos los `raw`
  reproduce `input` byte a byte. `None` solo cuando `input` está vacío.
- **Elementos, módulo `element`.** `Element<'a>` es `Simple(Value<'a>)` o
  `Composite(Vec<Value<'a>>)` con `Value<'a> = Cow<'a, [u8]>`: prestado del buffer salvo
  que haya habido que quitar un byte de release (entonces propio). Composite cuando el
  elemento contiene un separador de componente sin escapar. La repetición (`^`) se
  reconoce en `Delimiters` pero no se divide (YAGNI hasta que una spec lo pida).
- **Segmento y tokenizer, módulo `segment` y `tokenizer`.** `Segment<'a> { index, raw,
  id: &'a [u8], elements: Vec<Element<'a>>, terminated }`. `Tokenizer<'a>` implementa
  `Iterator<Item = Segment<'a>>`. **El tokenizer no tiene errores por segmento**: todo
  frame se emite como segmento, incluidos los vacíos (`~~`) y la trivia final, que llevan
  `id` vacío. N1 y P7 se cumplen por construcción: el consumidor decide qué hacer con un
  `id` vacío. Dos constructores: `Tokenizer::new(&[u8]) -> Result<_, IsaError>` lee los
  delimitadores del ISA (tolerando trivia antes de él); `with_delimiters(&[u8],
  Delimiters)` para fragmentos o para inyectar `release`.
- **Serialización simétrica (N2 inverso, D7).** `Segment::write_to(&self, &Delimiters,
  &mut impl io::Write) -> Result<(), WriteError>` reconstruye el segmento desde `id` y
  `elements`, escapando con `release` cualquier byte delimitador dentro de un valor; sin
  `release`, un valor con delimitador es `WriteError::DelimiterInValue { byte }`. No
  escribe la trivia de `raw`: es el camino del escritor, no el lossless.
- **Errores.** Solo de construcción (`IsaError`) y de escritura (`WriteError`). Nunca
  `panic` ante input.

**Gate de verificación (salida del Stage 1).**
- Property (proptest): para cualquier `input` de bytes arbitrarios, con y sin `release`,
  `concat(frames.raw) == input` (framing lossless).
- Property: para cualquier segmento generado (id, elementos simples y compuestos), escribir
  con `write_to` y volver a tokenizar devuelve los mismos `id` y `elements`. Con `release`
  configurado, los valores pueden contener cualquier byte, delimitadores incluidos.
- Con separador `|`, un valor que contiene `*` se conserva intacto. Bytes no UTF-8 en un
  valor pasan intactos.
- Las cinco fixtures: `concat(segments.raw) == bytes del archivo`; segmentos terminados
  igual al conteo de `~` (32, 69, 51, 22, 65); `blue_cross` solo tokeniza con
  `with_delimiters` y con `new` da `NotIsa`; `trizetto` contiene un segmento `XX`.
- Costura framing→tokenizer (N7): el número de segmentos es igual al número de frames y
  los índices son consecutivos desde 0.
- Fixtures reescritas: para cada segmento no vacío, `write_to` reproduce exactamente
  `raw` sin su trivia inicial.
- Bench criterion: tokenizar las tres fixtures mayores con throughput en bytes. Sin umbral
  todavía; solo línea base registrada.

**Dependencias.** Stage 0. Sin dependencias nuevas.

**Rust que exprimes.** Lifetimes en structs e iteradores (`Segment<'a>` presta del
buffer), `&[u8]` vs `&str` (los archivos EDI no se asumen UTF-8), `Cow` como primer
contacto con prestado-o-propio (adelanta D1), `Result` y enums de error con `From` para
`?`, `Iterator` manual con estado, funciones puras y su prueba por propiedad, `io::Write`
genérico para la serialización.

**Fuera de alcance.** Reconocer qué significa un segmento (Stage 3), árbol en memoria
(Stage 2), división de repeticiones `^`, decodificación a `&str`, tokenizer por trozos
(D6), inferir `release` del archivo.

### Stage 2 · Documento lossless — APROBADO 2026-10-02

Materialización opcional del flujo (P9). El camino principal sigue siendo el iterador; el
documento existe para quien necesita acceso aleatorio, el archivo entero en memoria, o un
valor sin lifetime que cruce a Python.

**Propósito.** Un `Document` que retiene todo el archivo (N1), presta del buffer cuando
puede (N4), puede volverse dueño cuando hace falta (resuelve D1 según T5), y cuya costura
con el tokenizer se verifica (N7): recorrer el documento produce exactamente los mismos
`Segment` que el tokenizer.

**Entregable / contrato.**
- `Span { raw: Range<usize>, body: Range<usize>, terminated: bool }`: dónde vive un
  segmento dentro de los bytes. Los `raw` de los spans son contiguos y cubren todo el
  buffer sin huecos ni solapes: es la misma ley lossless de Stage 1 expresada en índices.
- `Document<'a> { bytes: Cow<'a, [u8]>, delims: Delimiters, spans: Vec<Span> }`.
  Constructores: `Document::parse(bytes: impl Into<Cow<'a, [u8]>>) -> Result<Self,
  IsaError>` (lee el ISA, tolera trivia inicial) y `Document::with_delimiters(bytes,
  Delimiters)`. Aceptan `&'a [u8]` (presta) o `Vec<u8>` (posee) con la misma firma.
- Construir el documento solo hace *framing*: no parsea elementos. Es más barato que
  tokenizar; los elementos se parsean al pedir un segmento.
- Acceso: `len()`, `is_empty()`, `as_bytes() -> &[u8]`, `delimiters()`, `spans() ->
  &[Span]`, `segment(i) -> Option<Segment<'_>>` (índice 0-based, igual que
  `Segment::index`), `segments() -> Segments<'_, 'a>` (iterador con estado propio), y
  `IntoIterator for &Document` para `for segment in &doc`.
- `into_owned(self) -> Document<'static>`: copia los bytes solo si eran prestados;
  los spans se reutilizan. Es la única copia del archivo que existe en el crate.
- Sin errores nuevos: `parse` devuelve `IsaError`; todo lo demás es infalible.

**Gate de verificación (salida del Stage 2).**
- Las cinco fixtures: `doc.as_bytes() == archivo`; `doc.len()` igual al número de
  segmentos del tokenizer; `doc.segment(i)` igual (con `==`, campo a campo, `index`
  incluido) al i-ésimo `Segment` del tokenizer; `concat(spans.raw) == archivo`;
  `into_owned()` produce los mismos segmentos.
- Property: para cualquier `input` y delimitadores con/sin release, los segmentos del
  documento son iguales a los del tokenizer, y los spans particionan `0..len` sin huecos.
- Unit: documento vacío; `segment` fuera de rango es `None`; `parse` desde `Vec<u8>` da
  `Document<'static>`; tras `into_owned()` el buffer original puede soltarse y el documento
  sigue siendo usable (lo comprueba el compilador); `for s in &doc` funciona.
- Bench: construir el documento de las tres fixtures mayores, junto al bench de tokenizar,
  para registrar que indexar es más barato que tokenizar.

**Dependencias.** Stage 1. Sin dependencias nuevas.

**Rust que exprimes.** `Cow` como campo de struct y `impl Into<Cow<'a, [u8]>>` en firmas
(una API que presta o posee sin duplicarse); métodos `&self -> Segment<'_>` (prestar del
propio struct); un iterador manual con dos lifetimes (`Segments<'d, 'a>`); `IntoIterator`
para `&T`; `Range<usize>` como índice en lugar de punteros (arena); `into_owned` y la
promoción a `'static`.

**Fuera de alcance.** Árbol de loops (Stage 3), mutación del documento, escritura distinta
de `as_bytes`, tokenizer por trozos (D6).

### Stage 3 · Motor declarativo de loops — APROBADO 2026-10-02

La capa que convierte un flujo plano de segmentos en una jerarquía de loops, sin saber nada
del 835: todo el conocimiento del estándar llega como datos (P2, P6). Resuelve D2.

**Propósito.** Un intérprete genérico `LoopEngine` alimentado segmento a segmento (P4) que
emite eventos (`LoopOpened`, `LoopClosed`, `Captured`, `Unmatched`) según una *spec-dato*
que describe la estructura de loops. El 835 que shippea el crate es una spec más; un
usuario la extiende o sobrescribe dando datos en el mismo formato (N3). Nada se descarta:
un segmento que la spec no reconoce se emite como `Unmatched` con su índice (N1).

**Decisiones de diseño (cada una con la alternativa descartada).**
- **T6 · Formato: JSON con `serde`.** La spec se deserializa con `serde` a un struct
  `Spec`; el built-in del 835 vive como `specs/835.json` embebido con `include_str!`, de
  modo que built-in y extensión son literalmente el mismo formato. JSON porque es lo que un
  `dict` de Python ya es, no tiene ambigüedades y `serde_json` es ubicuo. Descartado TOML
  (más legible a mano pero una segunda sintaxis que Python no habla nativamente) y YAML
  (ambigüedades, crate de referencia sin mantener). Descartado hardcodear el 835 en Rust:
  rompe N3.
- **T7 · Dependencias: `serde` y `serde_json` entran al core.** El gate "`[dependencies]`
  vacío" pasa a ser "sin dependencias de runtime ni de I/O". Son librerías de datos puras,
  compilan a WASM, y escribir un parser JSON a mano no enseña nada que importe aquí.
- **T8 · Loops planos con `parent`, fusión por JSON Merge Patch (RFC 7386).** Los loops
  se declaran como un mapa `id → { parent, trigger, segments, end? }`, no como árbol
  anidado. Así "añadir el segmento propietario ZZ1 al loop 2100" es un parche de tres
  líneas y "sobrescribir un loop" es reemplazar una clave. La fusión sigue la semántica
  estándar de merge patch: objetos se fusionan en profundidad, arrays y escalares se
  reemplazan, `null` borra. Es la misma operación que `dict.update` recursivo en Python.
  Descartado el árbol anidado (fusionar exige rutas) y una sintaxis de parche propia.
- **T9 · Algoritmo de detección con ancestros implícitos.** Para cada segmento, en orden:
  (a) si dispara un loop hijo de *cualquier* loop abierto, buscando del más interno al más
  externo y por último la raíz, se cierran los loops por encima de ese padre y se abre el
  hijo (esto cubre abrir un hijo del loop actual, repetir un hermano y abrir un primo); los
  disparadores ganan a la captura: un segmento que dispara un loop alcanzable abre ese loop
  aunque el loop actual lo liste entre sus segmentos; (b) si algún loop abierto lo acepta,
  del más interno al más externo, se cierran los de encima y se captura; si es el segmento
  `end` de ese loop, además se cierra; (c) si nada encaja, `Unmatched` y la ruta no cambia. Un disparador cuyo loop no es hijo de ningún loop abierto pero cuya cadena
  de ancestros llega a la raíz abre esos ancestros con `implicit: true`: así un fragmento
  que empieza en `ST` (como `blue_cross`) produce una transacción dentro de un sobre
  implícito en vez de treinta segmentos `Unmatched`. Descartado el modo estricto (fragmentos
  ilegibles) y el modo laxo que abre cualquier loop en cualquier sitio (árboles inválidos).
- **T10 · Los eventos referencian segmentos por índice**, no por copia (consecuencia de
  T5). `feed` devuelve un slice de un buffer interno que se reutiliza: cero allocs por
  segmento en régimen (N4). Los ids de loop son índices internos (`LoopId`) resueltos a
  nombre por la spec, no `String`s en cada evento.

**Entregable / contrato.**
- Módulo `spec`: `Spec` (deserializable), `Spec::builtin_835()`, `Spec::from_json(&str)`,
  `Spec::merge_patch(&self, patch: &str) -> Result<Spec, SpecError>`, validación al cargar
  (padres existen, sin ciclos, al menos un loop, posiciones `where` canónicas, triggers de
  hermanos no idénticos) con `SpecError` que nombra el loop culpable, el ciclo completo, y
  si el fallo vino de un parche. Esquema de un loop:
  `{"parent": "2000", "trigger": {"segment": "CLP"}, "segments": ["CLP","CAS","NM1",…]}`;
  el trigger admite condiciones por posición: `{"segment":"N1","where":{"1":"PR"}}`;
  `"end": "SE"` marca el segmento que cierra el loop al capturarse.
- Módulo `engine`: `LoopEngine<'s>` con `new(&'s Spec)`, `feed(&mut self, &Segment<'_>)
  -> &[Event]`, `finish(&mut self) -> &[Event]` (cierra lo abierto), `path(&self) ->
  &[LoopId]`. `Event` es `Copy`: `LoopOpened { id, implicit }`, `LoopClosed { id }`,
  `Captured { id, segment: usize }`, `Unmatched { segment: usize }`.
- Módulo `tree`: `LoopTree` construido desde los eventos (o directamente desde un
  `Document` con `LoopTree::build(&Spec, &Document)`): nodos con loop, flag `implicit`,
  hijos e índices de segmentos capturados; los `Unmatched` cuelgan del nodo donde
  ocurrieron. Es lo que Stage 4 proyecta y lo que Stage 5 devuelve a Python.
- Spec built-in `835` (sirve a 4010 y 5010): sobre `interchange` (ISA/IEA) → `group`
  (GS/GE) → `transaction` (ST/SE, con BPR TRN CUR REF DTM y PLB) → `1000A` (N1*PR), `1000B`
  (N1*PE), `2000` (LX, TS3, TS2) → `2100` (CLP…) → `2110` (SVC…).

**Gate de verificación (salida del Stage 3).**
- Golden files: para las cinco fixtures y los cuatro samples pequeños, el flujo de eventos
  serializado línea a línea se compara con un archivo comprometido; para los dos samples
  grandes, un resumen (segmentos capturados por loop, loops abiertos por id, `Unmatched`).
  Regenerables con una variable de entorno, nunca editados a mano.
- Invariantes sobre los once archivos: cada índice de segmento aparece exactamente una
  vez entre `Captured` y `Unmatched`; los eventos abren y cierran balanceados; tras
  `finish` no queda nada abierto; el número de nodos `2100` es el número de `CLP` y el de
  `2110` el de `SVC`.
- Caso real de N3: el `XX` espurio de `trizetto` es `Unmatched` con la spec built-in y
  `Captured` en `2100` con un parche de usuario de tres líneas. Un loop propietario nuevo
  (trigger inventado bajo `2100`) se abre y captura solo con datos.
- `blue_cross` (fragmento) produce una transacción bajo `group` e `interchange` implícitos,
  con cero `Unmatched`.
- Unit sobre el motor con segmentos construidos a mano: cada regla (a)–(f) por separado,
  repetición de loop hermano, `end` que cierra, trigger con `where` que no coincide,
  `Unmatched` en la raíz y en un loop profundo.
- Unit sobre la spec: parche que añade un segmento a un loop, que sobrescribe un trigger,
  que añade un loop, que borra con `null`; errores de validación con el loop nombrado.
- Property: alimentar un documento entero produce exactamente los mismos eventos que
  alimentarlo partido en cualquier punto en dos motores encadenados por estado (`path`), o
  más simple: el resultado no depende de que los segmentos lleguen de un `Tokenizer` o de
  un `Document`.
- Bench: motor sobre los tres samples mayores, eventos por segundo y MiB/s.

**Dependencias.** Stage 2 (índices), `serde`, `serde_json`.

**Rust que exprimes.** Enums con datos como máquina de estados y `match` exhaustivo;
`serde` derive y `#[serde(deny_unknown_fields)]`; `Vec` como pila; lifetimes `'s` para
prestar la spec desde el motor; índices internados en vez de `String`; recursión sobre
`serde_json::Value` para el merge patch; construcción de un árbol con índices (arena);
validación con `Result` y errores que señalan el dato culpable.

**Fuera de alcance.** Nombres y tipos de elementos (`segments` en la spec, Stage 4);
validación de obligatoriedad y cardinalidad (SNIP, Stage 4); división de repeticiones.

### Stage 4 · Proyección a dominio + validación — APROBADO 2026-10-03

La capa que da significado de negocio al árbol: convierte eventos y segmentos en tablas
columnares tipadas y en diagnósticos SNIP, sin que el código sepa nada del 835: nombres,
tipos, obligatoriedad y el mapeo a tablas llegan como datos (P2, P6, N3). Resuelve D10 y
cierra, dentro de su pase de validación de specs, las issues #7, #17, #18 y #19.

**Propósito.** Dos consumidores sobre un único recorrido del flujo de segmentos (P4): un
`Projector` que llena columnas con la disposición de memoria de Arrow a partir de una
sección `tables` de la spec y emite los diagnósticos de tipo y obligatoriedad al leer cada
elemento, y un `EnvelopeChecker` que verifica la integridad de sobres (SNIP 1) a partir de
los eventos del motor. Todo fallo de datos es un `Diagnostic` que responde qué regla, dónde
y con qué dato (P10) y el flujo continúa (P7). Lo que Stage 5 entrega a Python son estas
tablas, sin copia, y esta lista de diagnósticos.

**Decisiones de diseño (cada una con la alternativa descartada).**
- **T11 · `Diagnostic` es una capa aparte y se explica solo.** Un struct propio con
  `rule: Rule` (enum con datos por variante), `level: SnipLevel`, `segment: Option<usize>`,
  `element: Option<usize>`, `component: Option<usize>`, `path: Vec<LoopRef>` (nombre del
  loop y ordinal de la instancia, `2100#3`) y `datum: Vec<u8>` (el valor ofensivo tal
  cual). Guarda valores propios, no ids, para que su `Display` sea autocontenido y sea el
  contrato que P10 exige, con un test por variante; el rango de bytes se resuelve desde
  `Document::spans` con el índice del segmento, como P10 ya admite. Son el camino frío, así
  que la asignación por diagnóstico no cuesta. Descartado emitir diagnósticos como eventos
  del motor (el motor dejaría de ser genérico, P1) y descartado guardar `LoopId`s y
  renderizar con la spec (un `Display` que necesita contexto no se explica solo).
- **T12 · `LoopOpened` lleva el índice del segmento que lo abrió.** `Event::LoopOpened
  { id, implicit, segment }`: en una apertura explícita es el disparador; en una implícita
  es el disparador del descendiente que forzó la cadena. El `Node` del árbol gana
  `opened_by: Option<usize>` (solo la raíz es `None`). `Event` sigue siendo `Copy` y del
  mismo tamaño por alineación; los goldens de eventos se regeneran una vez y se revisa el
  diff (solo las líneas `open` ganan `#índice`). Cierra #19. Descartado correlacionar la
  apertura implícita con el `Captured` que la sigue: frágil y duplica lógica del motor.
- **T13 · La spec gana una sección `segments`, global y clavada por posición.**
  `"segments": { "CLP": { "elements": { "1": { "name": "claim_id", "type": "AN",
  "required": true, "min": 1, "max": 38 }, … } } }`. Un `CLP` tiene los mismos elementos
  esté en el loop que esté, así que se define una vez por id de segmento, no por loop. Las
  posiciones como claves (no un array) siguen la convención de `where` y hacen que un
  parche de usuario retoque un solo elemento, porque el merge patch fusiona objetos y
  reemplaza arrays. Tipos: `AN`, `ID`, `N0`…`N9` (entero con decimales implícitos), `R`
  (decimal; `scale` opcional, por defecto 2), `DT`, `TM`; `composite` anida subelementos
  por posición. Un segmento listado en un loop sin entrada en `segments` es válido y
  opaco (N3). El pase de validación de la spec, en carga, además: rechaza ids vacíos en
  `segments`, `end` y la sección nueva, nombrando loop y entrada (#7); comprueba sobre
  `serde_json::Value` que la raíz, `loops`, cada loop, cada trigger, `where`, `segments`,
  cada segmento y sus `elements` son objetos, y lo dice en palabras llanas, "the spec must
  be a JSON object; found an array" (#18); rechaza dos hermanos cuyos disparadores se solapan,
  nombrando ambos loops y sus condiciones (#17): mismo segmento, ninguna posición presente
  en ambos `where` con valores distintos, y ningún conjunto de condiciones es superconjunto
  estricto del otro (un `N1` desnudo junto a `N1 {1:PR}` es un comodín válido porque el
  motor prefiere al más específico; `N1 {1:PR}` junto a `N1 {2:X}` se rechaza porque solo el
  nombre decidiría; `N1*PR` y `N1*PE` se excluyen, el built-in carga). Los ordinales de la
  ruta de un diagnóstico (`2100#3`) cuentan instancias del loop en todo el flujo. Descartado definir elementos dentro
  de cada loop (repetición y el dolor de #14) y elementos como array (un parche
  reemplazaría la lista entera).
- **T14 · Columnas propias con la disposición de Arrow, sin el crate `arrow`.** Un
  `Column` por tipo con `validity` como bitmap (un bit por fila, LSB primero) y los buffers
  que Arrow espera: `Binary` (offsets `i32` + bytes crudos, sin decodificar: lossless y
  cero-copia) para `AN` e `ID`; `Int64` para `N`n con la escala implícita como metadato;
  `Decimal128` (`i128` + escala por columna) para `R`; `Date32` para `DT`; `Time32` en
  segundos para `TM`. Un valor que no cumple su tipo o excede la escala es un diagnóstico y
  un nulo en la columna. Así Stage 5 expone las tablas por la interfaz C de Arrow sin
  copiar y el core no suma dependencias (P3, T7). Descartadas las filas como structs
  (una asignación por fila y transponer después) y depender de `arrow` en el core.
- **T15 · La proyección es declarativa: sección `tables` en la spec.** Una tabla ancla en
  uno o más loops (`"loops": ["2100", "2110"]`), opcionalmente en un segmento que se repite
  dentro de ellos (una fila por aparición) y opcionalmente en un grupo de elementos que se
  repite dentro del segmento (`"repeat": { "from": 2, "step": 3 }`, una fila por grupo:
  así `CAS` y `PLB` se explotan). Fuentes de columna: `element` (del primer segmento que
  cumple `segment` y `where`, en el loop ancla o en un loop descendiente nombrado con
  `loop`, con `element` y `component` opcional) y `segment_index`. Toda tabla recibe
  automáticamente `segment` (índice del segmento ancla) y una columna de índice por cada
  tabla padre (`claims.payment`, `services.claim`, `adjustments.claim` y
  `adjustments.service`, nulo cuando el ajuste es de claim). Los índices son ordinales
  globales, así que drenar las tablas por transacción no los invalida. Los tipos salen de
  `segments`; una columna sin definición de elemento es `Binary`. El built-in 835 trae
  `payments` (transaction), `claims` (2100), `services` (2110), `adjustments` (CAS en 2100
  y 2110) y `provider_adjustments` (PLB). Un payer con un `REF` propio añade una columna con
  un parche de tres líneas, igual que hoy añade un segmento a un loop. Descartadas las
  tablas del 835 escritas en Rust (rompe N3 y P2).
- **T16 · Un solo recorrido, dos consumidores.** `Processor<'s>` envuelve `LoopEngine`,
  `EnvelopeChecker` y `Projector`: `feed(&Segment) -> &Output` entrega eventos y
  diagnósticos nuevos; `finish()` cierra; `take_tables()` drena las filas acumuladas
  (memoria acotada claim a claim, P9, N4); `Processor::run(&spec, &Document) -> (Tables,
  Vec<Diagnostic>)` es la conveniencia de documento completo y recorre exactamente el mismo
  código. Validar tipo y obligatoriedad ocurre en el mismo acceso al elemento que llena la
  columna: no hay segunda lectura del árbol. Descartado un segundo recorrido del `LoopTree`
  para validar.
- **T17 · Alcance SNIP: niveles 1 y 2 completos; 3 opcional; 4–7 fuera.** Nivel 1, desde
  los eventos: `SE01` cuenta los segmentos `ST`…`SE`; `ST02`=`SE02`; `GS06`=`GE02`;
  `ISA13`=`IEA02`; `GE01` = número de `ST`; `IEA01` = número de `GS`; segmento `Unmatched`
  → `UnknownSegment`; apertura implícita → `ImplicitLoop` con el segmento causante (T12);
  loop con `end` cerrado sin haberlo capturado → `UnterminatedLoop`. Nivel 2, desde
  `segments`: `RequiredElementMissing`, `TypeMismatch`, `LengthOutOfRange`,
  `CompositeShape`. Nivel 3 (cuadre `BPR02` contra claims y `PLB`; `CLP03`−`CLP04` contra
  los `CAS` del claim y sus servicios; `SVC02`−`SVC03` contra los `CAS` del servicio) entra
  solo como última tarea opcional del plan y solo si sobre las tablas es una suma por grupo
  declarable en tres líneas de spec; si exige un lenguaje de reglas, pasa a D11. Fuera:
  situacionales (4), listas de códigos externos (5), tipos de producto (6), trading
  partner (7) y la cardinalidad de segmentos por loop (D11).

**Entregable / contrato.**
- Módulo `diagnostic`: `Diagnostic`, `Rule` (variantes de nivel 1 y 2 arriba, cada una con
  los datos que su `Display` necesita: esperado, encontrado, índices, path), `SnipLevel`,
  `LoopRef`. `Display` por variante es contrato; `Diagnostic::span(&self, &Document) ->
  Option<Span>` resuelve el rango de bytes.
- Módulo `spec` ampliado: `SegmentDef`, `ElementDef`, `ElementType`, `TableDef`,
  `ColumnSource`; `Spec::segment(id) -> Option<&SegmentDef>`, `Spec::tables() ->
  &[TableDef]`; variantes nuevas de `SpecError`: `NotAnObject { path, found }`,
  `EmptySegmentId { loop, key }`, `OverlappingTriggers { parent, a, b, conditions }`,
  `BadElementDef { segment, position, reason }`, `BadColumn { table, column, reason }`, con
  `Display` probado y `source()` donde haya causa.
- Módulo `engine`: `Event::LoopOpened { id, implicit, segment }`. Módulo `tree`:
  `Node::opened_by`.
- Módulo `column`: `Bitmap`, `Column` (`Binary`, `Int64`, `Decimal128`, `Date32`,
  `Time32`), `Table { name, columns: Vec<(String, Column)> }`, `Tables`; `len()` igual en
  todas las columnas de una tabla, invariante comprobado.
- Módulo `project`: `Projector<'s>` (`new(&Spec, &Delimiters)`, el separador de componentes
  hace falta para leer como un solo texto un elemento declarado sin composite que el
  tokenizer partió, como `ISA16`; `on(&Segment, &[Event]) -> &[Diagnostic]`, `take_tables()`). Módulo `check`: `EnvelopeChecker<'s>` (misma firma de `on`). Módulo
  `process`: `Processor<'s>` con `feed`, `finish`, `take_tables`, `diagnostics()` y
  `Processor::run`.
- Spec built-in `835` con `segments` para todos los segmentos que lista (ISA, GS, ST, BPR,
  TRN, CUR, REF, DTM, N1, N3, N4, PER, RDM, LX, TS3, TS2, CLP, CAS, NM1, MIA, MOA, AMT,
  QTY, SVC, LQ, PLB, SE, GE, IEA) y las cinco `tables`.

**Gate de verificación (salida del Stage 4).**
- Golden files sobre los once archivos: por archivo, cada tabla serializada fila a fila
  (resumen de conteos para los dos samples grandes) y la lista de diagnósticos con su
  `Display`; mismo interruptor `UPDATE_GOLDEN` y misma detección de huérfanos. Los goldens
  de eventos cambian una vez por T12 y el diff muestra solo `#índice` en las líneas `open`.
- Invariantes sobre los once: filas de `claims` = número de `CLP`, `services` = `SVC`,
  `adjustments` = grupos `CAS` con código de motivo, `provider_adjustments` = grupos `PLB`;
  todo índice de padre está en rango y apunta a la fila cuyo loop contiene al ancla; todo
  `segment` de fila es un índice capturado por un nodo del loop ancla; columnas de una
  tabla con la misma longitud y bitmap coherente con offsets; drenar por transacción y
  concatenar == `Processor::run` (incremental == documento, P9).
- Anomalías conocidas renderizadas: el `XX` de trizetto → un `UnknownSegment` con índice,
  path y `datum`; blue_cross → dos `ImplicitLoop` que nombran `ST` en `#0`; multi_claim →
  cuatro `UnknownSegment` dentro de `2100#1` y `2100#2`. Unit con segmentos a mano: `SE01`
  erróneo, `ST02`≠`SE02`, `GE01` erróneo, `ST` sin `SE`, `CLP03` no numérico (nombra
  segmento, elemento 3 y el texto), `CLP01` vacío siendo obligatorio, `DTM02` con fecha
  inválida, `R` con más decimales que la escala, composite con más componentes de los
  declarados. Un test de `Display` de texto completo por variante de `Rule` y de las
  variantes nuevas de `SpecError` (P10).
- Spec: `""` en `segments`, `end` y en la sección nueva se rechaza nombrando loop y
  entrada (#7); `[]` en la raíz, `loops` como array y trigger como array se rechazan con el
  mensaje en palabras llanas (#18); dos hermanos `N1 where {1:PR}` y `N1 where {2:X}` se
  rechazan nombrando ambos (#17) y el built-in carga; un parche de tres líneas añade una
  columna `REF` propia a `claims` y la tabla proyectada la muestra (N3).
- Property: valores aleatorios válidos de `N`n, `R`, `DT` y `TM` se parsean a su columna y
  se vuelven a formatear iguales; bytes aleatorios en cualquier elemento nunca producen un
  pánico, solo diagnósticos o nulos (P7); filas aleatorias escritas en `Column` se leen de
  vuelta iguales con su bitmap.
- Costura (N7): motor→proyector (eventos y segmentos construidos a mano producen las filas
  esperadas) y end-to-end por `Processor::run` sobre los once archivos.
- Bench: proyección completa sobre los tres samples grandes en bytes/s y filas/s; la base
  va en el mensaje del commit.

**Rust que exprimes.** Traits como contrato entre consumidores del mismo flujo; enums con
datos como vocabulario de diagnósticos y `Display` como contrato; bitmaps a mano (`u8`,
desplazamientos, máscaras); `i128` y parseo numérico sin asignar; `serde` con enums
etiquetados o `untagged` para `ColumnSource`; borrar la vida del motor con `take` de
buffers (drenar sin realojar); `impl Trait` en argumentos; iteradores sobre grupos de
elementos (`chunks`, `step_by`).

**Fuera de alcance.** SNIP 4–7; listas de códigos externos (CARC, RARC); cardinalidad de
segmentos por loop y lenguaje de reglas de cuadre (D11); interfaz C de Arrow y PyO3 (Stage
5); medir `Cow` frente a `Arc` (D8); escritor (D7).

### Stage 5 · Binding Python — APROBADO 2026-10-03

El stage que devuelve el proyecto a su problema original: ingerir 835 desde Python más
rápido que la librería vieja, sin perder nada por el camino. El core no cambia de contrato;
el binding lo envuelve, libera el GIL y entrega las tablas de Stage 4 sin copiarlas. Mide D8.

**Propósito.** Un paquete Python `oxedi835` construido con PyO3 y maturin que expone
`parse`, `stream`, `Spec`, `Document` y `Diagnostic`; las tablas salen por el protocolo
PyCapsule de Arrow para que Polars, pyarrow o DuckDB las consuman sin copia y sin que el
paquete dependa de ninguno de ellos. Todo el trabajo corre con el GIL liberado (N4). Los
diagnósticos son valores, nunca excepciones (P7); las excepciones quedan para los errores de
uso: spec inválida (`SpecError`) y entrada sin `ISA` (`ParseError`) con el texto del `Display`
del core, más las nativas de Python para índices, claves y tipos de argumento, todas con
regla, lugar y dato (P10).

**Decisiones de diseño (cada una con la alternativa descartada).**
- **T18 · PyO3 + maturin, wheels `abi3`, Python ≥ 3.11** (3.9 quedó fuera de soporte en
  octubre de 2025 y la ABI estable de 3.11 da acceso al protocolo buffer, así `parse`
  copia una sola vez cualquier objeto con buffer). Crate nuevo `crates/oxedi835_py`
  (`cdylib`) en el workspace; el core sigue sin más dependencias que `serde` y `serde_json`
  (P3, T7). Un wheel por plataforma, no por versión de Python. Descartado `ctypes` con
  cbindgen (sin tipos ni gestión del GIL) y UniFFI (sin Arrow ni iteradores naturales).
- **T19 · Copia única del buffer; D8 se mide aquí.** `parse` acepta cualquier objeto con
  protocolo buffer, copia los bytes una vez y construye un `Document<'static>` propio. Con
  ese documento en mano se mide D8: `Cow` propio frente a `Arc<[u8]>` con N documentos
  retenidos desde Python, en tiempo y memoria sobre los samples grandes; el resultado se
  registra en §6.1 y, si `Arc` gana, el cambio es local a la representación del buffer
  (T5). Descartado prestar del `PyBytes` sin copiar: ata la vida del documento a un objeto
  Python y complica cada método; el coste de la copia es de milisegundos por archivo.
- **T20 · GIL liberado en todo pase.** `parse`, `stream` y la escritura corren dentro de
  `allow_threads`; los tipos que cruzan son `Send` porque el core no comparte estado. El
  paralelismo real es entre archivos, desde un `ThreadPoolExecutor` (T2).
- **T21 · Arrow por PyCapsule con el crate `arrow` solo en el binding.** Las columnas de
  Stage 4 ya tienen la disposición de Arrow; el binding las envuelve como `Buffer`s sin
  copiar, forma `RecordBatch`es y los expone por `__arrow_c_stream__` /
  `__arrow_c_array__`. El `unsafe` de la interfaz C lo escribe arrow-rs, no nosotros.
  Descartado implementar la interfaz C a mano (más código inseguro para lo mismo) y
  devolver listas o numpy (copia, pierde el sentido de T14).
- **T22 · API pequeña y fiel al core.** `oxedi835.parse(data, spec=None) -> Result` con
  `.tables` (mapa nombre → tabla exportable a Arrow, con `.render()` que produce el mismo
  texto que los goldens), `.diagnostics` (lista de `Diagnostic` con `level`, `rule`,
  `segment`, `element`, `component`, `path`, `datum` y `__str__` igual al `Display`) y
  `.document` (`len`, indexado por posición con `id`, `elements` y `raw`, `write() ->
  bytes` idéntico a la entrada, N1). `oxedi835.stream(data, spec=None, by="transaction")`
  itera lotes de tablas por transacción con memoria acotada (es `take_tables` al cerrar
  cada `transaction`, P9). `Spec.builtin()`, `Spec.from_json(str)`, `Spec.patch(dict | str)
  -> Spec`, `Spec.to_json()`. `oxedi835.parse_file(path)` lee en Python y llama a `parse`:
  la I/O vive en la capa Python, no en el core. `SpecError(ValueError)` con el texto del
  `Display`. Descartada una API de objetos Claim/Service como la librería vieja: eso lo
  cubre 5b con una spec, no con código (N3).
- **T23 · Verificación sobre los mismos oráculos.** pytest recorre los once archivos y
  compara `tables.render()` byte a byte con `tests/golden/project/*.tables.txt` y los
  `str(diagnostic)` con `*.diagnostics.txt`; `document.write()` reproduce cada archivo; una
  prueba consume `.tables["claims"]` desde Polars por el protocolo PyCapsule y comprueba
  filas y tipos; un test de memoria recorre el sample mayor con `stream` y comprueba con
  `tracemalloc`/RSS que el pico queda acotado por una transacción, no por el archivo; un
  test de concurrencia comprueba que dos hilos parsean en paralelo (tiempo total menor que
  la suma). Un script, no un gate, cronometra `parse` frente a `edi-835-parser` en los
  archivos que ambos leen (adelanta 5b). El gate final instala el wheel en un venv limpio y
  ejecuta pytest desde fuera del repo.

**Entregable / contrato.**
- `crates/oxedi835_py`: `Cargo.toml` (`pyo3` con `abi3-py311`; el modo extensión se activa con
  `PYO3_BUILD_EXTENSION_MODULE` en `.cargo/config.toml` porque PyO3 0.29 retira la feature;
  los crates `arrow-*` solo con `ffi`), `pyproject.toml`
  con maturin, `src/lib.rs` con el módulo y las clases `Spec`, `Document`, `Segment`,
  `Tables`, `Table`, `Diagnostic`, `Result`, `Stream`; `python/oxedi835/__init__.py` con
  `parse_file` y los re-exports; `tests/` en pytest.
- Las clases Python no reimplementan nada: cada método delega en el core; el único código
  con lógica propia es el puente de columnas a `RecordBatch` y la conversión de
  `Diagnostic` a atributos.
- Baseline de D8 y del tiempo de `parse` por archivo en el mensaje de commit del bench.

**Gate de verificación (salida del Stage 5).**
- `cargo test --workspace --locked`, clippy, fmt, bench `--no-run` y `cargo doc` siguen en
  verde para todo el workspace; `maturin develop` y `pytest` en verde en local y en CI.
- Goldens de tablas y diagnósticos reproducidos desde Python en los once archivos; `write()`
  byte a byte; prueba Polars; test de memoria; test de concurrencia.
- D8 medido y registrado; decisión tomada en §6.1 con los números.
- Wheel instalado en un venv limpio pasa el smoke test.

**Rust que exprimes.** FFI con PyO3: `#[pyclass]`, `#[pymethods]`, `Py<T>` y `Bound<T>`,
el token del GIL y `allow_threads` con límites `Send`; borrar vidas con tipos propios en la
frontera; el protocolo PyCapsule de Arrow; maturin y `abi3`; `Arc` frente a `Cow` medido,
no supuesto; errores Rust a excepciones Python conservando el texto.

**Fuera de alcance.** Publicar en PyPI y la matriz manylinux/macOS/Windows (Stage 6);
`asyncio`; API orientada a objetos del 835 (5b); escritor (D7); tokenizer por trozos (D6).

### Stage 5b · Compatibilidad con `edi-835-parser` — APROBADO 2026-10-04

Oráculo de compatibilidad frente a la única librería Python específica del 835 con usuarios
reales (sondeo del 2026-10-03), y camino de migración en dos velocidades. Resuelve D12.

**Propósito.** Demostrar sobre archivos reales que la spec-dato reproduce el resultado de una
librería escrita a mano (N3), mostrar con los mismos archivos lo que esa librería pierde y
nosotros conservamos (N1), y ofrecer a sus usuarios un cambio de `import` hoy y una migración
a nuestra API nativa mañana.

**Hechos de partida** (`edi-835-parser` 1.8.0, medidos el 2026-10-04 sobre `united`).
`to_dataframe()` devuelve una fila por servicio (6.192); los claims sin servicios no aparecen.
26 columnas, de las que `adj_<n>_{group,code,amount}` y `ref_<n>_{qual,value}` son
dinámicas: hay tantas como repeticiones máximas tenga el archivo. Importes en `float64`,
fechas en `datetime64`, `was_forwarded` booleano. `count_claims()` 1.332, `count_patients()`
1.212, `sum_payments()` 173.305,0 (float). `payer`/`payee` son objetos con `name`,
`identification_code`, `address`, `location`. La librería hace `int()` sobre `N104` y falla
con ids `XV` alfanuméricos (#46).

**Decisiones de diseño (cada una con la alternativa descartada).**
- **T25 · El DataFrame compatible sale de una spec, y lo dinámico se resuelve con una tabla
  larga y un pivote.** Una spec `edi_835_parser` (parche sobre la built-in, en
  `specs/edi_835_parser.json`) declara una tabla `rows` anclada en `2110` con las columnas
  fijas de esa librería y tres tablas largas, `rows_adjustments` (grupos `CAS` del servicio),
  `rows_references` (`REF` del servicio) y `rows_remarks` (`LQ` del servicio), con su ordinal
  dentro del servicio. La capa Python pivota las largas a `adj_<n>_*` / `ref_<n>_*` /
  `rem_<n>_*` en el mismo orden que la librería. Lo que el formato `tables` aún no expresa
  (leer un loop que envuelve, distinguir ausente de vacío, elegir la última coincidencia) se
  calcula en Python con una línea de justificación cada uno y queda en una issue para
  extender el formato antes del escritor. Toda la
  semántica del 835 (qué segmento, qué cualificador, qué elemento) es dato; el código solo
  pivota y convierte tipos. Descartado reconstruir las 26 columnas con joins en Python sobre
  nuestras tablas nativas: escondería en código lo que la spec debe probar.
- **T26 · Paridad estricta por defecto; lo recuperado, a petición.** `to_dataframe()` devuelve
  exactamente lo que devuelve la librería (mismas filas, columnas, orden y tipos: `float64`,
  `datetime64`, `bool`); `to_dataframe(extended=True)` añade los claims sin servicios, los
  ajustes a nivel de claim, `PLB` y los `REF`/`AMT` que la librería no mapea, cada columna
  añadida con prefijo `x_` para que nunca choque. El diff entre ambos es la demostración de
  la parte 2. Descartado devolver siempre más: rompería a quien migra.
- **T27 · Superficie completa en `oxedi835.edi_835_parser`, tras el extra
  `oxedi835[edi-835-parser]`.** `parse(path | dir) -> TransactionSets`; `TransactionSets` con
  `__iter__`, `__len__`, `__repr__`, `count_claims`, `count_patients`, `sum_payments`,
  `sort_columns`, `to_dataframe`; `TransactionSet` con `payer`, `payee`, `to_dataframe`,
  `serialize_service`, y los objetos `interchange`, `financial_information`, `claims`,
  `organizations`, `Claim` (`claim`, `entities`, `services`, `references`, `dates`, `amount`)
  y `Service` (`service`, `dates`, `references`, `remarks`, `amount`, `adjustments`) como
  vistas de solo lectura sobre nuestro `Document` y nuestras tablas, sin reinterpretar bytes.
  El extra declara `pandas` y `pyarrow`; el paquete base no gana dependencias. Descartado el
  nombre genérico "compat". **Extensión acordada el 2026-10-04: lectura desde bytes**, la
  carencia que más sufren sus usuarios (la librería solo acepta rutas y obliga a escribir a
  disco lo que llega de S3, de una API o de una base de datos). `parse(path, debug=False)` es
  un reemplazo directo: misma firma y mismo comportamiento que el original (ruta o
  directorio, nada más); la lectura desde memoria se añade con métodos propios,
  `parse_bytes(data, file_path=None)` (`bytes`/`bytearray`/`memoryview`),
  `parse_file_obj(f, file_path=None)` (objetos binarios con `read()`) y `parse_many(items)`
  para una lista de bytes o de archivos; `file_path` rellena el atributo
  que la librería guarda en cada `TransactionSet` (por defecto `"<bytes>"`). Todo cae en el
  mismo `oxedi835.parse` con el GIL liberado; leer de ruta o de bytes da el mismo
  DataFrame, y un test lo comprueba. Ante archivos vacíos o que no son 835, la capa imita a
  la librería (devuelve un conjunto con `interchange` vacío y sigue); esa tolerancia vive solo
  en `oxedi835.edi_835_parser`: la API nativa mantiene el `ParseError` estricto (P10).
- **T28 · Equivalentes nativos sin pandas.** En `oxedi835.Result`: `count_claims()`,
  `count_patients()`, `sum_payments() -> Decimal`, `payer`, `payee`; en `Tables`/`Table`:
  `to_polars()` y `to_pandas()` con importación perezosa y error que nombra el extra
  (`oxedi835[polars]`, `oxedi835[pandas]`). Se calculan sobre las tablas Arrow, no sobre
  pandas. La tabla "método viejo → método nuevo" va en el README ahora y en el libro de
  Stage 8.
- **T29 · Dos oráculos.** (a) `scripts/compat_oracle.py` lee un directorio fuera del repo (los
  originales), corre ambas librerías con el shim de `N104` y escribe un informe de
  diferencias por archivo y columna sin volcar datos; no es un gate. (b) En CI, el mismo
  cotejo sobre los seis samples anonimizados con `edi-835-parser` instalado como dependencia
  de desarrollo y el shim aplicado: `to_dataframe()` igual celda a celda (`pandas.testing.
  assert_frame_equal`), y `count_*`/`sum_payments` iguales. Los goldens de la tabla `rows` se
  generan como los demás. Además, un test con `duckdb` (dependencia de desarrollo) consulta
  las tablas por PyCapsule.

**Entregable / contrato.**
- `specs/edi_835_parser.json` (parche sobre la built-in) con las tablas `rows`,
  `rows_adjustments`, `rows_references` y, para `extended`, las tablas que recuperan lo que la
  librería pierde. Si alguna columna exige algo que la sección `tables` no expresa, se anota
  como límite del formato y se decide en el plan si se extiende el formato o se calcula en la
  capa Python con una línea de justificación.
- `crates/oxedi835_py/python/oxedi835/edi_835_parser/` (paquete Python puro sobre el binding)
  y los métodos nativos de T28 en el binding o en Python, delegando en el core.
- `pyproject.toml`: extras `edi-835-parser`, `pandas`, `polars`; dependencias de desarrollo
  `edi-835-parser`, `duckdb`.
- README: sección "Coming from edi-835-parser" con los dos caminos y la tabla de métodos.

**Gate de verificación (salida del Stage 5b).**
- CI: `assert_frame_equal` entre `edi_835_parser.parse(...).to_dataframe()` y
  `oxedi835.edi_835_parser.parse(...).to_dataframe()` en los seis samples, leyendo tanto de
  ruta como de bytes y de archivo abierto (las tres entradas dan el mismo DataFrame); `count_claims`,
  `count_patients`, `sum_payments` iguales; `payer`/`payee` con los mismos campos.
- `extended=True` añade filas y columnas solo con prefijo `x_`, y un test enumera, por
  archivo, lo recuperado (claims sin servicios, ajustes de claim, `PLB`).
- Informe de `compat_oracle.py` sobre los originales sin diferencias fuera de las
  documentadas (el shim de `N104`).
- Métodos nativos de T28 iguales a sus equivalentes viejos en los seis samples (`Decimal`
  frente a `float` comparado con tolerancia de redondeo de céntimo).
- Test DuckDB; suites Rust y Python en verde; el paquete base sigue sin dependencias Python.

**Rust que exprimes.** Poco Rust: la prueba de N3 es dato. Lo que se aprende es de diseño:
expresar una salida ajena como proyección declarativa, tablas largas frente a anchas, y una
API de compatibilidad como vistas sin copia.

**Fuera de alcance.** Capas para otras librerías (el sondeo no lo justifica; `pyx12` va por
D16); escribir 835 (Stage 7); corregir `edi-835-parser` (se ofrece el parche de `N104` aguas
arriba, fuera del repo).

### Stage 6 · Distribución — APROBADO 2026-10-04

El stage que convierte el binding en un paquete instalable con `pip install oxedi835` en
Linux, macOS y Windows sin compilar nada, publicado sin secretos guardados y verificado en cada
plataforma antes de salir. Cierra con `0.1.0`, la primera versión usable.

**Decisiones de diseño (cada una con la alternativa descartada).**
- **T30 · Matriz de wheels `abi3` (Python ≥ 3.11), un wheel por plataforma.** Linux
  `manylinux_2_28` x86_64 y aarch64, Linux `musllinux_1_2` x86_64, macOS x86_64 y arm64 por
  separado, Windows x86_64; el sdist se construye solo en Linux. Descartados `universal2`
  (dobla el tamaño sin ganancia) y Windows ARM (sin demanda).
- **T31 · `PyO3/maturin-action`**, partiendo de lo que genera `maturin generate-ci github`;
  resuelve los contenedores manylinux/musllinux y la compilación cruzada a aarch64.
  Descartado un build a mano con `cibuildwheel` (más piezas para el mismo resultado con PyO3).
- **T32 · Publicación por *trusted publishing* (OIDC)**, sin tokens: el workflow publica con
  `pypa/gh-action-pypi-publish` desde los *environments* de GitHub `pypi` y `testpypi`, ambos
  con aprobación manual del dueño. Las versiones con sufijo `a`/`b`/`rc` van solo a TestPyPI; las
  finales, a PyPI. Cuando funcione, se revocan los tokens de `~/.pypirc` y de los secrets.
  Descartado publicar con token (secreto de larga vida en el repo).
- **T33 · Una sola fuente de versión y release por tag.** La versión vive en
  `[workspace.package] version` del `Cargo.toml` raíz; los crates la heredan y `pyproject.toml`
  la declara `dynamic` para que maturin la lea de Cargo. El workflow de release se dispara con
  un tag `v*` en `master` y falla si el tag no coincide con esa versión (traducida a PEP 440).
  Descartado mantener la versión en dos sitios.
- **T34 · crates.io diferido.** El crate se llama `edi835_core`, un nombre atado al 835 que
  chocaría con el kit de la familia X12 (D15); `0.1.0` sale solo en PyPI y crates.io se decide
  con el nombre neutro en el Stage 9 (reservando `oxedi835` allí cuando toque).
- **T35 · Verificación en cada plataforma antes de publicar.** Cada wheel se instala en su
  sistema, en Python 3.11 y 3.13, y pasa la suite pytest completa (no solo un import), con el
  mismo esquema que `scripts/smoke_wheel.sh`; el job de publicación depende de todos ellos. En
  Windows el checkout activa `core.symlinks` para que `LICENSE` y `THIRD_PARTY_NOTICES` sean
  archivos reales, y `make sdist-check` vale también para los wheels (licencias presentes y
  idénticas a la raíz).
- **T36 · Notas de versión.** `CHANGELOG.md` en formato "Keep a Changelog", escrito a mano en
  cada release; la release de GitHub lleva ese texto y los artefactos; versionado semántico
  `0.x`, en el que una minor puede romper la API (el README lo dice).

**Entregable / contrato.**
- `.github/workflows/release.yml` (build sdist, matriz de wheels, verificación por plataforma,
  publicación a TestPyPI o PyPI según el sufijo, release de GitHub) y el `ci.yml` actual sin
  cambios de comportamiento.
- Versión única en el workspace; `pyproject.toml` con `dynamic = ["version"]`; `CHANGELOG.md`.
- `Makefile`: objetivos `version` (imprime la versión) y `release-check` (comprueba tag,
  versión y changelog en local antes de etiquetar).
- Pasos del dueño (no automatizables desde el repo): registrar el *trusted publisher* en PyPI y
  TestPyPI (proyecto `oxedi835`, repo `javillegasna/oxedi835`, workflow `release.yml`,
  environments `pypi`/`testpypi`); el controlador crea los environments en GitHub con la
  aprobación manual.

**Gate de verificación (salida del Stage 6).**
- `v0.1.0rc1` sale por el workflow a TestPyPI con todos los wheels verificados en su sistema;
  `pip install --pre oxedi835` desde TestPyPI funciona en Linux, macOS y Windows (lo comprueba el
  propio workflow).
- `v0.1.0` sale a PyPI por el mismo camino tras la aprobación manual; la release de GitHub
  existe con el changelog; los tokens antiguos están revocados.
- Gates del repo (`make gates`, `make py-test`, `make dist`) en verde.

**Fuera de alcance.** crates.io (T34); Windows ARM y `universal2` (T30); documentación de
usuario perdurable (Stage 8); firma de artefactos con Sigstore más allá de la atestación que
`gh-action-pypi-publish` añade por defecto.

### Stage 5c · Estructura de módulos — APROBADO 2026-10-04

El stage que hace el código navegable sin cambiar lo que hace: los dos archivos que esconden
su código bajo varias responsabilidades se parten en submódulos cortos, los tests unitarios
salen del archivo que prueban a un archivo hermano, y cada módulo dice en su cabecera qué
contiene. Resuelve D13. Medido el 2026-10-04: `spec.rs` tiene 4.981 líneas (2.334 de código y
2.647 de tests) y `project.rs` 1.509 (943 de código); el resto de los archivos grandes lo son
por sus tests (`column.rs`: 199 líneas de código y 1.361 de tests).

**Decisiones de diseño (cada una con la alternativa descartada).**
- **T37 · Se parten solo `spec.rs` y `project.rs`.** `spec.rs` pasa a `spec/` con un archivo
  por responsabilidad (ids y loops con sus controles; segmentos y elementos; tablas y su
  compilación; `SpecError`; estructuras de deserialización; chequeo de forma; render de claves;
  merge patch); `project.rs` pasa a `project/` por responsabilidad. La partición exacta la fija
  el plan. Descartado partir todo archivo que supere un tamaño: movería archivos cuyo código
  ya cabe en una pantalla, sin ganar legibilidad.
- **T38 · Tests unitarios en un archivo hermano.** Cada módulo con tests declara
  `#[cfg(test)] mod tests;` y sus tests viven en `tests.rs` dentro de la carpeta del módulo,
  con acceso a lo privado como hoy. Vale para todos los módulos del core, también los que no se
  parten. Descartados dejarlos al final del archivo (archivos de 1.500 líneas con 200 de código)
  y moverlos a `tests/` como integración (perderían lo privado y forzarían a publicarlo).
- **T39 · Rutas públicas intactas.** Los submódulos nuevos son privados y el `mod.rs` de cada
  carpeta reexporta exactamente lo que hoy exporta el archivo; ninguna ruta `edi835_core::…`
  cambia, ni la que usa el binding. Un test de integración nombra cada ruta pública actual, de
  modo que falla al compilar si una desaparece. Descartado rediseñar la API en el mismo stage.
- **T40 · Guía de tamaño, no gate.** Unas 400 líneas de código por archivo, sin contar tests,
  escrita en `CLAUDE.md`. Descartado un chequeo en CI: forzaría cortes artificiales cuando una
  pieza coherente pasa del límite.
- **T41 · Descubrimiento en el código.** `lib.rs` sigue siendo el índice comentado; cada
  `mod.rs` nuevo abre con un `//!` corto que dice qué contiene cada uno de sus archivos.
  Descartado un mapa aparte: la documentación de arquitectura para humanos es el Stage 8 y un
  mapa duplicado se desactualiza.
  Un módulo es una carpeta (decidido 2026-10-04, tras partir `spec`): todo módulo del core es
  `x/mod.rs` con sus tests en `x/tests.rs` o `x/tests/`, y el lint de clippy
  `self_named_module_files` impide volver a la forma `x.rs` junto a `x/`. Descartada la forma
  `x.rs` + `x/` que recomienda la guía de Rust: los exploradores listan las carpetas antes que los
  archivos y separan el módulo de su carpeta.
- **T42 · Sin cambio de comportamiento, demostrado.** El mismo número de tests antes y
  después, los goldens intactos sin `UPDATE_GOLDEN`, rustdoc limpio y los benchmarks dentro del
  ruido. Un commit por archivo partido, para revisar por partes; `git blame` del contenido
  movido queda en el commit de la partición (se acepta).
- **T43 · La capa Python no se toca.** `_frame.py` (602 líneas) refleja la estructura de la
  librería que imita y su tamaño es razonable; tampoco el binding de Rust (≤ 266 líneas por
  archivo). Descartado partirla en el mismo stage: diff mayor sin beneficio claro.
- **T44 · Entran #64 y #71.** #64 (detalles de `frame.rs` y dos textos de error) cae en
  archivos que se tocan; #71 expone `oxedi835.__version__` derivado de la fuente única de
  versión, con su test. Son los únicos cambios visibles del stage.
- **T45 · Un archivo de benchmarks por capa** (aprobado con el plan, 2026-10-04).
  `benches/tokenize.rs` guarda los ocho grupos de criterion del pipeline; se parte en
  `tokenize.rs`, `engine.rs`, `check.rs` y `process.rs` con los cargadores en
  `benches/common/`. Los nombres de grupo y los ids no cambian, para que las líneas base
  guardadas sigan comparando. Descartado renombrar los grupos.

**Entregable / contrato.**
- `crates/edi835_core/src/spec/` y `crates/edi835_core/src/project/` con un `mod.rs` que
  reexporta la API actual; todos los módulos del core con sus tests en un `tests.rs` hermano.
- Test de integración de rutas públicas; guía de tamaño en `CLAUDE.md`.
- #64 y #71 cerrados por el PR.

**Gate de verificación (salida del Stage 5c).**
- `make gates` y `make py-test` en verde; el recuento de tests de cargo y pytest es el de
  `master` más los tests nuevos de T39 y T44, nunca menos.
- Ningún golden cambia; `cargo bench` sin regresiones fuera del ruido en tokenizer, engine y
  projector.
- Ningún archivo de código de `spec/` o `project/` supera la guía de T40 sin una razón escrita
  en el plan.

**Fuera de alcance.** Cambios de API o de comportamiento (salvo #71); la capa Python y el
binding (T43); rendimiento del proyector y memoria de los spans (#39 y #45, después de este
stage); documentación de arquitectura para humanos (Stage 8).

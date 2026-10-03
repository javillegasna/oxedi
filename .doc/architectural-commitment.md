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
| **7 · Escritor** *(YAGNI, D8)* | Builder: datos → árbol de loops → segmentos → bytes, dirigido por la misma spec (P6); calcula campos derivados (SE01, GE, IEA, ISA ancho fijo) | Escribir y volver a parsear == original; SNIP sobre lo generado | builder pattern, `fmt::Write` / `io::Write`, `Display`, formato de ancho fijo |

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
  `Arc` (`Arc::from(Vec)` vuelve a copiar el buffer). Iterar todos los segmentos: 4,2 frente
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
  en uso y se vea si una regla "suma por grupo" basta.
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
- **D13 · Estructura de módulos**: `spec.rs` supera las 2.500 líneas tras el Stage 4a y
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
- **D7 · Stage 7, Escritor**: ver §5. YAGNI hasta que haya un caso de generación. La mitad
  del trabajo ya la paga el round-trip de Stage 1 (serializar segmentos con escape) y la
  otra mitad la paga T4. Lo propio del escritor: campos derivados y builder desde dominio.

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
diagnósticos son valores, nunca excepciones (P7); el único error que se lanza es el de una
spec inválida, con el texto de su `Display` (P10).

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
- `crates/oxedi835_py`: `Cargo.toml` (`pyo3` con `abi3-py39` y `extension-module`,
  `arrow` solo con las features de `ffi`/`pyarrow`-free que hagan falta), `pyproject.toml`
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

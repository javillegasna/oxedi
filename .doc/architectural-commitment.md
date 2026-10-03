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
- **D8 · `Cow` + spans frente a `Arc<[u8]>` + spans**: en el stage siguiente al que
  tenga un consumidor que comparta el documento (previsiblemente Stage 5, Python), medir
  las dos representaciones en tiempo y en memoria sobre las mismas fixtures y los samples
  grandes: construir, iterar, `into_owned` o clonar, y retener N documentos a la vez. Se
  decide con números, no antes. El cambio es local porque los spans no cambian.
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


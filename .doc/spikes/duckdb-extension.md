# Spike: extensión de DuckDB para oxedi835

Fecha: 2026-10-04. DuckDB estable: v1.5.6 (Variegata, `069cc9f9b5`). Rama del prototipo:
`spike-duckdb-prototype` (directorio `spike/duckdb/`, fuera de los miembros del workspace; no toca
`crates/`). Toolchain: rustc 1.99.0, `duckdb-rs` 1.10506.0, `extension-ci-tools` `3fd6109`,
`extension-template-rs` `8ec303e`, `extension-template-c` `f060b12`, CMake 4.4.4 y gcc del sistema.

## Veredicto

**Viable, y más fácil de lo que suponía D17.** Las dos vías construyen, cargan y devuelven las
mismas tablas que `oxedi835.parse_file` en los seis samples (30 de 30 tablas idénticas, filas y
md5 de cada fila). El dato que cambia el planteamiento: **en DuckDB v1.5.6 la parte inestable de
la API C de extensiones está vacía**. Las 546 entradas del struct `duckdb_ext_api_v1` son estables
(404 de la banda v1.2.0 y 142 estabilizadas en v1.5.6), así que `duckdb-rs` ya no obliga a fijar
la extensión a una versión exacta de DuckDB. La plantilla de Rust compilada con
`USE_UNSTABLE_C_API=0` produce un binario con ABI `C_STRUCT` que carga sin recompilar en DuckDB
2.0.0-dev. Además, todo lo que pide la escritura (funciones de copia, leer otras tablas) está en
el subconjunto estable.

## Ruta recomendada

**Extensión en Rust sobre `duckdb-rs`, con la API C estable (ABI `C_STRUCT`, objetivo v1.5.6).**
Es la plantilla oficial con un solo cambio: `USE_UNSTABLE_C_API=0`. Razones:

1. **Funciona y es compatible hacia delante.** El mismo binario carga en v1.5.6 y en
   2.0.0.dev2610011535 con resultados idénticos. El de ABI inestable falla en 2.0 con
   `The file was built specifically for DuckDB version 'v1.5.6'`.
2. **Todo el código es Rust.** No suma C ni CMake al repositorio. El `unsafe` queda en pocos
   puntos (escribir en los buffers de los vectores, la función de copia) y `duckdb-rs`, que
   mantiene DuckDB, cubre el resto. Las reglas del proyecto (sin `unwrap`, P10) se aplican con las
   mismas herramientas que en el core.
3. **Tiene precedente.** Unas 60 extensiones comunitarias en Rust se publican por este camino
   (`rusty_sheet`, `qvd`).
4. **La escritura cabe.** `COPY ... (FORMAT x)` necesita la banda v1.5.6 en cualquier vía, así que
   apuntar a v1.5.6 no recorta nada de lo que pide D7.

Dos condiciones llevarían a la **vía C** (extensión fina en C con una staticlib de Rust, también
probada aquí):

- que sea obligatorio que la misma extensión cargue en la línea 1.4 LTS (`andium`), cuyo struct no
  tiene la banda v1.5.6;
- que `duckdb-rs` empiece a usar funciones de la cola inestable cuando DuckDB vuelva a tenerla.

La vía C tiene una ventaja más: su capa `extern "C"` sobre el core es el germen de la API C de D17
(a). Pero D17 puede decidir esa API por su cuenta; DuckDB no la exige.

## Respuestas

### 1. Cobertura de la API C estable

**Evidencia.**

- El encabezado `duckdb_extension.h` de `libduckdb-linux-amd64.zip` v1.5.6 (idéntico al que
  empaqueta `libduckdb-sys` 1.10506.0) tiene una sola guarda de versión dentro del struct:
  `#if DUCKDB_API_VERSION_AT_LEAST(1, 5, 6)` (líneas 552–728). No tiene bloque
  `DUCKDB_API_ALLOW_UNSTABLE`.
- En `api_spec/VERSIONING.md` de DuckDB v1.5.6, la estructura documentada es: «band v1.2.0 — 404
  slots, always present», «band v1.5.6 — 142 slots», y la cola inestable «always at the end». El
  mismo documento dice: «nothing was stabilized between v1.2.0 and v1.5.6».
- El struct que genera `duckdb-rs` (bindgen con `DUCKDB_EXTENSION_API_VERSION_UNSTABLE`) tiene 546
  campos. La comprobación fue `awk` sobre `bindgen_bundled_version_loadable.rs`, y cada uno de los
  546 tiene como último estado `stable` (498) o `deprecated` (48).
- El historial de cada función sale del `history:` de su doc comment. La tabla completa está en
  `spike/duckdb/evidence/capi_lifecycle_v1.5.6.txt`.

**Clasificación.** Todas las funciones de esta lista son estables. La columna «desde» dice desde
qué versión.

| Necesidad | Funciones | Estado |
|---|---|---|
| Registrar función de tabla, bind con parámetros posicionales y con nombre, columnas de resultado, init, scan | `duckdb_create_table_function`, `_add_parameter`, `_add_named_parameter` (v0.8.0), `_set_bind/_init/_function`, `duckdb_register_table_function`, `duckdb_bind_add_result_column`, `duckdb_bind_get_parameter`, `duckdb_bind_get_named_parameter`, `duckdb_bind_set_bind_data`, `duckdb_bind_set_cardinality`, `duckdb_bind_set_error`, `duckdb_init_set_init_data`, `duckdb_init_set_max_threads`, `duckdb_init_get_column_index` (proyección), `duckdb_function_get_bind_data/_init_data`, `duckdb_function_set_error` | estable, banda v1.2.0 (vienen de v0.3.3–v0.8.0) |
| Llenar `duckdb_data_chunk` | `duckdb_data_chunk_get_vector`, `_set_size`, `duckdb_vector_get_data`, `duckdb_vector_size` | estable, v1.2.0 |
| VARCHAR / BLOB | `duckdb_vector_assign_string_element_len` (sirve para BLOB) | estable, v1.2.0; la variante `duckdb_unsafe_vector_assign_string_element_len` es estable desde v1.5.6 |
| BIGINT, DATE, TIME | `duckdb_create_logical_type` + escritura directa en `duckdb_vector_get_data` (`int64_t`, `duckdb_date` = días `int32`, `duckdb_time` = µs `int64`) | estable, v1.2.0 |
| DECIMAL(p,s) | `duckdb_create_decimal_type`; el almacenamiento físico depende del ancho (`int16`/`int32`/`int64`/`duckdb_hugeint` si p > 18; el nuestro es p = 38) | estable, v1.2.0 |
| Máscara de validez | `duckdb_vector_ensure_validity_writable`, `duckdb_vector_get_validity`, `duckdb_validity_set_row_invalid` | estable, v1.2.0 |
| Arrow como resultado de una función de tabla | `duckdb_schema_from_arrow`, `duckdb_data_chunk_from_arrow`, `duckdb_destroy_arrow_converted_schema` (y a la inversa `duckdb_to_arrow_schema`, `duckdb_data_chunk_to_arrow`) | estable desde v1.5.6 (inestable desde v1.4.0). `duckdb_arrow_scan` y `duckdb_arrow_array_scan` están **deprecated** desde v1.0.0 |
| Funciones de copia (`COPY ... TO ... (FORMAT x)`) | `duckdb_create_copy_function`, `_set_name`, `_set_bind`, `_bind_get_column_count/_type`, `_bind_get_options`, `_set_global_init`, `_global_init_get_file_path`, `_set_sink`, `_set_finalize`, `duckdb_register_copy_function`, y `_set_copy_from_function` para `COPY ... FROM` | estable desde v1.5.6 (inestable desde v1.5.0) |
| Ejecutar una consulta o leer otra tabla desde la extensión | `duckdb_connect` sobre la base que entrega `duckdb_extension_access.get_database`, `duckdb_query`, `duckdb_prepare`, `duckdb_fetch_chunk`; contexto: `duckdb_table_function_get_client_context`, `duckdb_client_context_get_catalog`, `duckdb_catalog_get_entry` | `duckdb_connect`/`duckdb_query` estables desde v0.1.0; el contexto y el catálogo, desde v1.5.6 |
| Sistema de archivos de DuckDB (rutas remotas o `httpfs`) | `duckdb_client_context_get_file_system`, `duckdb_file_system_open`, `duckdb_file_handle_read` | estable desde v1.5.6 |

**Solo inestable: nada.** En v1.5.6 no hay ninguna función solo inestable en el struct.

**Lo que falta en la API C, estable o no:**

- No hay parámetros de tipo `TABLE` ni funciones *table in-out* para funciones de tabla.
- No hay forma de ejecutar SQL en la conexión o transacción de quien llama:
  `duckdb_connection_get_client_context` va de conexión a contexto, no al revés.

### 2. Vía plantilla de Rust

**Prototipo.** Está en `spike/duckdb/rust_ext/`: la plantilla con `edi835_core` como dependencia
por ruta.

- `read_835(path, table_name := 'claims')` devuelve la tabla con sus columnas y tipos reales:
  Binary → BLOB, Int64 → BIGINT, Decimal128(38,s) → DECIMAL(38,s), Date32 → DATE,
  Time32 → TIME.
- El parámetro con nombre no puede llamarse `table`: `Parser Error: syntax error at or near "table"`.
- Los errores llegan como `Binder Error: read_835: no table "nope"; the spec projects [...]` y
  `Binder Error: read_835: /nonexistent: No such file or directory (os error 2)`.

**Comparación con el oráculo.** `spike/duckdb/scripts/compare.py` compara contra
`pa.table(oxedi835.parse_file(p).tables[t])` (binding reconstruido en release con
`maturin develop --release`).

- Mide `count(*)` y `md5(string_agg(t::VARCHAR, '|' ORDER BY t."row"))` sobre cada fila completa
  en los seis samples y las cinco tablas: `mismatches: 0` con ABI inestable y con ABI estable.
- `claims` de `edi835_test_united.rmt`: 1332 filas, md5 `ac6f194173ae…` en ambos lados.
- `services` (6192 filas) cruza varios lotes de 2048.
- Se cargó en el CLI (`duckdb -unsigned`, `LOAD '…/oxedi835.duckdb_extension'`) y en Python
  `duckdb` 1.5.6 (`allow_unsigned_extensions`).
- `make test_release` de ci-tools corre `test/sql/read_835.test` (SQLLogicTest): `SUCCESS`.

**Datos.**

| | Plantilla Rust (`duckdb-rs`) |
|---|---|
| Build release desde cero | 51,8 s de reloj (16 núcleos; compila `duckdb-rs` y el core con LTO); incremental 15,6 s |
| Tamaño del binario | 1 714 630 bytes (`rusty_quack` de la plantilla: 404 342) |
| Versión de DuckDB | `duckdb = "~1.10506.0"` → DuckDB v1.5.6; metadatos `duckdb_version = v1.5.6` |
| Funciones inestables | con `USE_UNSTABLE_C_API=1` (por defecto en la plantilla): ABI `C_STRUCT_UNSTABLE`, fijo a v1.5.6. Con `=0`: ABI `C_STRUCT`, ninguna inestable (el struct no tiene cola) |

**Matriz de carga** (`evidence/load_matrix.txt`).

| Binario | DuckDB 1.4.5 | DuckDB 1.5.6 | DuckDB 2.0.0.dev2610011535 |
|---|---|---|---|
| Rust, ABI inestable | falla: «built specifically for DuckDB version 'v1.5.6'» | carga | falla (mismo error) |
| Rust, ABI estable v1.5.6 | falla: «built for DuckDB C API version 'v1.5.6'» | carga | **carga**, 30/30 tablas idénticas |
| C + staticlib, ABI estable v1.2.0 | **carga**, 30/30 | carga, 30/30 | **carga**, 30/30 |

**Detalles de construcción.**

- La librería debe llamarse como la extensión (`[lib] name = "oxedi835"`). Si no, el Makefile no
  encuentra `liboxedi835.so`.
- El nombre del archivo `.duckdb_extension` debe coincidir con el símbolo `<nombre>_init_c_api`.
- Para registrar la función de copia escribí el entrypoint a mano (sin la macro): `duckdb-rs` no
  expone el `duckdb_connection` crudo ni envuelve las funciones de copia, pero `duckdb::ffi` sí las
  trae.

### 3. Vía C estable

**Sí, sin bloqueos.** El prototipo está en `spike/duckdb/c_ext/`:

- `extension-template-c` con los encabezados v1.5.6;
- `USE_UNSTABLE_C_API=0` y `TARGET_DUCKDB_VERSION=v1.2.0`;
- un `oxedi835_ext.c` de unas 200 líneas que registra `read_835` y copia filas;
- `ffi/`, una staticlib de Rust (`oxedi835_ffi`) que expone `oxedi835_parse`,
  `oxedi835_table_rows/_columns/_column_name/_column`, `oxedi835_table_free` y
  `oxedi835_string_free`.

`oxedi835_table_column` entrega los buffers del core con la disposición de Arrow (validez,
offsets, datos) sin copiar. El C los vuelca en los vectores de DuckDB.

| | C + staticlib de Rust |
|---|---|
| Build release desde cero | 10,5 s (`cargo build` del core 9,9 s + CMake) |
| Tamaño | 673 070 bytes |
| ABI | `C_STRUCT`, `duckdb_version = v1.2.0`; carga en 1.4.5, 1.5.6 y 2.0-dev |
| Símbolos `duckdb_*` sin resolver | 0 (`nm -D --undefined-only`): todo pasa por el struct |
| Comparación y SQLLogicTest | `mismatches: 0`; `make test_release`: `SUCCESS` |

No falta nada de la API estable. El coste está en otra parte:

- un segundo lenguaje con su propio `unsafe` y su gestión de memoria (`duckdb_malloc`,
  destructores, cadenas de error que cruzan la frontera);
- CMake en la cadena;
- la API C del core a mantener como contrato.

Para la escritura (funciones de copia) habría que subir el objetivo a v1.5.6, y entonces esta vía
también deja de cargar en 1.4.

### 4. Escritura

**Función de tabla que recibe nombres de tabla: factible, con límites claros.**
`oxedi835_rows_of(name)` abre en el bind una segunda conexión a la misma base (`try_clone`, es
decir `duckdb_connect`), ejecuta `SELECT * FROM "name"` y recorre todas las filas. No se bloquea.
Resultados:

| Caso | Resultado |
|---|---|
| Tabla persistente (`CREATE TABLE claims AS SELECT * FROM read_835(...)`) | 1332 filas |
| Tabla `TEMP` | `Catalog Error: Table with name tmp_claims does not exist!` |
| Objeto registrado desde Python (`con.register('x', arrow)`) y vista sobre él | mismo error |
| Filas sin confirmar de la transacción de quien llama | invisibles: `BEGIN; INSERT; rows_of` → 1; tras `COMMIT` → 2 |

La segunda conexión ve solo lo que está confirmado en el catálogo compartido. La API C no permite
ejecutar en la conexión de quien llama ni recibir parámetros `TABLE`.

**Formato de `COPY`: factible y sin esos límites.** `COPY (...) TO 'f' (FORMAT oxedi835_probe)`
registra bind, global init (ruta del archivo), sink (un `duckdb_data_chunk` por lote) y finalize
con las funciones estables de v1.5.6. Escribió:

- `rows=6192 columns=13` para `services`;
- `rows=1332 columns=24` desde una tabla `TEMP`;
- `rows=3` desde un Arrow registrado en Python;
- lo mismo en DuckDB 2.0-dev.

La consulta corre en el contexto de quien llama, así que ve tablas temporales, objetos registrados
y la transacción en curso. Su límite es el de D17: una sola consulta, es decir un esquema
aplanado.

**Consecuencia para el diseño.** El `COPY` es la vía robusta. Una opción para tener varias tablas
sin aplanar: `COPY` con un formato que acepte una consulta que una las tablas (por ejemplo,
`payments` con listas o structs anidados de `claims` y `services`) y que el escritor reconstruya
la jerarquía con los ordinales de padre. Otra opción es la función por nombres, limitada a tablas
persistentes y confirmadas. Las dos se deciden en el §7 del escritor.

### 5. Rendimiento

Mediana de 50 ejecuciones en Python `duckdb` 1.5.6, binding en release, `edi835_test_united.rmt`
(629 300 bytes). Datos en `evidence/bench_*.txt`.

| Camino | ms |
|---|---|
| `read_835` (Rust) → `count`/`sum` | 16,7 |
| `read_835` (Rust) → tabla Arrow | 17,7 |
| `read_835` (C) → `count`/`sum` | 16,3 |
| `parse_file` solo | 15,8 |
| `parse_file` + `from_arrow` → `count`/`sum` | 18,8 |
| `parse_file` + `from_arrow` → tabla Arrow | 19,8 |

**Mismo orden de magnitud.** El parseo domina (unos 16 ms). La extensión ahorra unos 2 ms porque
no cruza a Python ni construye Arrow.

- CLI: `Run Time (s): real 0.017` por consulta.
- `COPY (SELECT * FROM read_835(..., table_name := 'services')) TO 'svc.parquet'`: 0,024 s.

El prototipo no usa *projection pushdown* (siempre llena todas las columnas) ni paraleliza varios
archivos; las dos son mejoras posibles, no necesarias.

### 6. Repositorio comunitario

Fuente: `duckdb/community-extensions` en `69d1ff0` (2026-10-04), 359 descriptores.

- **`description.yml`.**
  - Bloque `extension`: `name`, `description`, `version`, `language`, `build`, `license` y
    `maintainers` (los usan 352–359 de 359).
  - Bloque `repo`: `github` y `ref` (commit). Son opcionales `ref_next` (commit para la próxima
    versión de DuckDB) y `andium` (ref para la línea 1.4).
  - Bloque `docs`: `hello_world` y `extended_description`.
  - Opcionales de `extension`: `excluded_platforms`, `opt_in_platforms`, `requires_toolchains`,
    `vcpkg_commit`, `test_config` y `canonical_name`.
  - Ejemplo de Rust: `rusty_sheet` (`language: Rust`, `build: cargo`,
    `requires_toolchains: "rust;python3"`).
- **Toolchain.** `build:` es una etiqueta. `qvd` lo comenta: «community label; the actual build
  goes through the Rust C-API Makefile». El CI ejecuta `make configure_ci`, `make release` y
  `make test_release` del repositorio de la extensión con `extension-ci-tools`. `rust` en
  `requires_toolchains` activa `dtolnay/rust-toolchain@stable`. Para la vía C con staticlib basta
  `requires_toolchains: "rust;python3"`; ya hay extensiones `language: C` con `build: cmake`
  (`read_stat`, `healthkit_export`).
- **Licencias.** 285 de 359 son MIT, 45 Apache-2.0 y 10 incluso BSL 1.1, así que el repositorio no
  exige una licencia concreta. Ni la guía de desarrollo ni el anuncio de 2024 imponen una. El
  prototipo en Rust arrastra 105 crates en el árbol normal, todas permisivas (MIT, Apache-2.0,
  BSD, ISC, Zlib, Unicode-3.0, CDLA-Permissive-2.0). Las tablas MIT de `THIRD_PARTY_NOTICES`
  (`edi-835-parser`) viven solo en el paquete Python (`_codes.py`), no en el core, así que la
  extensión no las incluye. Si algún día se llevan al core o a la extensión, el aviso MIT viaja
  con el binario.
- **Plataformas.**
  - Matriz por defecto: `linux_amd64`, `linux_arm64`, `linux_amd64_musl`, `linux_arm64_musl`,
    `osx_amd64`, `osx_arm64`, `windows_amd64`, `windows_arm64`, `windows_amd64_mingw` y
    `wasm_mvp/eh/threads`.
  - Las extensiones en Rust excluyen
    `wasm_mvp;wasm_eh;wasm_threads;windows_amd64_rtools;windows_amd64_mingw;linux_amd64_musl`
    (`rusty_sheet` y `qvd`). Habría que probar si nuestro core compila para musl y WASM: no usa
    I/O ni hilos. La lectura del archivo sí debe pasar por el sistema de archivos de DuckDB para
    funcionar en WASM.
- **Actualizaciones por versión.**
  - Cambiar `ref` en el descriptor publica la nueva versión contra la DuckDB estable vigente
    (`DUCKDB_LATEST_STABLE: 'v1.5.6'` en `build.yml`).
  - En cada versión de DuckDB el CI recompila todas las extensiones. En las de API C,
    `set_duckdb_version` es `nop` (`base.Makefile`): con ABI estable el código no cambia, y solo
    hace falta `ref_next` si deja de compilar.
  - Los usuarios no pueden instalar versiones antiguas (UPDATING.md: «does not currently provide a
    way for users to install an older community extension source version»).
  - La firma la pone el CI comunitario. Sin él hace falta `allow_unsigned_extensions` o
    `-unsigned`.

## Riesgos

1. **`duckdb-rs` y la cola inestable futura.** `libduckdb-sys` genera el struct con
   `DUCKDB_EXTENSION_API_VERSION_UNSTABLE`. Hoy la cola está vacía, pero cuando DuckDB añada
   funciones inestables, una versión nueva de `duckdb-rs` las pondrá al final del struct. Con ABI
   `C_STRUCT`, cualquier llamada de `duckdb-rs` a esas posiciones iría al puntero equivocado en una
   DuckDB más nueva. Mitigación:
   - fijar `duckdb-rs` en `Cargo.lock`;
   - subir de versión solo tras comprobar en su encabezado que no hay cola inestable o que no la
     usamos;
   - mantener un test con `next` en el CI (el comunitario ya compila `build_next`).
2. **El ABI estable de `duckdb-rs` no es el camino documentado de la plantilla.** La plantilla
   dice que requiere `USE_UNSTABLE_C_API=1`. Esa nota quedó atrás en v1.5.6, pero nadie lo
   garantiza. Si DuckDB lo vuelve a exigir, se cae a la vía C o a Rust sin `duckdb-rs` contra el
   encabezado estable.
3. **Sin línea 1.4 con la vía Rust.** El binario con objetivo v1.5.6 no carga en 1.4.5. Soportar
   `andium` pediría otra ref compilada con ABI inestable, o la vía C con objetivo v1.2.0 (solo
   lectura).
4. **Lectura por nombre de tabla.** Una segunda conexión no ve tablas `TEMP`, objetos registrados
   desde clientes ni datos sin confirmar.
5. **Estado global.** El prototipo guarda la conexión en un `OnceLock` de proceso: dos bases
   abiertas en el mismo proceso compartirían la conexión de la primera. En producción, la
   conexión o la base van en el `extra_info` de cada función.
6. **E/S con `std::fs`.** No sirve para `s3://`, `https://` ni WASM. Hay que usar el sistema de
   archivos de DuckDB (estable desde v1.5.6).
7. **Plataformas.** Sin musl ni WASM al principio, como las demás extensiones en Rust.
8. **Memoria.** `read_835` parsea el archivo entero en el bind y conserva `Tables` hasta el final
   del scan. Encaja con el modelo del core (un archivo en memoria), pero hace falta cuidado con
   globs de muchos archivos grandes.
9. **Mapeo de tipos.** El `Int64` con escala implícita sale como BIGINT y pierde la escala; en
   Arrow va en los metadatos del campo. Hay que decidir si pasa a DECIMAL o se documenta.

## Esquema propuesto para un §7 de la etapa de extensión

**Entrega.**

- Crate `crates/oxedi835_duckdb` (Rust, `duckdb-rs`, ABI `C_STRUCT` v1.5.6) fuera del core: el
  core sigue sans-IO y sin dependencias nuevas.
- `read_835(path | lista | glob, table_name := 'claims')` sobre las cinco tablas de la spec.
- Una función o tabla de diagnósticos con la forma de `Diagnostic` (regla, ubicación, dato).
- Lectura de archivos por el sistema de archivos de DuckDB.
- Columnas `filename` opcionales.
- Tests SQLLogicTest sobre los samples.
- Descriptor listo para `community-extensions`.

**Decisiones abiertas para la conversación previa.**

- Una función con parámetro o una por tabla.
- Tipo de las columnas Binary: BLOB o VARCHAR validado.
- Escala de los `Int64`.
- Soporte de 1.4 LTS (decide la vía).
- Paralelismo por archivo.
- *Projection pushdown*.

**Puertas.**

- Igualdad fila a fila (md5 por tabla) con `oxedi835.parse_file` como oráculo en los seis samples
  y en los fixtures.
- Un test por mensaje de error (P10) en SQLLogicTest.
- Carga verificada en la estable vigente y en `next` (o el dev de PyPI).
- Job de CI en Linux con `make release` y `make test_release`.
- Sin `unwrap`/`panic` en el crate.
- `clippy -D warnings` y `fmt`.

**Fuera.**

- `write_835` y el formato de `COPY` (llegan con el escritor de D7; aquí solo queda probada la
  plomería).
- WASM y musl.
- Builds propios sin firmar.
- La API C del core para otros lenguajes (D17 a).
- Funciones *table in-out*.

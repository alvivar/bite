# Code review — BITE (v0.3.1)

Actualizado sobre `0f9f278`. Las referencias `archivo:línea` corresponden a ese
commit. Lo pendiente está ordenado por prioridad; lo terminado está al final, en
[Completado](#completado).

Los puntos marcados con **[verificado]** se reprodujeron en la revisión original
(código de `ac405f5`) con un cliente Python contra el binario. No se han vuelto
a reproducir desde entonces, salvo donde se indica.

**Resumen:** los tres P0 originales (framing TCP, escritura no bloqueante y
persistencia) están resueltos, con tests de regresión, y el Dockerfile ya
construía antes de la revisión. No queda pendiente ninguno de los tres P0 de
esta revisión. Lo pendiente es:

- **P1:** dos contratos de `Commands.md` que la implementación no cumple. Hay
  que decidir el comportamiento antes de escribir código.
- **P2:** fallos que se enmascaran o matan partes del proceso.
- **P3/P4:** simplificaciones e higiene. Conviene hacerlas junto con cambios que
  ya toquen esos archivos, no como una refactorización aparte.

---

## P1 — La implementación no coincide con `Commands.md`

Ambos puntos requieren una decisión antes de implementar. BITE almacena bytes,
no texto, así que ni el protocolo ni la documentación resuelven por sí solos
cuál debe ser el comportamiento. Hay que acordar la compatibilidad con clientes
existentes y qué hacer con valores no UTF-8 o que no son números.

### 4. `j`/`js` devuelven arrays de bytes, no strings **[verificado]**

`src/data.rs` `kv_to_json` (líneas 275–286, `json!(v)` en la 282)

`kv_to_json` hace `json!(v)` con `v: &Vec<u8>`, que serde serializa como una
secuencia de números:

    s data.name BITE  →  OK
    j data            →  {"name":[66,73,84,69]}      (Commands.md promete {"name":"BITE"})

En cambio `#j` (`src/subs.rs:98-102`) usa `String::from_utf8_lossy` y devuelve
strings. Las dos rutas no coinciden entre sí, ni con la documentación. Es un
resto de la migración "BITE is full bytes now" (8/2022).

**Opciones a decidir:**
- Convertir con `from_utf8_lossy`, igual que `#j`. Los bytes no UTF-8 se
  reemplazan.
- Documentar que los valores JSON son arrays de bytes y ajustar `#j`.

En ambos casos cambia la respuesta que reciben los clientes actuales de una de
las dos rutas.

### 5. `+1` almacena y devuelve 8 bytes binarios **[verificado]**

`src/data.rs` `Action::Inc` (líneas 105–135), `vec_to_u64` (302–314),
`u64_to_vec` (316–318)

    +1 num  →  b'\x00\x00\x00\x00\x00\x00\x00\x01'
    g num   →  b'\x00\x00\x00\x00\x00\x00\x00\x01'

Commands.md dice `+1 numberkey → 10`. Además:
- La heurística de `vec_to_u64` es inexacta, como reconoce su propio
  comentario: el texto `"12345678"` se interpreta como un u64 binario.
- Un valor que no es un número se trata en silencio como 0 (`unwrap_or(0)`).
- `+ 1` sobre `u64::MAX` no está controlado.

**A decidir:**
- ¿Número como texto (coherente con `s numberkey 9`) o binario documentado?
- ¿Qué pasa con los valores de 8 bytes que ya estén guardados en `db.bin`?
- ¿Error o 0 para valores inválidos?
- ¿Qué hacer con el desbordamiento?

---

## P2 — Fallos que se enmascaran o matan el proceso

### 7. Panics en hilos sueltos: falta una política general

`src/main.rs:119-133`

Ocho hilos con `.unwrap()` en `send`/`recv`/`poller.modify`. Si uno hace panic,
el proceso sigue vivo pero cojo (por ejemplo, sin parser acepta conexiones y
nunca responde).

Resuelto en parte:
- El caso concreto de la revisión original (`socket.shutdown(...).unwrap()` en
  heartbeat) ya no existe (`e3a7cbb`).
- Un error de persistencia ahora termina el proceso con un mensaje (`ccc2edc`).

**Pendiente:** decidir qué hacer ante un panic en el resto de hilos. Opciones
sin aprobar:
- `panic::set_hook` con `process::abort()`, para que Docker reinicie
  (`restart: unless-stopped`).
- Límites de error locales, como el del hilo de DB.

### 8. `server.accept()?` en el loop principal

`src/main.rs:146`

Un error transitorio de `accept` (`ECONNABORTED`, `EMFILE`, `WouldBlock` por un
despertar espurio) tumba el servidor. El riesgo es concreto y justifica un
`match` que registre el error y continúe.

### 9. `_ => unreachable!()` sobre eventos del SO

`src/main.rs:199`

Es una afirmación sobre input externo. Si `polling` entrega un evento sin
`readable` ni `writable` (HUP/error en algunas plataformas), muere el hilo
principal.

### 10. IDs y tamaños truncados en la cabecera

`src/main.rs:136,151-158`, `src/message.rs` `get_header`/`stamp_header` (68–85)

- **IDs:** `id_count` es un `usize` sin límite y el protocolo usa 2 bytes. Con
  más de 65535 conexiones simultáneas, `stamp_header` trunca el id en silencio.
  El cliente no pasa entonces la validación `message.from != id`
  (`src/reader.rs:101`) y recibe un error engañoso. Bastaría con un
  `assert!(client_id <= u16::MAX)` o con rechazar la conexión.
- **Tamaño (observación, sin reproducir):** `stamp_header` también trunca en
  silencio `size` si una respuesta supera 65529 bytes. Puede pasar con las
  respuestas agregadas de `k`/`j`/`js` y rompería el framing del cliente.

### 11. Fugas de memoria en `subs.rs`

`src/subs.rs:65-68`

`Del` usa `entry(key).or_default()`, así que cada `#- claveinexistente` crea un
`Vec` vacío que nunca se elimina. Conviene usar `get_mut`. Además, `id_keys`
tampoco se limpia en `Del`, solo en `DelAll`.

### 12. `SetList`: separador multibyte y clave vacía

`src/data.rs:84-103` (separador en la línea 85)

- `key.chars().next().unwrap() as u8` trunca en silencio los caracteres
  multibyte, aunque la documentación dice "any byte".
- Un separador al final (`sl | a 1|`) inserta la clave `""` en el mapa.

### 13. Búsqueda de hijos por prefijo de string, no de path

`src/data.rs:172-176` (`k`), `207-211` (`jtrim`), `239-243` (`j`/`js`)

`range(key..).take_while(starts_with(key))` hace que `k user` también devuelva
`username`. Si la intención es "hijos", el filtro debería ser
`k == key || k.starts_with(&format!("{key}."))`.

### 14. `SetIfNone` responde antes de saber el resultado

`src/parser.rs:146-157`, `src/data.rs:63-78`

Responde `OK` **antes** de saber si la clave existía, así que el cliente no
puede distinguir "se escribió" de "ya existía". Si es el comportamiento
deseado, hay que documentarlo; si no, la respuesta debe salir desde `data.rs`.

### 15. Observaciones de ciclo de vida (sin reproducir)

- Las acciones que ya estaban en cola (`Read`/`Write`/`Queue`) pueden aplicarse
  a un id recién reutilizado. Las comprobaciones de `closed` en reader y
  heartbeat evitan un `Drop` duplicado desde esos dos hilos, pero el Writer
  puede enviar un segundo `Drop` sobre un id ya reutilizado.
- La cola de envío (`SendQueue`, `src/connection.rs`) no tiene límite: un
  cliente que no lee acumula memoria sin tope.

Son anteriores a los cambios P0 y no se han verificado. Solo merecen trabajo
cuando haya un caso concreto.

---

## P3 — Simplificaciones

Solo cuando se toque el archivo con una necesidad concreta; nada de esto añade
abstracción, la quita.

- **`src/parser.rs`:** el bloque
  `writer_tx.send(Queue(Order { from_id, to_id: from_id, msg_id, data: OK.into() })).unwrap()`
  aparece 10 veces (8 con `OK`, 2 con `NO`). Un closure local
  `let reply = |data: &str| ...` lo reduciría y haría visible qué comandos
  responden `OK`, `NO` o nada.
- **`src/data.rs:172, 207, 239`:** el mismo `range + take_while + collect` tres
  veces (`todo.txt` ya lo anota). Conviene resolverlo junto con #13.
- **`src/subs.rs:22,104`:** `Sub.command: Command` obliga a un
  `_ => unreachable!()`. Un `enum SubKind { Get, KeyValue, Json }` pondría el
  contrato en el tipo.
- **`src/parser.rs:352-353`:** `next_line` con `#[allow(dead_code)]`, se puede
  borrar. **`impl Display for Command`** (`src/parser.rs:54-58`) no se usa,
  también se puede borrar.
- **`src/parser.rs:81-89`:** `text` se asigna dos veces y compara `utf8.len()`
  (bytes) con un límite en caracteres.
- **`src/connection.rs:70`:** `vec![0; BUFFER_SIZE]` se asigna en cada
  iteración de `read`; `chunk` se puede declarar fuera del loop.
- **`src/data.rs:188-194`:** el `if let Some(last) ... pop()` para quitar el
  `\0` final se puede reemplazar construyendo el mensaje con `join`.
- **`src/main.rs:39-41`:** `#[macro_use] extern crate log; extern crate pretty_env_logger;`
  sobra en edition 2021.
- **Dos `Connection` por cliente** (`src/main.rs:165-181`): el reader usa
  `messages`/`last_read` y el writer usa `send_queue`/`last_write`; el resto de
  campos sobra en cada lado. Separarlos evita contención entre los hilos, que
  es un motivo válido, pero conviene hacerlo explícito (con un comentario o con
  dos structs).
- **Clippy:** `.split('.').last()` debería ser `.next_back()`
  (`src/data.rs:181`, `src/subs.rs:89,99`). Son los 3 únicos warnings actuales.

---

## P4 — Higiene del repo y documentación

- `src/concat.py` y `src/concat.txt` (una copia del código) siguen en `src/` y
  trackeados. Conviene mover el script fuera de `src` e ignorar `concat.txt`.
- La documentación dice JSON, pero el formato es bincode en `data/db.bin`:
  - README:70: "serialized into a json file". Además, el TODO de README:80
    ("serialized correctly instead of json") está desactualizado.
  - Commands.md:106: "data/DB.json".
  - Además, Commands.md:57 usa `b data.author` para el comando `k`.
- `docker-compose.yml` y `.docker/BITE-with-a-WebSocket-Proxy/docker-compose.yml`
  construyen desde GitHub (`build: https://github.com/alvivar/bite.git`), así
  que `docker-compose up --build` no usa el código local. Tampoco hay
  `.dockerignore`. Ninguna de las dos cosas impide construir; cambiarlas no
  está decidido.
- **Tests:** solo se justifican ante un riesgo concreto. Al resolver #4, #5,
  #11, #12 o #13, conviene añadir un test del comportamiento elegido. No hace
  falta un proyecto de cobertura general.
- `docs/persistence-change.html` explica solo #3. El usuario pidió aplazar su
  ampliación a todos los P0.

---

## Orden sugerido de trabajo

1. Decidir los contratos de #4 y #5 (y documentar #14), después implementar.
2. #8, #9 y #10: cambios pequeños con riesgo concreto. Decidir la política de
   #7.
3. #11, #12 y #13, junto con la simplificación de búsqueda de hijos (P3).
4. P3/P4 cuando se toquen esos archivos.

---

## Lo que está bien

- La separación por hilos con canales tipados (`enum Action` por módulo) se lee
  de arriba abajo sin saltos.
- El protocolo binario es mínimo y está bien documentado en `Protocol.md`; la
  validación `from != id` es correcta.
- La reutilización de IDs vía `used_ids` y el cleaner centralizado limpian
  poller, mapas y suscripciones.
- El flag atómico `modified` con guardado periódico es simple y suficiente.
- `parser.rs` trabaja en `&[u8]` con cursor y lifetimes en vez de copiar
  strings.
- Framing, escritura y persistencia tienen tests de regresión deterministas
  (16 en total).

---

## Completado

### 2. Framing con fragmentación TCP — `e3a7cbb`

(`e3a7cbb117cb70030e37afccc791b0ab713137ad`)

**Problema (original, [verificado]):**
- `Messages::feed` cerraba la conexión si el buffer acumulado tenía menos de 6
  bytes o más de 65535. Así fallaban las cabeceras partidas entre segmentos y
  dos mensajes válidos de 40 KB leídos juntos.
- `pending_read` tenía la semántica invertida, así que heartbeat nunca cerraba
  un cliente que se quedaba callado a mitad de un mensaje.

**Solución:**
- `Messages::feed` solo acumula. `next_message` extrae mensajes completos y
  rechaza un tamaño declarado menor que 6 (`src/message.rs`).
- El buffer vive en `Connection.messages`, en lugar de un mapa aparte en
  `Reader`.
- `last_read` se actualiza con cada lectura no vacía.
- Heartbeat cierra el lector con un mensaje incompleto e inactivo mandando
  `Drop` directamente al Cleaner (sin `shutdown().unwrap()`), con la misma
  cadencia de unos 30–90 s. Las comprobaciones de `closed` evitan el `Drop`
  duplicado desde reader y heartbeat.

**Evidencia:**
- 6 tests en `message.rs` y 1 en `reader.rs`.
- Prueba manual: cabecera partida, segmento de 4 bytes y 2×40 KB en un envío
  funcionan. Media cabecera sin más datos se cerró a los 89 s, y el id
  reutilizado quedó sin bytes residuales.

### 1. Escritura parcial no bloqueante — `adde98a`

(`adde98ad053162e32c1ed155d347fa188d13f5b6`)

**Problema (original):**
- Ante un `WouldBlock`, `write()` devolvía `Err` y perdía los bytes ya
  enviados.
- `try_write` trataba ese error como fatal.
- El mensaje ya se había sacado de la cola con `send_queue.remove(0)`.

Resultado: desconexión al primer `WouldBlock` y frames truncados.

**Solución:**
- `SendQueue` (`VecDeque<Vec<u8>>` más un offset `sent`, en
  `src/connection.rs`): el frame del frente se conserva hasta escribirse
  entero.
- `WouldBlock` deja el estado intacto para el siguiente evento de escritura.
- `Interrupted` reintenta; `Ok(0)` devuelve `BrokenPipe`; los demás errores
  son fatales.

**Evidencia:**
- 5 tests con un writer simulado que verifican el offset, la cola y el orden.
- Prueba manual: un cliente lento recibió 300 respuestas de 60000 bytes
  íntegras y en orden. El commit anterior cortaba la conexión con "operation
  would block".

### 3. Persistencia sin corrupción silenciosa — `ccc2edc`

(`ccc2edc52a03a307444371f9faa51c9d7e37d7b0`)

**Problema (original):**
- `save_to_file` truncaba `db.bin` *antes* de serializar y escribir, así que
  una interrupción dejaba el snapshot vacío o parcial.
- `load_from_file` ignoraba un archivo corrupto y arrancaba con el mapa vacío.
  La siguiente mutación marcaba el mapa como modificado, y el guardado
  posterior sobrescribía el archivo corrupto.
- Los `unwrap()` de guardado solo mataban el hilo de DB.

**Solución (`src/db.rs`, `src/main.rs`):**
- Guardado: escribe `db.bin.tmp`, `write_all`, `sync_all`, cierra y hace
  `fs::rename` sobre `db.bin`. Nunca borra ni trunca el original antes.
- `load_from_file` devuelve `Result`. Si falta el archivo o está vacío, arranca
  con el mapa vacío; cualquier otro fallo es un error que deja intactos el
  mapa y el archivo.
- Un error al guardar termina `DB::handle`, y `main` hace
  `eprintln!` + `process::exit(1)`. Un error al arrancar sale por el `Result`
  de `main`. No hay hook global de panic.

**Evidencia:**
- 4 tests en `db.rs`.
- Prueba manual en Windows: reinicio, archivo corrupto al arrancar (exit 1 y
  archivo intacto) y guardado fallido (exit 1 y snapshot previo intacto).
- En Docker linux/amd64, con un volumen de Docker Desktop: guardar, reiniciar,
  reemplazar un snapshot existente y reiniciar funcionan (tarea Docker, sin
  commit). No se ha probado en Linux nativo.

**Límites:**
- Se pierden los cambios que no estén en el último snapshot escrito con éxito.
  El ciclo de 4 s no es un plazo garantizado desde el `OK`.
- No hay fsync del directorio, así que no hay garantía ante un corte de
  energía.
- Un solo servidor por directorio de datos.

### 6. Docker — ya resuelto en `ac405f5`, antes de esta revisión

El Dockerfile ya usaba `rust:1.98.1`; el §6 original estaba desactualizado.
Verificado sin cambios de código:
- El build local funciona.
- El `Cargo.lock` usado en el build es idéntico a `HEAD`.
- Un build con `--locked` pasa (verificación independiente).
- Arranca y responde a set/get por el protocolo.

El runtime distroless Debian 12 (glibc 2.36) es compatible con el binario
actual, que requiere como máximo `GLIBC_2.34`. El builder es Debian 13, así que
una dependencia futura podría requerir un glibc más nuevo; hoy no ocurre.

### Tests

"Tests: cero" ya no aplica. Hay 16 tests de regresión:
- 6 de framing (`message.rs`),
- 1 de lectura (`reader.rs`),
- 5 de escritura (`connection.rs`),
- 4 de persistencia (`db.rs`).

### Guía de persistencia — `0f9f278`

`docs/persistence-change.html` explica solo el cambio #3. La ampliación a todos
los P0 está aplazada.

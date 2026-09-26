# Code review — BITE (v0.3.1)

Ordenado por prioridad. Los puntos marcados con **[verificado]** se reprodujeron
contra el binario real con un cliente Python.

**Resumen:** la arquitectura (un hilo por responsabilidad conectado con canales
`mpsc`, poller `smol` en oneshot) es clara y el código es fácil de seguir. Los
problemas de fondo no son de estilo sino de **corrección en el framing TCP, en
la escritura no bloqueante y en la persistencia**, más varios fallos que se
enmascaran silenciosamente. Hay también duplicación que se puede reducir sin
añadir abstracción especulativa.

---

## P0 — Corrección: afectan a cualquier cliente bajo uso real

### 1. Una escritura parcial cierra la conexión y pierde datos

`src/connection.rs:52-61,107-129`, `src/writer.rs:79-88`

`write()` devuelve `Err(WouldBlock)` tras haber escrito `total_written` bytes;
ese contador se pierde. `try_write` trata **cualquier** `Err` como fatal y marca
`closed = true`. Resultado:

- El mecanismo "poll writable → reintentar" nunca reintenta: en el primer
  `WouldBlock`, el cliente se desconecta.
- Antes de eso, `send_queue.remove(0)` ya sacó el mensaje de la cola; los bytes
  no escritos se pierden. Si el socket sobreviviera, el cliente recibiría un
  frame truncado seguido de la cabecera del siguiente → framing roto en el
  cliente.

Para un servidor que empuja notificaciones de suscripción a muchos clientes de
juego, esto aparece bajo carga.

**Fix:** mantener el mensaje al frente de la cola y avanzar un offset
(`VecDeque<Vec<u8>>` + `usize` de bytes ya enviados); tratar `WouldBlock` como
"no fatal" en `try_write`.

### 2. El framing rompe con fragmentación TCP normal **[verificado]**

`src/message.rs:57-70`, `src/reader.rs:73-97`

`Messages::feed` exige que el buffer acumulado tenga ≥ 6 bytes o cierra la
conexión. TCP no garantiza eso.

    # cabecera del 2º mensaje partida en dos segmentos
    split -> connection dropped
    # primer segmento de 4 bytes
    frag<6 -> connection dropped
    log: "Connection #1 closed, feed failed: Message received is smaller than 6 bytes"

Lo mismo con `buffer_len > 65535`: dos mensajes válidos de 40 KB que llegan en
un mismo `read` se rechazan aunque cada uno cumpla el protocolo. La comprobación
de tamaño debe aplicarse al `size` del **primer mensaje de la cabecera**, no al
buffer total. Con < 6 bytes lo correcto es devolver `Received::None` y esperar.

Además, `pending_read` tiene la semántica invertida (`reader.rs:80-94`): se pone
a `true` en `Pending` (que significa "hay *más* mensajes completos en el buffer")
y **no se toca** en `None` (el caso real de "mensaje incompleto esperando
bytes"). Consecuencia: `Heartbeat::drop_idle_readers` (`heartbeat.rs:38-50`)
nunca dispara para un cliente que envía media cabecera y se queda callado —
exactamente el caso para el que existe.

### 3. Persistencia: corrupción silenciosa con pérdida total

`src/db.rs:38-56,58-73`

- `save_to_file` trunca y escribe **in place**. Un crash / kill del contenedor
  a mitad de escritura deja `db.bin` corrupto.
- `load_from_file` hace `if let Ok(data) = bincode::deserialize(...)`: si el
  archivo está corrupto **arranca con mapa vacío en silencio**, y a los 4 s el
  siguiente `save_to_file` sobrescribe el archivo corrupto con un mapa vacío.
  Pérdida total de datos sin un solo log.

Viola directamente "never silently mask failures".

**Fix:** escribir a `db.bin.tmp` + `fs::rename`; en carga, fallar (panic/exit
con mensaje) si el archivo no está vacío y no deserializa.

---

## P1 — La implementación no coincide con `Commands.md`

### 4. `j`/`js` devuelven arrays de bytes, no strings **[verificado]**

`src/data.rs:281-293`

`kv_to_json` hace `json!(v)` con `v: &Vec<u8>`, que serde serializa como
secuencia de números:

    s data.name BITE  →  OK
    j data            →  {"name":[66,73,84,69]}      (Commands.md promete {"name":"BITE"})

Mientras que `#j` (`subs.rs:98-102`) sí usa `String::from_utf8_lossy` → string.
Inconsistente entre sí y con la documentación. Es un resto de la migración
"BITE is full bytes now" (8/2022). Decidir: o los valores JSON son
`from_utf8_lossy` (coherente con `#j`) o documentar que son bytes.

### 5. `+1` almacena y devuelve 8 bytes binarios **[verificado]**

`src/data.rs:104-131,309-320`

    +1 num  →  b'\x00\x00\x00\x00\x00\x00\x00\x01'
    g num   →  b'\x00\x00\x00\x00\x00\x00\x00\x01'

Commands.md dice `+1 numberkey → 10`. Además `vec_to_u64` tiene una heurística
reconocida como inexacta en su propio comentario ("12345678" se trata como u64
binario). Si el protocolo es textual (`s numberkey 9`), lo simple y coherente es
almacenar/devolver el número como texto y eliminar la heurística.

### 6. Docker no construye

`Dockerfile:1` usa `rust:1.76.0`; `Cargo.lock` es `version = 4` (requiere
≥ 1.78) → `docker-compose up --build` falla.

---

## P2 — Fallos que se enmascaran o matan el proceso

### 7. Panics silenciosos en hilos

`src/main.rs:117-124`

Ocho hilos con `.unwrap()` en `send`/`recv`/`poller.modify`. Si uno panickea,
el proceso sigue vivo pero cojo (p. ej. sin parser: acepta conexiones y no
responde nunca). Caso concreto: `heartbeat.rs:45`
`socket.shutdown(...).unwrap()` falla con `ENOTCONN` si el peer ya reseteó →
**el hilo de heartbeat muere para siempre**, sin log.

**Fix mínimo:** `panic::set_hook` que haga `process::abort()`, para que el fallo
sea explícito y Docker reinicie (`restart: unless-stopped`).

### 8. `server.accept()?` en el loop principal

`src/main.rs:134`

Un error transitorio de `accept` (`ECONNABORTED`, `EMFILE`, `WouldBlock` por
wakeup espurio) tumba el servidor. Aquí hay riesgo concreto para justificar un
`match` que loguee y continúe.

### 9. `_ => unreachable!()` sobre input externo

`src/main.rs:189`

Es una afirmación sobre eventos del SO. Si `polling` entrega un evento sin
`readable` ni `writable` (HUP/error en algunas plataformas), muere el hilo
principal.

### 10. IDs de cliente sin tope

`src/main.rs:127,144`

`id_count` es `usize` sin límite; el protocolo tiene 2 bytes. Con > 65535
conexiones concurrentes, `stamp_header` trunca en silencio y el cliente falla la
validación `message.from != id` con un error engañoso. Un
`assert!(client_id <= u16::MAX)` o rechazar la conexión es suficiente.

### 11. Fugas de memoria en `subs.rs`

`src/subs.rs:64-66`

`Del` usa `entry(key).or_default()`: cada `#- claveinexistente` crea un `Vec`
vacío que nunca se elimina. Usar `get_mut`. `id_keys` tampoco se limpia en
`Del`, solo en `DelAll`.

### 12. `SetList`: separador multibyte y clave vacía

`src/data.rs:83-100`

`key.chars().next().unwrap() as u8` trunca chars multibyte en silencio (docs:
"any byte"); un separador final (`sl | a 1|`) inserta la clave `""` en el mapa.

### 13. Búsqueda de hijos por prefijo de string, no de path

`src/data.rs:167-171` (y `204-211`, `240-247`)

`range(key..).take_while(starts_with(key))`: `k user` devuelve también
`username`. Si la intención es "hijos", el filtro debería ser
`k == key || k.starts_with(&format!("{key}."))`.

### 14. `SetIfNone` responde antes de saber el resultado

`src/parser.rs:151-162`

Responde `OK` **antes** de saber si la clave existía, así que el cliente no
puede distinguir "se escribió" de "ya existía". Si es lo deseado, documentarlo;
si no, la respuesta debe salir desde `data.rs`.

---

## P3 — Complejidad sin necesidad / simplificaciones

Nada de esto añade abstracción; la quita.

- **`parser.rs:103-270`**: el bloque
  `writer_tx.send(Queue(Order { from_id, to_id: from_id, msg_id, data: OK.into() })).unwrap()`
  se repite 10 veces. Un closure local `let reply = |data: &str| ...` reduce
  ~100 líneas y hace visible qué comandos responden `OK` vs `NO` vs nada.
- **`data.rs:166-172, 204-211, 240-247`**: el mismo
  `range + take_while + collect` tres veces (`todo.txt` ya lo anota). Una
  función local `children(&map, &key)` es un cambio directo.
- **`subs.rs:23-26, 110`**: `Sub.command: Command` obliga a un
  `_ => unreachable!()`. Un `enum SubKind { Get, KeyValue, Json }` codifica el
  contrato en el tipo y elimina la comprobación en runtime.
- **`parser.rs:333-359` `next_line`** con `#[allow(dead_code)]`: borrar.
  **`impl Display for Command`** (`parser.rs:54-58`) no se usa: borrar.
- **`parser.rs:80-88`**: `text` se asigna dos veces y compara `utf8.len()`
  (bytes) con un límite de chars. Alternativa:
  `let text = if utf8.chars().count() > LIMIT { format!("{} (..{LIMIT})", truncate(&utf8, LIMIT)) } else { utf8.into_owned() };`
- **`connection.rs:66-70`**: `vec![0; BUFFER_SIZE]` se asigna en cada
  iteración del loop; declarar `chunk` fuera.
- **`data.rs:181-186`**: `if let Some(last) = message.iter().last() { if last == &b'\0' { message.pop(); } }`
  → construir con `join(&b'\0')`, o `message.pop()` si `!key_value.is_empty()`.
- **`main.rs:37-39`**: `#[macro_use] extern crate log; extern crate pretty_env_logger;`
  sobra en edition 2021; `use log::info;` donde se use.
- **Dos `Connection` por cliente** (`main.rs:136-171`): la mitad de los campos
  de cada `Connection` están sin usar según el mapa (`send_queue`/`last_write`
  en readers; `pending_read`/`last_read` en writers). Separarlos evita
  contención entre hilos reader y writer — motivo válido — pero conviene
  hacerlo explícito (comentario o dos structs), porque hoy parece accidental.
- Clippy: `.split('.').last()` → `.next_back()` (`data.rs:178`,
  `subs.rs:92,99`).

---

## P4 — Higiene del repo y documentación

- `src/concat.py` y `src/concat.txt` (57 KB, copia del código) están en `src/`
  y trackeados. Mover el script fuera de `src` y añadir `concat.txt` a
  `.gitignore`.
- README: "storing on a BTreeMap serialized into a **json** file" → es bincode
  en `data/db.bin`. Commands.md: "stored sorted on **data/DB.json**" → idem.
  Commands.md usa `b data.author` para el comando `k`.
- **Tests: cero.** Solo se justifican ante riesgo concreto, y aquí lo hay:
  `Messages::feed` y `parse` son funciones puras con bugs demostrados (§2). El
  doc-comment de `parse` ya contiene un ejemplo listo para ser test. Cuatro o
  cinco tests de fragmentación (cabecera partida, dos mensajes en un read,
  mensaje parcial, `size` < 6 en cabecera) habrían atrapado §2.

---

## Lo que está bien

- Separación por hilos con canales tipados (`enum Action` por módulo): se lee
  de arriba abajo sin saltos.
- Protocolo binario mínimo y bien documentado en `Protocol.md`; la validación
  `from != id` es correcta.
- Reutilización de IDs vía `used_ids` y cleaner centralizado que limpia poller,
  mapas y suscripciones.
- `db_modified` con flag atómico y guardado con throttle: simple y suficiente.
- `parser.rs` trabaja en `&[u8]` con cursor y lifetimes en vez de copiar
  strings.

---

## Orden sugerido de trabajo

1. §1 escritura parcial y §2 framing — afectan a cualquier cliente bajo carga.
2. §3 persistencia — pérdida de datos silenciosa.
3. §4/§5 — que la implementación coincida con `Commands.md`; §6 Dockerfile.
4. §7-§14 — fallos enmascarados.
5. §P3/§P4 — simplificaciones e higiene, idealmente junto con los cambios
   anteriores en los mismos archivos.

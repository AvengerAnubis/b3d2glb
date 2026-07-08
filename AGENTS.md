# Project context for AI agents

## Что это за проект

`b3d2glb` конвертирует Blitz3D `.b3d` модели в формат glTF 2.0 (бинарный
`.glb` или отдельные `.gltf` + `.bin` + текстуры).

Часть проекта **Stranded II remake**. Оригинальные файлы игры:

```
/home/admen/Games/umu/umu-default/drive_c/Games/StrandedII/
```

## Лицензия

Проект распространяется под **GNU GPL v3** (см. `LICENSE`).

Весь код в `src/` — GPLv3, за исключением `src/b3d_parser.rs` и
`src/b3d_parser/utils.rs`, которые содержат код, производный от крейта
[`b3d`](https://github.com/DotWith/b3d/) (DotWith, MIT OR Apache-2.0).
См. `NOTICE`.

## Архитектура

```
src/
  main.rs              — точка входа, поиск файлов, диспетчеризация
  cli.rs               — парсинг аргументов CLI
  math.rs              — Mat4, умножение, обращение, конвертация координат
  b3d.rs               — извлечение данных из B3D: джоинты, меш, анимация
  b3d_parser.rs        — парсер B3D-формата (производный от DotWith/b3d)
  b3d_parser/utils.rs  — вспомогательные типы (Vec2, Vec3, Vec4, Chunk)
  texture.rs           — поиск текстур, конвертация в PNG, кеш на диске
  writer.rs            — генерация glTF/GLB
  lib.rs               — реэкспорт модулей
  bin/dump.rs          — утилита для дампа B3D-файлов
```

## Системы координат

- B3D: левая, Y-up. glTF: правая, Y-up.
- Позиции/нормали: `swap_yz_pos` меняет Y и Z местами.
- Кватернионы: `[w, x, y, z]` → negate Z → `[x, y, z, w]` для glTF.
- Матрицы внутри проекта — **row-major** (`m[row][col]`), но пишутся в буфер
  **column-by-column** (glTF требует column-major).

## Ключевые технические детали

### Матричная конвенция

`b3d_to_mat4` возвращает row-major TRS-матрицу с трансляцией в `m[3][0..2]`
(последняя строка). Это *транспонировано* относительно стандартной
column-major конвенции, где трансляция в `m[0..2][3]` (последний столбец).

`compute_world_matrix(parent, local)` = `parent * local` — умножение работает
корректно в row-major.

### IBM (inverse bind matrix) сериализация

IBMs пишутся в GLB-буфер **column-by-column** для column-major лэйаута glTF:

```rust
for col in 0..4 {
    ibm_data.extend_from_slice(&inv[0][col].to_le_bytes());
    ibm_data.extend_from_slice(&inv[1][col].to_le_bytes());
    ibm_data.extend_from_slice(&inv[2][col].to_le_bytes());
    ibm_data.extend_from_slice(&inv[3][col].to_le_bytes());
}
```

Старая реализация писала row-by-row → транспонированные IBM → чёрный
растянутый рендер в Bevy.

### B3D vertex-joint mapping

B3D хранит **максимум 1 кость на вершину** (weight=1.0). Не-скиненные
вершины получают joint=0/weight=0 (4-широкие JOINT/WEIGHT векторы с
нулями).

### Флаги B3D-вертексов (`Verts.flags`)

- `flags & 1` — есть нормали. Если **нет** (флаг=0) → нормали
  **вычисляются** из треугольников (функция `compute_normals()` в b3d.rs).
- `flags & 2` — есть цвет вершины.

### Прозрачность текстур

`alphaMode` в glTF-материале определяется так:
1. B3D-флаги текстуры: `flags & 2` (alpha канал), `flags & 4` (color key),
   `blend == 1` (alpha blend mode).
2. **Фолбэк**: если флаги молчат — проверяются фактические пиксели PNG
   (`png_has_alpha()`). Если есть не полностью непрозрачные пиксели →
   `alphaMode: "MASK"` + `alphaCutoff: 0.5`.

### Поиск текстур (`find_texture`)

Стратегии (первое совпадение побеждает):
1. `context_dir / raw_path` (сохраняет структуру директорий из B3D)
2. `context_dir / filename` (только имя файла)
3. `context_dir / lowercase_filename`
4. Легаси-пути Stranded II: `mods/Stranded II/gfx/` и `gfx/`

Текстуры кешируются как PNG в `<out>/textures/<stem>.png`.

## Library API

### `Converter` (builder)

```rust
use b3d2glb::writer::Converter;

// B3D → GLB в память
let glb: Vec<u8> = Converter::new("model", "/path/to/game")
    .convert_bytes(&b3d_data)?;

// С опциями
let glb = Converter::new("model", "/path/to/game")
    .glb(true)
    .material(0.0, 0.9)
    .color_override(1.0, 0.0, 0.0, 0.5)
    .tex_cache(&"/tmp/cache")
    .convert_bytes(&b3d_data)?;

// В файл
Converter::new("model", "/path/to/game")
    .convert_to_file(input_path, output_path)?;

// Низкоуровневый доступ к glTF-структурам
let (json, bin, images) = Converter::new("model", "/path/to/game")
    .build(&b3d_data)?;
```

### Публичные функции (модуль `writer`)

| Функция | Описание |
|---------|----------|
| `build_gltf_inner(...)` | Распаршенные B3D → `(JSON, Buffer, Images)` |
| `pad_to_4(data)` | Выровнять до 4 байт |
| `pad_to_4_in_place(data)` | Выровнять до 4 байт (in-place) |

### Публичные функции (модуль `b3d`)

| Функция | Описание |
|---------|----------|
| `B3D::read(&bytes)` | Распарсить B3D |
| `collect_mesh(&b3d)` | Извлечь меш |
| `collect_joints(&b3d)` | Извлечь кости |
| `collect_anims(&b3d)` | Извлечь анимации |

### CLI аргументы

| Флаг | Назначение |
|------|-----------|
| `-b` / `--glb` | бинарный GLB вместо раздельных файлов |
| `-o DIR` / `--out DIR` | выходная директория |
| `-c DIR` / `--context DIR` | корневая директория для поиска текстур |
| `-m VAL` / `--material VAL` | металлик/раффнесс (например `0.0m0.9r` или `0.0,0.9`) |
| `-C R,G,B[,A]` / `--color R,G,B[,A]` | базовый цвет фолбэка |
| `--help` | справка |

### Нормали

Если B3D-файл не содержит нормали (`flags & 1 == 0`), они вычисляются
из треугольников через взвешенное векторное произведение. Вычисление
происходит ПОСЛЕ конвертации координат в glTF-пространство (Z negate).

## Разработка

```bash
cargo build --release

# Тест с monkey.b3d (есть скин, текстура, анимация)
cargo run --bin b3d2glb --release -- -b -o /tmp/test \
  -c /path/to/StrandedII /path/to/StrandedII/gfx/monkey.b3d

# Дамп структуры B3D-файла
cargo run --bin dump --release -- /path/to/model.b3d

# Тесты
cargo test
```

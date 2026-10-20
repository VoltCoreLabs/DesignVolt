# Структура файлов DesignVolt

Цель: любой файл читается целиком за один заход. Новая фича = новый файл, а не рост старого.

## Лимиты
- Файл `.rs` ≤ ~300 строк (без тестов), функция ≤ ~50. Исключения — большие таблицы данных и файлы тестов.
- `mod.rs` / `lib.rs` — только объявления модулей и реэкспорт.
- Один файл = одна ответственность. Тесты — `#[cfg(test)] mod tests` внизу файла или отдельный `tests.rs`.

## Дерево

```
DesignVolt/
├─ CLAUDE.md, README.md, plan.md        правила для ИИ, описание, текущий план
├─ Cargo.toml                           [workspace] и общие версии зависимостей
├─ docs/                                stack, architecture, design-system, structure, decisions (ADR), log, roadmap
├─ assets/                              fonts/ (Golos Text), icons/phosphor/
├─ crates/
│  ├─ dv-core/src/                      document.rs (модель), command.rs (команды), history.rs (undo/redo), text.rs (формат файла)
│  ├─ dv-geom/src/                      transform.rs (матрицы, границы, попадание), index.rs (R-tree)
│  ├─ dv-render/                        src/viewport.rs, src/paint.rs, src/grid.rs (точки); examples/bench.rs
│  └─ dv-ui/src/                        draw.rs (Cx), text.rs, icons.rs, anim.rs, clipboard.rs, input.rs,
│                                       theme/ (colors, metrics, motion), widgets/ (button, segmented, text_edit, text_input)
└─ apps/desktop/src/
   ├─ main.rs, app.rs, chrome.rs, titlebar.rs     окно, экраны, ресайз, верхний бар
   ├─ input.rs, i18n.rs                           интерфейс Screen и курсоры; строки RU/EN
   ├─ library.rs, library_io.rs, store.rs         проекты и папки (модель), файлы данных, профиль и настройки
   ├─ screens/                                    welcome, settings, projects (+ _actions, _bar, _cards, _input, _menu, _modal, _paint)
   └─ editor/                                     state, interact (мышь), keys (клавиши), canvas, panels, toolbar,
                                                  layout, handles (маркеры), ops (операции над документом), ops_clip (буфер), ops_order (порядок слоёв), ops_layer (замок), reparent (перенос в фрейм), rename (имя слоя), layer_drag (перетаскивание строк слоёв), session (файл и автосохранение), tests
```

## Правила масштабирования
1. Новая команда документа → вариант `Command` в `dv-core/src/command.rs` с обратной командой и тестом.
2. Новый тип узла → `NodeKind` в `dv-core`, форма в `dv-geom` (попадание) и `dv-render` (рисование).
3. Новый виджет → файл в `dv-ui/src/widgets/`; значения только из токенов темы.
4. Строки интерфейса — только через `i18n::Msg`, не в коде.
5. Зависимости: `dv-core` ← `dv-geom` ← `dv-render` ← `apps/desktop`; `dv-ui` не зависит от документа.
6. Общие версии зависимостей — только в `[workspace.dependencies]`.

## Как ИИ читает код
1. `CLAUDE.md` → `plan.md` → нужный крейт; файл искать через Grep по имени типа или функции.
2. Файл больше лимита — сначала разбить, потом менять.

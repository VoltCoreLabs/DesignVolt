# Структура файлов DesignVolt

Цель: любой файл читается целиком за один заход. Новая фича = новый файл, а не рост старого.

## Лимиты (жёсткие)
- Файл `.rs` ≤ **300 строк** (без тестов); больше 400 — обязательно делить. Функция ≤ 50 строк.
- `mod.rs` — только `pub mod` / `pub use`, без логики.
- Тесты: небольшие — `#[cfg(test)] mod tests` внизу файла; большие — `tests/` или `foo/tests.rs`.
- Один файл = одна ответственность. У каждого крейта `README.md` на 5–10 строк (что делает, API, чего нет).

## Дерево (целевое; сейчас есть только `src/main.rs`)

```
DesignVolt/
├─ CLAUDE.md                 правила для ИИ
├─ plan.md                   задачи и этапы
├─ Cargo.toml                [workspace], общие версии зависимостей
├─ .cargo/config.toml        alias dev (cargo-watch)
├─ docs/
│  ├─ stack.md               зафиксированный стек
│  ├─ architecture.md        слои и правила
│  ├─ design-system.md       токены, анимации, виджеты (в WordVolt — design.md)
│  ├─ structure.md           этот файл
│  ├─ decisions.md           ADR
│  └─ log.md                 журнал сессий
├─ assets/                   шрифты, иконки, тема
├─ crates/
│  ├─ dv-core/               документ, команды, undo
│  ├─ dv-geom/               геометрия, булевы операции, hit-test
│  ├─ dv-render/             Vello-сцена
│  ├─ dv-text/               текст
│  └─ dv-ui/                 дизайн-система, виджеты, анимации
└─ apps/desktop/             бинарник `designvolt` (main.rs ≤ 50 строк)
```

### dv-core
```
src/
├─ lib.rs
├─ document/   document.rs, page.rs, node.rs (arena), shape.rs, text_node.rs
├─ style/      fill.rs, stroke.rs, effect.rs, gradient.rs
├─ command/    mod.rs (trait + Executor), один файл на команду
├─ history/    undo_stack.rs, coalesce.rs
└─ error.rs
```

### dv-ui
```
src/
├─ lib.rs
├─ theme/      colors.rs, spacing.rs, radius.rs, typography.rs, shadow.rs, motion.rs
├─ anim/       animated.rs, spring.rs, easing.rs, scheduler.rs
├─ ui_kit/     button.rs, icon_button.rs, toggle.rs, text_input.rs, number_field.rs, slider.rs, dropdown.rs, tooltip.rs, context_menu.rs
├─ panels/     layers.rs, properties.rs, toolbar.rs
└─ canvas/     canvas_view.rs, viewport.rs, tools/
```

## Правила масштабирования
1. Новая команда редактирования → новый файл в `dv-core/src/command/` + строка в `mod.rs`.
2. Новый тип узла → `document/<name>.rs` + вариант в `Node` + рендер в `dv-render` + hit-test в `dv-geom`.
3. Новый виджет → файл в `dv-ui/src/ui_kit/`; значения — только из токенов.
4. Зависимости строго вниз: `app → ui → render → geom → core`. Нарушение = ошибка ревью.
5. Общие версии зависимостей — только в `[workspace.dependencies]`.
6. Публичный API крейта — через `lib.rs` (`pub use`), внутренности `pub(crate)`.

## Как ИИ читает код
1. `CLAUDE.md` → `plan.md` (текущий этап) → `crates/<name>/README.md`.
2. Нужный файл находить через Grep по имени типа/функции, читать только его.
3. Файл > 300 строк — сначала разбить, потом менять.

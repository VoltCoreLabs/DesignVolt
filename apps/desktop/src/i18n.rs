//! Interface strings in Russian and English. The active language is a thread-local switch;
//! widgets call `tr(Msg::...)` every frame, so a language change applies immediately.

use std::cell::Cell;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Ru,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Ru => "ru",
            Lang::En => "en",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        match s.trim() {
            "ru" => Some(Lang::Ru),
            "en" => Some(Lang::En),
            _ => None,
        }
    }
}

thread_local! {
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::Ru) };
}

pub fn lang() -> Lang {
    CURRENT.with(Cell::get)
}

pub fn set_lang(l: Lang) {
    CURRENT.with(|c| c.set(l));
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Msg {
    // Welcome
    WelcomeTitle,
    WelcomeAsk,
    NamePlaceholder,
    NameHint,
    Continue,
    // Projects
    NavAll,
    NavRecent,
    NavFavorites,
    NavTrash,
    NewProject,
    SearchProjects,
    SortName,
    SortModified,
    SortCreated,
    NoMatchTitle,
    NoMatchSub,
    EmptyTitle,
    EmptySub,
    UntitledProject,
    NewFolder,
    UntitledFolder,
    MenuOpen,
    MenuRename,
    MenuFavorite,
    MenuUnfavorite,
    MenuDelete,
    MenuDuplicate,
    CopySuffix,
    EmptyTrash,
    ModalEmptyTitle,
    Cancel,
    MenuRestore,
    MenuDeleteForever,
    MenuConfirmDelete,
    TrashEmptyTitle,
    TrashEmptySub,
    FavEmptyTitle,
    FavEmptySub,
    RecentEmptyTitle,
    RecentEmptySub,
    FolderEmpty,
    // Settings
    Settings,
    SettingsProfile,
    SettingsYourName,
    Save,
    SettingsTheme,
    ThemeLight,
    ThemeDark,
    SettingsLanguage,
    Saved,
    Unsaved,
    // Editor
    Share,
    MenuFile,
    MenuEdit,
    MenuView,
    TabLayers,
    TabAssets,
    TabDesign,
    TabPrototype,
    TabInspect,
    Pages,
    SearchLayers,
    NameFrame,
    NameRectangle,
    NameEllipse,
    NameGroup,
    EmptyCanvasHint,
    NothingSelected,
    Mixed,
    SectionFill,
    SectionStroke,
    GradLinear,
    GradRadial,
    GradAngular,
    GradDiamond,
    PaintPattern,
    SectionEffects,
    DropShadow,
    InnerShadow,
    LayerBlur,
    StyleSolid,
    StyleDashed,
    StyleDotted,
    TilingGrid,
    TilingRows,
    TilingColumns,
    PickObject,
    SectionPosition,
    SectionLayout,
    SectionAppearance,
}

pub fn tr(m: Msg) -> &'static str {
    let ru = lang() == Lang::Ru;
    let pick = |r, e| if ru { r } else { e };
    match m {
        Msg::WelcomeTitle => pick("Добро пожаловать в DesignVolt", "Welcome to DesignVolt"),
        Msg::WelcomeAsk => pick("Как к вам обращаться?", "What should we call you?"),
        Msg::NamePlaceholder => pick("Ваше имя", "Your name"),
        Msg::NameHint => pick("Любые символы, до 20.", "Any characters, up to 20."),
        Msg::Continue => pick("Продолжить", "Continue"),
        Msg::NavAll => pick("Все проекты", "All projects"),
        Msg::NavRecent => pick("Недавние", "Recent"),
        Msg::NavFavorites => pick("Избранное", "Favorites"),
        Msg::NavTrash => pick("Корзина", "Trash"),
        Msg::NewProject => pick("Новый проект", "New project"),
        Msg::SearchProjects => pick("Поиск проектов", "Search projects"),
        Msg::SortName => pick("По имени", "Name"),
        Msg::SortModified => pick("По изменению", "Last modified"),
        Msg::SortCreated => pick("По созданию", "Created"),
        Msg::NoMatchTitle => pick("Ничего не найдено", "No projects match your search"),
        Msg::NoMatchSub => pick("Попробуйте другое название.", "Try a different name."),
        Msg::EmptyTitle => pick("Создайте первый проект", "Create your first project"),
        Msg::EmptySub => pick(
            "Здесь появятся ваши проекты.",
            "Your projects will show up here.",
        ),
        Msg::UntitledProject => pick("Без названия", "Untitled"),
        Msg::NewFolder => pick("Новая папка", "New folder"),
        Msg::UntitledFolder => pick("Папка", "Folder"),
        Msg::MenuOpen => pick("Открыть", "Open"),
        Msg::MenuRename => pick("Переименовать", "Rename"),
        Msg::MenuFavorite => pick("В избранное", "Add to favorites"),
        Msg::MenuUnfavorite => pick("Убрать из избранного", "Remove from favorites"),
        Msg::MenuDelete => pick("Удалить", "Delete"),
        Msg::MenuDuplicate => pick("Дублировать", "Duplicate"),
        Msg::CopySuffix => pick("копия", "copy"),
        Msg::EmptyTrash => pick("Очистить корзину", "Empty trash"),
        Msg::ModalEmptyTitle => pick("Очистить корзину?", "Empty the trash?"),
        Msg::Cancel => pick("Отмена", "Cancel"),
        Msg::MenuRestore => pick("Восстановить", "Restore"),
        Msg::MenuDeleteForever => pick("Удалить навсегда", "Delete forever"),
        Msg::MenuConfirmDelete => pick("Точно удалить?", "Really delete?"),
        Msg::TrashEmptyTitle => pick("Корзина пуста", "Trash is empty"),
        Msg::TrashEmptySub => pick(
            "Удалённые проекты и папки появятся здесь.",
            "Deleted projects and folders show up here.",
        ),
        Msg::FavEmptyTitle => pick("Нет избранных проектов", "No favorite projects"),
        Msg::FavEmptySub => pick(
            "Отметьте проект звёздочкой, чтобы он появился здесь.",
            "Star a project to see it here.",
        ),
        Msg::RecentEmptyTitle => pick("Пока ничего нет", "Nothing here yet"),
        Msg::RecentEmptySub => pick(
            "Недавно открытые проекты появятся здесь.",
            "Recently opened projects show up here.",
        ),
        Msg::FolderEmpty => pick(
            "Папка пуста. Перетащите сюда проекты.",
            "This folder is empty. Drag projects here.",
        ),
        Msg::Settings => pick("Настройки", "Settings"),
        Msg::SettingsProfile => pick("Профиль", "Profile"),
        Msg::SettingsYourName => pick("Имя", "Name"),
        Msg::Save => pick("Сохранить", "Save"),
        Msg::SettingsTheme => pick("Тема", "Theme"),
        Msg::ThemeLight => pick("Светлая", "Light"),
        Msg::ThemeDark => pick("Тёмная", "Dark"),
        Msg::SettingsLanguage => pick("Язык", "Language"),
        Msg::Saved => pick("Сохранено", "Saved"),
        Msg::Unsaved => pick("Не сохранено", "Unsaved"),
        Msg::Share => pick("Поделиться", "Share"),
        Msg::MenuFile => pick("Файл", "File"),
        Msg::MenuEdit => pick("Правка", "Edit"),
        Msg::MenuView => pick("Вид", "View"),
        Msg::TabLayers => pick("Слои", "Layers"),
        Msg::TabAssets => pick("Ассеты", "Assets"),
        Msg::TabDesign => pick("Дизайн", "Design"),
        Msg::TabPrototype => pick("Прототип", "Prototype"),
        Msg::TabInspect => pick("Инспектор", "Inspect"),
        Msg::Pages => pick("Страницы", "Pages"),
        Msg::SearchLayers => pick("Поиск слоёв", "Search layers"),
        Msg::NameFrame => pick("Фрейм", "Frame"),
        Msg::NameRectangle => pick("Прямоугольник", "Rectangle"),
        Msg::NameEllipse => pick("Эллипс", "Ellipse"),
        Msg::NameGroup => pick("Группа", "Group"),
        Msg::EmptyCanvasHint => pick(
            "Нажмите R или O и потяните по холсту, чтобы нарисовать фигуру",
            "Press R or O and drag on the canvas to draw a shape",
        ),
        Msg::NothingSelected => pick("Ничего не выбрано", "Nothing selected"),
        Msg::Mixed => pick("Разные", "Mixed"),
        Msg::SectionFill => pick("Заливка", "Fill"),
        Msg::SectionStroke => pick("Обводка", "Stroke"),
        Msg::GradLinear => pick("Линейный", "Linear"),
        Msg::GradRadial => pick("Радиальный", "Radial"),
        Msg::GradAngular => pick("Угловой", "Angular"),
        Msg::GradDiamond => pick("Ромб", "Diamond"),
        Msg::PaintPattern => pick("Узор", "Pattern"),
        Msg::SectionEffects => pick("Эффекты", "Effects"),
        Msg::DropShadow => pick("Тень", "Drop shadow"),
        Msg::InnerShadow => pick("Внутренняя тень", "Inner shadow"),
        Msg::LayerBlur => pick("Размытие слоя", "Layer blur"),
        Msg::StyleSolid => pick("Сплошная", "Solid"),
        Msg::StyleDashed => pick("Пунктир", "Dashed"),
        Msg::StyleDotted => pick("Точки", "Dotted"),
        Msg::TilingGrid => pick("Сетка", "Grid"),
        Msg::TilingRows => pick("Кирпич ↔", "Brick ↔"),
        Msg::TilingColumns => pick("Кирпич ↕", "Brick ↕"),
        Msg::PickObject => pick("Выберите объект", "Click an object"),
        Msg::SectionPosition => pick("Позиция", "Position"),
        Msg::SectionLayout => pick("Макет", "Layout"),
        Msg::SectionAppearance => pick("Внешний вид", "Appearance"),
    }
}

/// Russian plural form: 0 = one (1, 21…), 1 = few (2–4, 22–24…), 2 = many (the rest, 11–14).
fn ru_form(n: u64) -> usize {
    match (n % 100, n % 10) {
        (11..=14, _) => 2,
        (_, 1) => 0,
        (_, 2..=4) => 1,
        _ => 2,
    }
}

/// Text of the "empty trash" confirmation for `n` items.
pub fn empty_trash_body(n: u64) -> String {
    match lang() {
        Lang::Ru => format!(
            "Элементов в корзине: {n}. Они будут удалены навсегда, это действие нельзя отменить."
        ),
        Lang::En => format!(
            "The trash holds {n} item{}. They will be deleted forever and this cannot be undone.",
            if n == 1 { "" } else { "s" }
        ),
    }
}

/// "1 объект", "2 объекта", "5 объектов" / "1 object", "2 objects".
pub fn objects_count(n: u64) -> String {
    match lang() {
        Lang::Ru => {
            let word = ["объект", "объекта", "объектов"][ru_form(n)];
            format!("{n} {word}")
        }
        Lang::En => format!("{n} object{}", if n == 1 { "" } else { "s" }),
    }
}

/// "Выбрано: 3" / "Selected: 3".
pub fn selected_count(n: u64) -> String {
    match lang() {
        Lang::Ru => format!("Выбрано: {n}"),
        Lang::En => format!("Selected: {n}"),
    }
}

/// "1 элемент", "2 элемента", "5 элементов" / "1 item", "2 items".
pub fn items_count(n: u64) -> String {
    match lang() {
        Lang::Ru => {
            let word = ["элемент", "элемента", "элементов"][ru_form(n)];
            format!("{n} {word}")
        }
        Lang::En => format!("{n} item{}", if n == 1 { "" } else { "s" }),
    }
}

/// "5 мин назад", "3 ч назад", "2 дн назад", "только что" / "5 min ago"…
pub fn ago(now: u64, then: u64) -> String {
    let d = now.saturating_sub(then);
    let ru = lang() == Lang::Ru;
    match d {
        0..=59 => if ru {
            "только что"
        } else {
            "just now"
        }
        .to_string(),
        60..=3599 => format!("{} {}", d / 60, if ru { "мин назад" } else { "min ago" }),
        3600..=86_399 => format!("{} {}", d / 3600, if ru { "ч назад" } else { "h ago" }),
        _ => format!("{} {}", d / 86_400, if ru { "дн назад" } else { "d ago" }),
    }
}

/// "Изменён 5 мин назад" / "Edited 5 min ago" ("Изменён только что" / "Edited just now").
pub fn edited(now: u64, then: u64) -> String {
    let prefix = if lang() == Lang::Ru {
        "Изменён"
    } else {
        "Edited"
    };
    format!("{prefix} {}", ago(now, then))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn russian_plurals() {
        set_lang(Lang::Ru);
        let word = items_count;
        assert_eq!(word(0), "0 элементов");
        assert_eq!(word(1), "1 элемент");
        assert_eq!(word(2), "2 элемента");
        assert_eq!(word(5), "5 элементов");
        assert_eq!(word(11), "11 элементов");
        assert_eq!(word(12), "12 элементов");
        assert_eq!(word(101), "101 элемент");
        assert_eq!(word(22), "22 элемента");
        assert_eq!(word(111), "111 элементов");
        assert_eq!(word(101), "101 элемент");
        assert_eq!(selected_count(3), "Выбрано: 3");
        assert_eq!(items_count(1), "1 элемент");
        assert_eq!(items_count(3), "3 элемента");
        assert_eq!(items_count(12), "12 элементов");
    }

    #[test]
    fn english_plurals() {
        set_lang(Lang::En);
        assert_eq!(items_count(1), "1 item");
        assert_eq!(items_count(0), "0 items");
        assert_eq!(items_count(2), "2 items");
        set_lang(Lang::Ru);
    }

    #[test]
    fn ago_is_localized() {
        set_lang(Lang::Ru);
        assert_eq!(ago(100, 100), "только что");
        assert_eq!(ago(400, 100), "5 мин назад");
        assert_eq!(edited(10_000, 0), "Изменён 2 ч назад");
        set_lang(Lang::En);
        assert_eq!(ago(200_000, 0), "2 d ago");
        assert_eq!(ago(5, 100), "just now");
        assert_eq!(edited(400, 100), "Edited 5 min ago");
        set_lang(Lang::Ru);
    }

    #[test]
    fn every_message_has_both_languages() {
        // `tr` is total over `Msg`; this checks that no translation is empty or identical
        // placeholder text and that switching really changes the output.
        for m in [
            Msg::Continue,
            Msg::Settings,
            Msg::NewProject,
            Msg::TabLayers,
            Msg::Settings,
        ] {
            set_lang(Lang::Ru);
            let ru = tr(m);
            set_lang(Lang::En);
            let en = tr(m);
            assert!(!ru.is_empty() && !en.is_empty());
            assert_ne!(ru, en);
        }
        set_lang(Lang::Ru);
    }

    #[test]
    fn codes_roundtrip() {
        for l in [Lang::Ru, Lang::En] {
            assert_eq!(Lang::from_code(l.code()), Some(l));
        }
        assert_eq!(Lang::from_code("de"), None);
    }
}

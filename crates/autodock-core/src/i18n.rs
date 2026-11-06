//! Message language for everything the core says to a person.
//!
//! Every message carries both texts next to each other (`tr!("português", "english")`),
//! so a message cannot be added without its translation: the build fails instead.
//! The language of the running command is kept in a thread-local, set once at the
//! entry point (`with_lang`) so deep helper functions do not need a parameter.
use std::cell::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    Pt,
    #[default]
    En,
}

impl Lang {
    /// Parses "pt", "pt-BR", "pt_BR.UTF-8", "en-US"... Anything else is English.
    pub fn parse(value: Option<&str>) -> Lang {
        match value.map(|v| v.trim().to_ascii_lowercase()) {
            Some(v) if v == "pt" || v.starts_with("pt-") || v.starts_with("pt_") => Lang::Pt,
            _ => Lang::En,
        }
    }

    /// Language from the usual locale environment variables (used by the CLI).
    pub fn from_environment() -> Lang {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|name| std::env::var(name).ok())
            .find(|value| !value.is_empty())
            .map(|value| Lang::parse(Some(&value)))
            .unwrap_or_default()
    }
}

thread_local! {
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::En) };
}

/// The language messages are produced in on this thread.
pub fn current() -> Lang {
    CURRENT.with(Cell::get)
}

/// Runs `f` with `lang` as the message language, restoring the previous one after.
pub fn with_lang<T>(lang: Lang, f: impl FnOnce() -> T) -> T {
    let previous = CURRENT.with(|cell| cell.replace(lang));
    let result = f();
    CURRENT.with(|cell| cell.set(previous));
    result
}

/// `tr!("texto {x}", "text {x}", x = value)`: the Portuguese or English text
/// according to the current language. Both are checked as format strings.
macro_rules! tr {
    ($pt:literal, $en:literal $(, $($arg:tt)*)?) => {
        if $crate::i18n::current() == $crate::i18n::Lang::Pt {
            format!($pt $(, $($arg)*)?)
        } else {
            format!($en $(, $($arg)*)?)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_locale_spellings() {
        for pt in ["pt", "PT", "pt-BR", "pt_BR", "pt_BR.UTF-8", " pt-PT "] {
            assert_eq!(Lang::parse(Some(pt)), Lang::Pt, "{pt}");
        }
        for en in ["en", "en-US", "de", "", "ptx", "portuguese"] {
            assert_eq!(Lang::parse(Some(en)), Lang::En, "{en}");
        }
        assert_eq!(Lang::parse(None), Lang::En);
    }

    #[test]
    fn language_is_scoped_to_the_closure_and_the_thread() {
        assert_eq!(current(), Lang::En);
        let inside = with_lang(Lang::Pt, || {
            let nested = with_lang(Lang::En, current);
            (current(), nested)
        });
        assert_eq!(inside, (Lang::Pt, Lang::En));
        assert_eq!(current(), Lang::En);
        with_lang(Lang::Pt, || {
            std::thread::spawn(|| assert_eq!(current(), Lang::En))
                .join()
                .unwrap();
        });
    }

    #[test]
    fn tr_picks_the_text_and_formats_arguments() {
        let value = 7;
        assert_eq!(
            with_lang(Lang::Pt, || tr!("sete: {value}", "seven: {value}")),
            "sete: 7"
        );
        assert_eq!(
            with_lang(Lang::En, || tr!("sete: {value}", "seven: {value}")),
            "seven: 7"
        );
        assert_eq!(with_lang(Lang::En, || tr!("a {}", "b {}", 1 + 1)), "b 2");
    }
}

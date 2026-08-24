#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    Ru,
    De,
}

impl Locale {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "en" => Some(Self::En),
            "ru" => Some(Self::Ru),
            "de" => Some(Self::De),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluralForm {
    One,
    Few,
    Many,
    Other,
}

pub fn plural_form(locale: Locale, n: u32) -> PluralForm {
    match locale {
        Locale::En | Locale::De => {
            if n == 1 {
                PluralForm::One
            } else {
                PluralForm::Other
            }
        }
        Locale::Ru => {
            let mod10 = n % 10;
            let mod100 = n % 100;
            if mod10 == 1 && mod100 != 11 {
                PluralForm::One
            } else if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) {
                PluralForm::Few
            } else if mod10 == 0 || (5..=9).contains(&mod10) || (11..=14).contains(&mod100) {
                PluralForm::Many
            } else {
                PluralForm::Other
            }
        }
    }
}

fn plural_form_key(form: &PluralForm) -> &'static str {
    match form {
        PluralForm::One => "one",
        PluralForm::Few => "few",
        PluralForm::Many => "many",
        PluralForm::Other => "other",
    }
}

pub struct I18nStrings {
    locale: Locale,
    data: toml::Value,
}

impl I18nStrings {
    pub fn load(locale: Locale, toml_content: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let data: toml::Value = toml::from_str(toml_content)?;
        Ok(Self { locale, data })
    }

    pub fn get(&self, key: &str) -> String {
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = &self.data;
        for part in &parts {
            current = match current.get(part) {
                Some(v) => v,
                None => return key.to_string(),
            };
        }
        match current {
            toml::Value::String(s) => s.clone(),
            toml::Value::Table(t) => {
                let form = plural_form(self.locale, 0);
                let form_key = plural_form_key(&form);
                t.get(form_key)
                    .and_then(|v| v.as_str())
                    .unwrap_or(key)
                    .to_string()
            }
            _ => key.to_string(),
        }
    }

    pub fn get_plural(&self, key: &str, n: u32) -> String {
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = &self.data;
        for part in &parts {
            current = match current.get(part) {
                Some(v) => v,
                None => return format!("{key}[{n}]"),
            };
        }
        if let toml::Value::Table(t) = current {
            let form = plural_form(self.locale, n);
            let form_key = plural_form_key(&form);
            let template = t.get(form_key).and_then(|v| v.as_str()).unwrap_or(key);
            template.replace("{}", &n.to_string())
        } else {
            format!("{key}[{n}]")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── plural_form: English ──

    #[test]
    fn plural_form_en_one() {
        assert_eq!(plural_form(Locale::En, 1), PluralForm::One);
    }

    #[test]
    fn plural_form_en_other() {
        assert_eq!(plural_form(Locale::En, 0), PluralForm::Other);
        assert_eq!(plural_form(Locale::En, 2), PluralForm::Other);
        assert_eq!(plural_form(Locale::En, 5), PluralForm::Other);
    }

    // ── plural_form: Russian ──

    #[test]
    fn plural_form_ru_one() {
        assert_eq!(plural_form(Locale::Ru, 1), PluralForm::One);
        assert_eq!(plural_form(Locale::Ru, 21), PluralForm::One);
        assert_eq!(plural_form(Locale::Ru, 101), PluralForm::One);
    }

    #[test]
    fn plural_form_ru_few() {
        assert_eq!(plural_form(Locale::Ru, 2), PluralForm::Few);
        assert_eq!(plural_form(Locale::Ru, 3), PluralForm::Few);
        assert_eq!(plural_form(Locale::Ru, 4), PluralForm::Few);
        assert_eq!(plural_form(Locale::Ru, 22), PluralForm::Few);
    }

    #[test]
    fn plural_form_ru_many() {
        assert_eq!(plural_form(Locale::Ru, 0), PluralForm::Many);
        assert_eq!(plural_form(Locale::Ru, 5), PluralForm::Many);
        assert_eq!(plural_form(Locale::Ru, 11), PluralForm::Many);
        assert_eq!(plural_form(Locale::Ru, 14), PluralForm::Many);
        assert_eq!(plural_form(Locale::Ru, 20), PluralForm::Many);
    }

    // ── plural_form: German ──

    #[test]
    fn plural_form_de_one() {
        assert_eq!(plural_form(Locale::De, 1), PluralForm::One);
    }

    #[test]
    fn plural_form_de_other() {
        assert_eq!(plural_form(Locale::De, 0), PluralForm::Other);
        assert_eq!(plural_form(Locale::De, 2), PluralForm::Other);
    }

    // ── I18nStrings ──

    const EN_TOML: &str = include_str!("../../../assets/i18n/en.toml");

    #[test]
    fn i18n_load_and_get_simple_key() {
        let i18n = I18nStrings::load(Locale::En, EN_TOML).expect("load en.toml");
        assert_eq!(i18n.get("format.zip"), "ZIP");
    }

    #[test]
    fn i18n_get_plural() {
        let i18n = I18nStrings::load(Locale::En, EN_TOML).expect("load en.toml");
        assert_eq!(i18n.get_plural("progress.entries", 3), "3 entries");
    }
}

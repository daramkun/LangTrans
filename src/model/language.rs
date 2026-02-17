use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    En,
    Es,
    Fr,
    De,
    Pt,
    Ja,
    Ko,
    Zh,
    Ar,
    Ru,
    Hi,
}

impl Language {
    pub fn from_code(code: &str) -> Result<Self, AppError> {
        match code.to_lowercase().as_str() {
            "en" => Ok(Language::En),
            "es" => Ok(Language::Es),
            "fr" => Ok(Language::Fr),
            "de" => Ok(Language::De),
            "pt" => Ok(Language::Pt),
            "ja" => Ok(Language::Ja),
            "ko" => Ok(Language::Ko),
            "zh" => Ok(Language::Zh),
            "ar" => Ok(Language::Ar),
            "ru" => Ok(Language::Ru),
            "hi" => Ok(Language::Hi),
            _ => Err(AppError::BadRequest(format!(
                "Unsupported language code: '{}'. Supported: en, es, fr, de, pt, ja, ko, zh, ar, ru, hi",
                code
            ))),
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Language::En => "English",
            Language::Es => "Spanish",
            Language::Fr => "French",
            Language::De => "German",
            Language::Pt => "Portuguese",
            Language::Ja => "Japanese",
            Language::Ko => "Korean",
            Language::Zh => "Chinese",
            Language::Ar => "Arabic",
            Language::Ru => "Russian",
            Language::Hi => "Hindi",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_code_valid() {
        assert_eq!(Language::from_code("en").unwrap(), Language::En);
        assert_eq!(Language::from_code("KO").unwrap(), Language::Ko);
        assert_eq!(Language::from_code("Ja").unwrap(), Language::Ja);
    }

    #[test]
    fn test_from_code_invalid() {
        assert!(Language::from_code("xx").is_err());
        assert!(Language::from_code("").is_err());
    }

    #[test]
    fn test_display_name() {
        assert_eq!(Language::En.display_name(), "English");
        assert_eq!(Language::Ko.display_name(), "Korean");
    }
}

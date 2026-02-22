use super::language::Language;

pub fn build_translation_prompt(from: Language, to: Language, text: &str) -> String {
    // Qwen2.5 ChatML format
    format!(
        "<|im_start|>system\n\
         You are a professional translator.<|im_end|>\n\
         <|im_start|>user\n\
         Translate the following text from {} to {}. Provide only the translation without any explanation.\n\n\
         {}<|im_end|>\n\
         <|im_start|>assistant\n",
        from.display_name(),
        to.display_name(),
        text
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_format() {
        let prompt = build_translation_prompt(Language::En, Language::Ko, "Hello world");
        assert!(prompt.contains("<|im_start|>system"));
        assert!(prompt.contains("translator"));
        assert!(prompt.contains("English"));
        assert!(prompt.contains("Korean"));
        assert!(prompt.contains("Hello world"));
        assert!(prompt.contains("<|im_start|>assistant"));
        assert!(prompt.ends_with("<|im_start|>assistant\n"));
    }
}

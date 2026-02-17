use super::language::Language;

pub fn build_translation_prompt(from: Language, to: Language, text: &str) -> String {
    format!(
        "<bos><start_of_turn>instruction\n\
         Translate the following text from {} to {}.\n\
         Provide the final translation immediately without any other text.<end_of_turn>\n\
         <start_of_turn>source\n\
         {}<end_of_turn>\n\
         <start_of_turn>translation\n",
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
        assert!(prompt.contains("<bos>"));
        assert!(prompt.contains("English"));
        assert!(prompt.contains("Korean"));
        assert!(prompt.contains("Hello world"));
        assert!(prompt.contains("<start_of_turn>translation"));
        assert!(prompt.ends_with("<start_of_turn>translation\n"));
    }
}

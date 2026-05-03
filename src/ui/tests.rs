use super::panels::editor_word_count;

#[test]
fn editor_word_count_ignores_punctuation_only_tokens() {
    assert_eq!(editor_word_count("."), 0);
    assert_eq!(editor_word_count("...   !"), 0);
}

#[test]
fn editor_word_count_still_counts_text_tokens() {
    assert_eq!(editor_word_count("hello"), 1);
    assert_eq!(editor_word_count("hello . world"), 2);
    assert_eq!(editor_word_count("note-1"), 1);
}

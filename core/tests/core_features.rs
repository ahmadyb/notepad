use notepad_core::{extract_by_colour, highlight_stats, replace_all, ColorOrder, EditorBuffer, FileManager, FindOptions, LineColour, LineEnding, LineMetadata, ListType, TextEncoding};

fn coloured(colour: LineColour) -> LineMetadata {
    LineMetadata { colour, ..LineMetadata::default() }
}

#[test]
fn highlight_toggle_and_extraction_are_deterministic() {
    let mut buffer = EditorBuffer::new("alpha\nbeta\ngamma");
    buffer.set_selection(0, buffer.text().len());
    buffer.apply_colour(LineColour::Yellow);
    assert_eq!(buffer.metadata().iter().filter(|m| m.colour == LineColour::Yellow).count(), 3);
    assert_eq!(
        extract_by_colour(buffer.text(), buffer.metadata(), &[LineColour::Yellow], ColorOrder::Document),
        "alpha\nbeta\ngamma"
    );
    let stats = highlight_stats(buffer.text(), buffer.metadata());
    assert_eq!(stats.highlighted_lines, 3);
}

#[test]
fn list_metadata_survives_newline_edits() {
    let mut buffer = EditorBuffer::from_parts(
        "- first\n- second",
        vec![
            LineMetadata { list_type: ListType::Bullet, ..LineMetadata::default() },
            LineMetadata { list_type: ListType::Bullet, ..LineMetadata::default() },
        ],
        7,
    );
    buffer.handle_enter(4);
    assert_eq!(buffer.line_count(), 3);
    assert_eq!(buffer.metadata()[1].list_type, ListType::Bullet);
}

#[test]
fn custom_colours_round_trip_through_grouped_output() {
    let metadata = vec![coloured(LineColour::Custom(0x123456)), coloured(LineColour::Blue)];
    assert_eq!(
        extract_by_colour("a\nb", &metadata, &[LineColour::Custom(0x123456)], ColorOrder::Grouped),
        "# #123456\na"
    );
}

#[test]
fn search_and_replace_support_unicode_and_literal_dollars() {
    let options = FindOptions::default();
    let (result, count) = replace_all("Éclair $1 éclair", "éclair", "$2", &options).unwrap();
    assert_eq!((result, count), ("$2 $2".to_owned(), 2));
}

#[test]
fn bom_and_line_ending_survive_an_atomic_save() {
    let root = std::env::temp_dir().join(format!("notepad-pro-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("bom.txt");
    FileManager::save_file_with_bom(&path, "one\ntwo", TextEncoding::Utf16Le, LineEnding::CrLf, true).unwrap();
    let loaded = FileManager::load_file(&path).unwrap();
    assert_eq!(loaded.encoding, TextEncoding::Utf16Le);
    assert!(loaded.had_bom);
    assert_eq!(loaded.line_ending, LineEnding::CrLf);
    assert_eq!(loaded.text, "one\ntwo");
    std::fs::remove_dir_all(root).unwrap();
}

use super::page_cursor;

#[test]
fn page_cursor_requires_an_extra_entry() {
    let mut entries = vec![40, 30, 20];
    assert_eq!(page_cursor(&mut entries, 2, |id| *id), Some(30));
    assert_eq!(entries, vec![40, 30]);
    assert_eq!(page_cursor(&mut entries, 2, |id| *id), None);

    entries.clear();
    assert_eq!(page_cursor(&mut entries, 2, |id| *id), None);
}

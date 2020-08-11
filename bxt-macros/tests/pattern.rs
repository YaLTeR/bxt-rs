use bxt_macros::pattern;

#[test]
fn uppercase() {
    assert_eq!(
        pattern!(01 AC ?? 44),
        &[Some(0x01), Some(0xAC), None, Some(0x44)],
    );
}

#[test]
fn lowercase() {
    assert_eq!(
        pattern!(01 ac ?? 44),
        &[Some(0x01), Some(0xAC), None, Some(0x44)],
    );
}

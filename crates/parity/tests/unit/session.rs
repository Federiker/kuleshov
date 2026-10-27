use super::*;

#[test]
fn uuids_are_numbered_by_first_appearance() {
    let mut uuids = Vec::new();
    let text = name_uuids(
        "out/0b7d2c55-58a4-4d1c-9a49-8c1e7bb3d1f2.mp3 list/9e2f4b1e-3c6a-4f7e-8d2b-1a5c6e7f8091.txt \
again/0b7d2c55-58a4-4d1c-9a49-8c1e7bb3d1f2.mp3",
        &mut uuids,
    );
    assert_eq!(text, "out/{uuid1}.mp3 list/{uuid2}.txt again/{uuid1}.mp3");
    // Numbering goes on across the texts of one scenario.
    assert_eq!(
        name_uuids("9e2f4b1e-3c6a-4f7e-8d2b-1a5c6e7f8091", &mut uuids),
        "{uuid2}"
    );
}

#[test]
fn text_that_only_looks_like_a_uuid_is_kept() {
    let mut uuids = Vec::new();
    for text in [
        "0B7D2C55-58A4-4D1C-9A49-8C1E7BB3D1F2",
        "0b7d2c55-58a4-4d1c-9a49-8c1e7bb3d1f",
        "张伟 O'Brien L'été",
    ] {
        assert_eq!(name_uuids(text, &mut uuids), text);
    }
    assert!(uuids.is_empty());
}

#[test]
fn ffmpeg_log_addresses_are_named() {
    assert_eq!(
        name_addresses("[in#0 @ 000001a61a1e2540] Error / [mp3 @ 0x55d0c8a4b2c0] x / a @ z] c @ ]"),
        "[in#0 @ {address}] Error / [mp3 @ {address}] x / a @ z] c @ ]"
    );
}

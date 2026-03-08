#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_runtime::slug::Slug;

/// Fuzz Slug::slugify and Slug::try_from.
/// Slug normalizes untrusted strings for use in URLs and NATS subjects.
/// Must not panic on any input.
fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else { return };

    // slugify must never panic
    let slug = Slug::slugify(s);
    let slug_str = slug.as_ref();

    // Slug output must only contain valid slug characters
    for c in slug_str.chars() {
        assert!(
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_',
            "slugify produced invalid char {:?} from input {:?}",
            c, &s[..s.len().min(100)]
        );
    }

    // slugify_unique must also not panic
    let unique = Slug::slugify_unique(s);
    let unique_str = unique.as_ref();
    for c in unique_str.chars() {
        assert!(
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_',
            "slugify_unique produced invalid char {:?} from input {:?}",
            c, &s[..s.len().min(100)]
        );
    }

    // try_from validation — must not panic, only Ok or Err
    let _ = Slug::try_from(s);
    let _ = Slug::try_from(s.to_string());

    // Determinism
    let slug2 = Slug::slugify(s);
    assert_eq!(slug_str, slug2.as_ref(), "slugify not deterministic");
});

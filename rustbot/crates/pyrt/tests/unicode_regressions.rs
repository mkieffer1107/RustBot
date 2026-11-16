use pyrt::{Value, unicode};

#[test]
fn canonical_order_is_stable_and_stops_at_starters() {
    assert_eq!(
        unicode::normalize("NFD", "a\u{0315}\u{0300}").unwrap(),
        "a\u{0300}\u{0315}"
    );
    assert_eq!(
        unicode::normalize("NFC", "a\u{0315}\u{0300}").unwrap(),
        "à\u{0315}"
    );
    // Equal combining classes preserve order, including at the start of text.
    assert_eq!(
        unicode::normalize("NFD", "\u{0301}\u{0327}\u{0300}").unwrap(),
        "\u{0327}\u{0301}\u{0300}"
    );
    assert_eq!(
        unicode::normalize("NFD", "a\u{0315}\u{0300}b\u{0315}\u{0327}").unwrap(),
        "a\u{0300}\u{0315}b\u{0327}\u{0315}"
    );
}

#[test]
fn composition_obeys_equal_class_blocking_and_class_zero_boundaries() {
    assert_eq!(
        unicode::normalize("NFC", "A\u{0305}\u{0301}").unwrap(),
        "A\u{0305}\u{0301}"
    );
    assert_eq!(
        unicode::normalize("NFC", "A\u{0327}\u{0301}").unwrap(),
        "Á\u{0327}"
    );
    // CGJ is a class-zero boundary: composition cannot reach the earlier A.
    assert_eq!(
        unicode::normalize("NFC", "A\u{034f}\u{030a}").unwrap(),
        "A\u{034f}\u{030a}"
    );
    // Recursive composition may consume multiple marks without blocking the
    // next mark when the preceding mark was itself consumed.
    assert_eq!(unicode::normalize("NFC", "A\u{030a}\u{0301}").unwrap(), "Ǻ");
}

#[test]
fn singleton_decompositions_and_composition_exclusions_are_distinct() {
    assert_eq!(unicode::normalize("NFC", "ÅΩK").unwrap(), "ÅΩK");
    assert_eq!(unicode::normalize("NFD", "ÅΩK").unwrap(), "A\u{030a}ΩK");
    for (original, decomposed) in [
        ("\u{0344}", "\u{0308}\u{0301}"),
        ("\u{0958}", "\u{0915}\u{093c}"),
        ("\u{0f73}", "\u{0f71}\u{0f72}"),
    ] {
        for form in ["NFC", "NFD", "NFKC", "NFKD"] {
            assert_eq!(unicode::normalize(form, original).unwrap(), decomposed);
            assert_eq!(unicode::normalize(form, decomposed).unwrap(), decomposed);
        }
    }
}

#[test]
fn hangul_decomposition_and_composition_are_algorithmic() {
    for (syllable, jamo) in [
        ("가", "\u{1100}\u{1161}"),
        ("각", "\u{1100}\u{1161}\u{11a8}"),
        ("힣", "\u{1112}\u{1175}\u{11c2}"),
    ] {
        for form in ["NFD", "NFKD"] {
            assert_eq!(unicode::normalize(form, syllable).unwrap(), jamo);
        }
        for form in ["NFC", "NFKC"] {
            assert_eq!(unicode::normalize(form, jamo).unwrap(), syllable);
        }
    }
    assert_eq!(unicode::normalize("NFC", "가\u{11a8}").unwrap(), "각");
    assert_eq!(
        unicode::normalize("NFC", "\u{1100}\u{1161}\u{11a8}\u{11a8}").unwrap(),
        "각\u{11a8}"
    );
    // A nonzero combining class between L and V prevents Hangul composition.
    assert_eq!(
        unicode::normalize("NFC", "\u{1100}\u{0301}\u{1161}").unwrap(),
        "\u{1100}\u{0301}\u{1161}"
    );
}

#[test]
fn compatibility_forms_recurse_before_ordering_and_composition() {
    let compatibility = "\u{fb03}\u{2460}\u{00a0}\u{2163}";
    assert_eq!(
        unicode::normalize("NFD", compatibility).unwrap(),
        compatibility
    );
    assert_eq!(
        unicode::normalize("NFKD", compatibility).unwrap(),
        "ffi1 IV"
    );
    assert_eq!(
        unicode::normalize("NFKC", compatibility).unwrap(),
        "ffi1 IV"
    );
    assert_eq!(
        unicode::normalize("NFKD", "\u{ff76}\u{ff9e}").unwrap(),
        "カ\u{3099}"
    );
    assert_eq!(
        unicode::normalize("NFKC", "\u{ff76}\u{ff9e}").unwrap(),
        "ガ"
    );
    assert_eq!(
        unicode::normalize("NFD", "\u{1e9b}\u{0323}").unwrap(),
        "\u{017f}\u{0323}\u{0307}"
    );
    assert_eq!(
        unicode::normalize("NFKD", "\u{1e9b}\u{0323}").unwrap(),
        "s\u{0323}\u{0307}"
    );
    assert_eq!(
        unicode::normalize("NFKC", "\u{1e9b}\u{0323}").unwrap(),
        "\u{1e69}"
    );
}

#[test]
fn combining_classes_include_unicode_16_additions() {
    for (character, class) in [
        ('a', 0),
        ('\u{0300}', 230),
        ('\u{0315}', 232),
        ('\u{0327}', 202),
        ('\u{034f}', 0),
        ('\u{093c}', 7),
        ('\u{0f71}', 129),
        ('\u{1e944}', 230),
        ('\u{10efd}', 220),
        ('\u{113ce}', 9),
        ('\u{1612f}', 9),
    ] {
        assert_eq!(unicode::combining(character), class);
    }
    assert_eq!(
        unicode::call("combining", &[Value::from("\u{113ce}")])
            .unwrap()
            .as_int()
            .unwrap(),
        9.into()
    );
}

#[test]
fn argument_errors_and_empty_form_shortcut_match_cpython() {
    assert_eq!(unicode::normalize("invalid", "").unwrap(), "");
    assert_eq!(
        unicode::normalize("nfc", "a").unwrap_err().kind,
        "ValueError"
    );
    assert_eq!(
        unicode::call("normalize", &[Value::from(1_i64), Value::from("")])
            .unwrap_err()
            .kind,
        "TypeError"
    );
    assert_eq!(
        unicode::call("normalize", &[Value::from("NFC")])
            .unwrap_err()
            .kind,
        "TypeError"
    );
    for value in [Value::from(""), Value::from("ab"), Value::from(1_i64)] {
        assert_eq!(
            unicode::call("combining", &[value]).unwrap_err().kind,
            "TypeError"
        );
    }
}

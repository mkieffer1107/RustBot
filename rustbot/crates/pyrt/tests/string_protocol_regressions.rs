use pyrt::{
    Value,
    bytes::{self, BytesObject},
    canon_json, formatting,
    object::PyObject,
    runtime::Runtime,
    strings,
};
use serde_json::json;
fn object(value: &Value) -> &dyn PyObject {
    let Value::Object(object) = value else {
        panic!()
    };
    &**object
}

#[test]
fn utf8_encoding_is_bytes_with_python_repr_hash_and_code_unit_iteration() {
    let value = strings::call_method("aé😀", "encode", &[]).unwrap();
    assert_eq!(
        bytes::bytes(&value).unwrap(),
        [97, 195, 169, 240, 159, 152, 128]
    );
    assert_eq!(
        formatting::repr(&value).unwrap(),
        r"b'a\xc3\xa9\xf0\x9f\x98\x80'"
    );
    assert_eq!(object(&value).hash().unwrap(), 5539404024069187928);
    assert_eq!(object(&value).len().unwrap(), 7);
    assert_eq!(
        canon_json(&Value::list(value.sequence().unwrap())).unwrap(),
        json!([97, 195, 169, 240, 159, 152, 128])
    );
    assert_eq!(
        canon_json(&value).unwrap(),
        json!({"$repr":r"b'a\xc3\xa9\xf0\x9f\x98\x80'","$type":"bytes"})
    );
    assert!(
        value
            .equals(&BytesObject::value(vec![97, 195, 169, 240, 159, 152, 128]))
            .unwrap()
    );
    assert_eq!(
        bytes::decode(&bytes::bytes(&value).unwrap(), "UTF_8", "strict").unwrap(),
        "aé😀"
    );
    let mut rt = Runtime::default();
    let iterator = rt.iter(&value).unwrap();
    assert_eq!(
        canon_json(&rt.next(&iterator).unwrap().unwrap()).unwrap(),
        json!(97)
    );
}

#[test]
fn bytes_quoting_indexing_slicing_and_subsequence_membership() {
    assert_eq!(bytes::repr(b"'"), "b\"'\"");
    assert_eq!(bytes::repr(b"\""), "b'\"'");
    assert_eq!(
        bytes::repr(b"'\"\\\n\r\t\x7f\xff"),
        r#"b'\'"\\\n\r\t\x7f\xff'"#
    );
    let value = BytesObject::value((0..8).collect());
    assert_eq!(object(&value).hash().unwrap(), -1525574692105212182);
    assert_eq!(
        canon_json(&object(&value).get_item(&Value::from(-1i64)).unwrap()).unwrap(),
        json!(7)
    );
    assert!(
        object(&value)
            .contains(&BytesObject::value(vec![2, 3]))
            .unwrap()
    );
    assert!(
        object(&value)
            .contains(&BytesObject::value(vec![]))
            .unwrap()
    );
    assert!(
        !object(&value)
            .contains(&BytesObject::value(vec![3, 2]))
            .unwrap()
    );
    assert_eq!(
        object(&value)
            .contains(&Value::from(256i64))
            .unwrap_err()
            .kind,
        "ValueError"
    );
    assert_eq!(
        object(&value)
            .get_item(&Value::Float(1.0))
            .unwrap_err()
            .kind,
        "TypeError"
    );
    assert_eq!(
        bytes::bytes(&bytes::slice(&value, None, Some(6), 1).unwrap()).unwrap(),
        [0, 1, 2, 3, 4, 5]
    );
    assert_eq!(
        bytes::bytes(&bytes::slice(&value, None, None, -2).unwrap()).unwrap(),
        [7, 5, 3, 1]
    );
}

#[test]
fn ascii_latin1_and_utf8_error_policies_follow_python() {
    assert_eq!(
        bytes::bytes(&bytes::encode("éÿ", "latin_1", "strict").unwrap()).unwrap(),
        [233, 255]
    );
    assert_eq!(
        bytes::bytes(&bytes::encode("aé😀b", "ascii", "ignore").unwrap()).unwrap(),
        b"ab"
    );
    assert_eq!(
        bytes::bytes(&bytes::encode("aé😀b", "ascii", "replace").unwrap()).unwrap(),
        b"a??b"
    );
    assert_eq!(
        bytes::bytes(&bytes::encode("aé😀b", "latin1", "replace").unwrap()).unwrap(),
        [97, 233, 63, 98]
    );
    let error = bytes::encode("aé😀b", "ascii", "strict").unwrap_err();
    assert_eq!(error.kind, "UnicodeEncodeError");
    assert_eq!(
        error.message,
        "'ascii' codec can't encode characters in position 1-2: ordinal not in range(128)"
    );
    assert_eq!(
        bytes::encode("😀", "ascii", "strict").unwrap_err().message,
        r"'ascii' codec can't encode character '\U0001f600' in position 0: ordinal not in range(128)"
    );
    assert_eq!(bytes::decode(b"a\xffb", "ascii", "replace").unwrap(), "a�b");
    assert_eq!(bytes::decode(b"a\xffb", "ascii", "ignore").unwrap(), "ab");
    assert_eq!(
        bytes::decode(b"a\xe2\x82", "utf-8", "replace").unwrap(),
        "a�"
    );
    assert_eq!(
        bytes::decode(b"a\xe2\x82", "utf-8", "strict")
            .unwrap_err()
            .message,
        "'utf-8' codec can't decode bytes in position 1-2: unexpected end of data"
    );
    assert_eq!(
        bytes::encode("abc", "nonexistent", "strict")
            .unwrap_err()
            .kind,
        "LookupError"
    );
    // Python looks up the error handler only when an encoding error occurs.
    assert!(bytes::encode("abc", "ascii", "nonexistent").is_ok());
    assert_eq!(
        bytes::encode("é", "ascii", "nonexistent").unwrap_err().kind,
        "LookupError"
    );
}

#[test]
fn maketrans_and_translate_match_normalization_and_deletion_cases() {
    let mapping = Value::dict();
    for (a, b) in [("æ", "ae"), ("ß", "ss"), ("ø", "o"), ("😀", "smile")] {
        mapping.dict_set(Value::from(a), Value::from(b)).unwrap();
    }
    let table = strings::maketrans(&[mapping]).unwrap();
    assert_eq!(
        strings::call_method("smørrebrød æ ß 😀", "translate", &[table])
            .unwrap()
            .as_str()
            .unwrap(),
        "smorrebrod ae ss smile"
    );
    let table =
        strings::maketrans(&[Value::from("ab😀"), Value::from("xyé"), Value::from("b!")]).unwrap();
    assert_eq!(strings::translate("ab😀!z", &table).unwrap(), "xéz");
    assert_eq!(
        canon_json(&table).unwrap(),
        json!({"$dict":[[97,120],[98,null],[128512,233],[33,null]]})
    );
    assert_eq!(
        strings::maketrans(&[Value::from("a"), Value::from("ab")])
            .unwrap_err()
            .kind,
        "ValueError"
    );
    let invalid = Value::dict();
    invalid.dict_set(Value::from("ab"), Value::None).unwrap();
    assert_eq!(
        strings::maketrans(&[invalid]).unwrap_err().kind,
        "ValueError"
    );
    let invalid = Value::dict();
    invalid
        .dict_set(Value::from("a"), Value::list(vec![]))
        .unwrap();
    let invalid = strings::maketrans(&[invalid]).unwrap();
    assert_eq!(
        strings::translate("a", &invalid).unwrap_err().kind,
        "TypeError"
    );
    assert_eq!(strings::translate("z", &invalid).unwrap(), "z");
    let invalid = Value::dict();
    invalid
        .dict_set(Value::from(97i64), Value::from(-1i64))
        .unwrap();
    assert_eq!(
        strings::translate("a", &invalid).unwrap_err().kind,
        "ValueError"
    );
}

#[test]
fn format_map_handles_mapping_keys_nested_specs_and_rejects_positions() {
    let mapping = Value::dict();
    mapping
        .dict_set(Value::from("name"), Value::from("é"))
        .unwrap();
    mapping
        .dict_set(Value::from("value"), Value::Float(1.25))
        .unwrap();
    mapping
        .dict_set(Value::from("width"), Value::from(6i64))
        .unwrap();
    assert_eq!(
        strings::call_method(
            "{name!r}: {value:{width}.2f}",
            "format_map",
            std::slice::from_ref(&mapping)
        )
        .unwrap()
        .as_str()
        .unwrap(),
        "'é':   1.25"
    );
    assert_eq!(strings::format_map("{{0}}", &mapping).unwrap(), "{0}");
    for text in ["{}", "{0}", "{value:{0}}"] {
        let error = strings::format_map(text, &mapping).unwrap_err();
        assert_eq!(error.kind, "ValueError");
        assert_eq!(error.message, "Format string contains positional fields");
    }
    assert_eq!(
        strings::format_map("{missing}", &mapping).unwrap_err().kind,
        "KeyError"
    );
}

use pyrt::{Value, literals};

#[test]
fn nested_literals_create_fresh_mutable_graphs() {
    let source = r#"{"$tuple":[[1,{"$dict":[["$tuple",[2]],["inner",{"$tuple":[3]}]]}]]}"#;
    let first = literals::decode(source).unwrap();
    let second = literals::decode(source).unwrap();
    let outer = first.sequence().unwrap()[0].clone();
    let nested = outer.sequence().unwrap()[1].clone();
    let user_key = nested.dict_get(&Value::from("$tuple")).unwrap();
    let Value::List(values) = user_key else {
        panic!("expected list")
    };
    values.borrow_mut().push(Value::from(9i64));
    let untouched = second.sequence().unwrap()[0].sequence().unwrap()[1]
        .dict_get(&Value::from("$tuple"))
        .unwrap()
        .sequence()
        .unwrap();
    assert_eq!(untouched.len(), 1);
    assert_eq!(untouched[0].as_int().unwrap().to_string(), "2");
}

#[test]
fn dict_keys_collide_as_python_and_retain_first_position() {
    let value = literals::decode(
        r#"{"$dict":[[1,"first"],[true,"second"],[{"$float":"1.0"},"third"],["tail",5]]}"#,
    )
    .unwrap();
    assert_eq!(
        value
            .dict_get(&Value::from(true))
            .unwrap()
            .as_str()
            .unwrap(),
        "third"
    );
    let entries = match value {
        Value::Dict(entries) => entries,
        _ => panic!("expected dict"),
    };
    let entries = entries.borrow();
    assert_eq!(entries.len(), 2);
    assert!(matches!(entries[0].0, Value::Int(_)));
    assert_eq!(entries[1].0.as_str().unwrap(), "tail");
}

#[test]
fn bigint_float_signs_and_unicode_are_preserved() {
    let value = literals::decode(r#"{"$tuple":[123456789012345678901234567890,{"$float":"-0.0"},{"$float":"inf"},{"$float":"nan"},"é😀"]}"#).unwrap().sequence().unwrap();
    assert_eq!(
        value[0].as_int().unwrap().to_string(),
        "123456789012345678901234567890"
    );
    assert_eq!(value[1].as_float().unwrap().to_bits(), (-0.0f64).to_bits());
    assert_eq!(value[2].as_float().unwrap(), f64::INFINITY);
    assert!(value[3].as_float().unwrap().is_nan());
    assert_eq!(value[4].as_str().unwrap(), "é😀");
}

#[test]
fn invalid_or_disallowed_tags_fail_without_partial_values() {
    for input in [
        "{",
        r#"{"$tuple":1}"#,
        r#"{"$dict":[[1]]}"#,
        r#"{"$float":"not-a-float"}"#,
        r#"{"$set":[]}"#,
        r#"[{"$frozenset":[]}]"#,
        r#"{"$repr":"x","$type":"T"}"#,
        r#"{"$date":"2026-01-01"}"#,
        r#"{"other":1}"#,
        r#"{"$tuple":[],"extra":0}"#,
    ] {
        assert!(literals::decode(input).is_err(), "{input}");
    }
    assert_eq!(
        literals::decode(r#"{"$dict":[[[1],2]]}"#).unwrap_err().kind,
        "TypeError"
    );
}

use num_bigint::BigInt;
use pyrt::object::{CallArgs, NativeFunction};
use pyrt::runtime::Runtime;
use pyrt::{Value, canon_json, formatting, stdlib};
use serde_json::json;
use std::cell::RefCell;
use std::rc::Rc;

fn runtime() -> Runtime {
    let mut rt = Runtime::default();
    stdlib::register(&mut rt);
    rt
}

fn call(rt: &mut Runtime, receiver: &Value, method: &str, args: Vec<Value>) -> Value {
    let method = rt.getattr(receiver, method).unwrap();
    rt.call(&method, CallArgs::new(args)).unwrap()
}

#[test]
fn simple_modules_import_once_and_use_shared_runtime_implementations() {
    let mut rt = runtime();
    let math = rt.import("math").unwrap();
    assert!(math.is_identical(&rt.import("math").unwrap()));
    assert!(
        matches!(call(&mut rt, &math, "sqrt", vec![Value::from(9_i64)]), Value::Float(v) if v == 3.0)
    );
    assert!(
        matches!(call(&mut rt, &math, "log10", vec![Value::Int(BigInt::from(10).pow(400))]), Value::Float(v) if v == 400.0)
    );
    let unicode = rt.import("unicodedata").unwrap();
    assert_eq!(
        call(
            &mut rt,
            &unicode,
            "normalize",
            vec![Value::from("NFKC"), Value::from("ﬃ①")]
        )
        .as_str()
        .unwrap(),
        "ffi1"
    );
    let string = rt.import("string").unwrap();
    assert_eq!(
        rt.getattr(&string, "punctuation")
            .unwrap()
            .as_str()
            .unwrap(),
        pyrt::math::PUNCTUATION
    );
    let calendar = rt.import("calendar").unwrap();
    assert_eq!(
        canon_json(&call(
            &mut rt,
            &calendar,
            "monthrange",
            vec![Value::from(2024_i64), Value::from(2_i64)]
        ))
        .unwrap(),
        json!({"$tuple":[3,29]})
    );
    let copy = rt.import("copy").unwrap();
    let child = Value::list(vec![Value::from(1_i64)]);
    let original = Value::list(vec![child.clone(), child]);
    let copied = call(&mut rt, &copy, "deepcopy", vec![original.clone()])
        .sequence()
        .unwrap();
    assert!(copied[0].is_identical(&copied[1]));
    assert!(!copied[0].is_identical(&original.sequence().unwrap()[0]));
}

#[test]
fn random_state_wrapper_preserves_count_provenance_and_canonical_tuple() {
    let mut rt = runtime();
    let random = rt.import("random").unwrap();
    assert!(
        matches!(call(&mut rt, &random, "random", vec![]), Value::Float(x) if x == 0.8444218515250481)
    );
    let state = call(&mut rt, &random, "getstate", vec![]);
    assert_eq!(state.type_name(), "tuple");
    assert_eq!(state.sequence().unwrap().len(), 3);
    assert!(canon_json(&state).unwrap().get("$tuple").is_some());
    let expected = call(&mut rt, &random, "getrandbits", vec![Value::from(65_i64)]);
    assert_eq!(rt.random.snapshot(), json!({"global":5,"instances":[]}));
    call(&mut rt, &random, "setstate", vec![state.clone()]);
    assert_eq!(rt.random.snapshot(), json!({"global":2,"instances":[]}));
    assert!(
        call(&mut rt, &random, "getrandbits", vec![Value::from(65_i64)])
            .equals(&expected)
            .unwrap()
    );
    // Rebuilding the Python tuple from its raw values loses identity provenance.
    let raw = Value::tuple(state.sequence().unwrap());
    call(&mut rt, &random, "setstate", vec![raw]);
    assert_eq!(
        rt.random.snapshot(),
        json!({"global":5,"instances":[],"opaque":true})
    );
}

#[test]
fn random_instances_follow_creation_order_and_private_seeds() {
    let mut rt = runtime();
    let random = rt.import("random").unwrap();
    let class = rt.getattr(&random, "Random").unwrap();
    let first = rt.call(&class, CallArgs::new(vec![])).unwrap();
    let private = rt
        .call(&class, CallArgs::new(vec![Value::from(7_i64)]))
        .unwrap();
    let expected = pyrt::random::PyRandom::from_str("0:0").random();
    assert!(matches!(call(&mut rt, &first, "random", vec![]), Value::Float(x) if x == expected));
    assert_eq!(
        call(&mut rt, &private, "randrange", vec![Value::from(10_i64)])
            .as_int()
            .unwrap(),
        5.into()
    );
    assert_eq!(rt.random.snapshot(), json!({"global":0,"instances":[2]}));
    let pool = Value::list((0..10).map(|n| Value::from(n as i64)).collect());
    let selected = call(&mut rt, &private, "sample", vec![pool, Value::from(3_i64)]);
    assert_eq!(canon_json(&selected).unwrap(), json!([2, 6, 0]));
}

#[test]
fn frozen_dates_datetimes_timezones_and_timedeltas_expose_python_attributes() {
    let mut rt = runtime();
    let datetime = rt.import("datetime").unwrap();
    let date_class = rt.getattr(&datetime, "date").unwrap();
    let today = call(&mut rt, &date_class, "today", vec![]);
    assert_eq!(formatting::str(&today).unwrap(), "2026-06-15");
    assert_eq!(
        formatting::repr(&today).unwrap(),
        "datetime.date(2026, 6, 15)"
    );
    assert_eq!(formatting::format(&today, "%A %B").unwrap(), "Monday June");
    assert_eq!(
        rt.getattr(&today, "day").unwrap().as_int().unwrap(),
        15.into()
    );
    assert_eq!(canon_json(&today).unwrap(), json!({"$date":"2026-06-15"}));
    assert_eq!(
        call(&mut rt, &today, "weekday", vec![]).as_int().unwrap(),
        0.into()
    );
    let delta_class = rt.getattr(&datetime, "timedelta").unwrap();
    let mut args = CallArgs::default();
    args.keyword("hours".into(), Value::Float(25.5)).unwrap();
    let delta = rt.call(&delta_class, args).unwrap();
    assert_eq!(formatting::str(&delta).unwrap(), "1 day, 1:30:00");
    assert_eq!(
        formatting::repr(&delta).unwrap(),
        "datetime.timedelta(days=1, seconds=5400)"
    );
    assert_eq!(
        rt.getattr(&delta, "days").unwrap().as_int().unwrap(),
        1.into()
    );
    let tomorrow = stdlib::temporal_binary("+", &today, &delta)
        .unwrap()
        .unwrap();
    assert_eq!(formatting::str(&tomorrow).unwrap(), "2026-06-16");
    let class = rt.getattr(&datetime, "datetime").unwrap();
    let zone = rt.getattr(&datetime, "timezone").unwrap();
    let utc = rt.getattr(&zone, "utc").unwrap();
    let now = call(&mut rt, &class, "now", vec![utc]);
    assert_eq!(formatting::str(&now).unwrap(), "2026-06-15 12:00:00+00:00");
    assert_eq!(
        formatting::repr(&now).unwrap(),
        "datetime.datetime(2026, 6, 15, 12, 0, tzinfo=datetime.timezone.utc)"
    );
    assert_eq!(
        canon_json(&now).unwrap(),
        json!({"$datetime":"2026-06-15T12:00:00+00:00"})
    );
    assert_eq!(
        formatting::str(&call(&mut rt, &now, "date", vec![])).unwrap(),
        "2026-06-15"
    );
    assert_eq!(
        call(
            &mut rt,
            &now,
            "strftime",
            vec![Value::from("%Y-%m-%d %H:%M %z")]
        )
        .as_str()
        .unwrap(),
        "2026-06-15 12:00 +0000"
    );
}

#[test]
fn patterns_and_matches_retain_group_metadata_and_codepoint_spans() {
    let mut rt = runtime();
    let re = rt.import("re").unwrap();
    let pattern = call(
        &mut rt,
        &re,
        "compile",
        vec![Value::from(r"(?P<word>é+)(x)?")],
    );
    assert_eq!(
        rt.getattr(&pattern, "groups").unwrap().as_int().unwrap(),
        2.into()
    );
    assert_eq!(
        rt.getattr(&pattern, "flags").unwrap().as_int().unwrap(),
        32.into()
    );
    assert_eq!(
        canon_json(&rt.getattr(&pattern, "groupindex").unwrap()).unwrap(),
        json!({"word":1})
    );
    let matched = call(&mut rt, &pattern, "search", vec![Value::from("😀éé!")]);
    assert_eq!(
        call(&mut rt, &matched, "group", vec![]).as_str().unwrap(),
        "éé"
    );
    assert_eq!(
        call(&mut rt, &matched, "group", vec![Value::from("word")])
            .as_str()
            .unwrap(),
        "éé"
    );
    assert_eq!(
        canon_json(&call(&mut rt, &matched, "span", vec![])).unwrap(),
        json!({"$tuple":[1,3]})
    );
    assert_eq!(
        canon_json(&call(&mut rt, &matched, "span", vec![Value::from(2_i64)])).unwrap(),
        json!({"$tuple":[-1,-1]})
    );
    assert_eq!(
        canon_json(&call(
            &mut rt,
            &matched,
            "groups",
            vec![Value::from("missing")]
        ))
        .unwrap(),
        json!({"$tuple":["éé","missing"]})
    );
    assert_eq!(
        call(
            &mut rt,
            &matched,
            "expand",
            vec![Value::from(r"<\g<word>>")]
        )
        .as_str()
        .unwrap(),
        "<éé>"
    );
    let iterator = call(&mut rt, &pattern, "finditer", vec![Value::from("é éé")]);
    assert_eq!(rt.collect(&iterator).unwrap().len(), 2);
}

#[test]
fn callable_regex_replacements_are_lazy_and_respect_count_and_errors() {
    let mut rt = runtime();
    let re = rt.import("re").unwrap();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let recorded = calls.clone();
    let callback = NativeFunction::value("replace", move |rt, args| {
        let method = rt.getattr(&args.positional[0], "group")?;
        let value = rt.call(&method, CallArgs::default())?;
        recorded.borrow_mut().push(value.as_str()?.to_owned());
        Ok(Value::from(value.as_str()?.to_uppercase()))
    });
    let result = call(
        &mut rt,
        &re,
        "subn",
        vec![
            Value::from(r"\w+"),
            callback,
            Value::from("one two three"),
            Value::from(1_i64),
        ],
    );
    assert_eq!(
        canon_json(&result).unwrap(),
        json!({"$tuple":["ONE two three",1]})
    );
    assert_eq!(*calls.borrow(), ["one"]);
    let seen = Rc::new(RefCell::new(0));
    let recorded = seen.clone();
    let callback = NativeFunction::value("fails", move |rt, _| {
        *recorded.borrow_mut() += 1;
        rt.random.global().borrow_mut().random();
        if *recorded.borrow() == 2 {
            return Err(pyrt::PyExc::value_error("callback failure"));
        }
        Ok(Value::from("x"))
    });
    let method = rt.getattr(&re, "sub").unwrap();
    assert_eq!(
        rt.call(
            &method,
            CallArgs::new(vec![
                Value::from(r"\w+"),
                callback,
                Value::from("one two three")
            ])
        )
        .unwrap_err()
        .kind,
        "ValueError"
    );
    assert_eq!(*seen.borrow(), 2);
    assert_eq!(rt.random.snapshot(), json!({"global":4,"instances":[]}));
}

use pyrt::{Value, object::CallArgs, runtime::Runtime};

fn method(rt: &mut Runtime, name: &str, args: Vec<Value>) -> pyrt::PyResult<Value> {
    let re = rt.import("re")?;
    let function = rt.getattr(&re, name)?;
    rt.call(&function, CallArgs::new(args))
}
fn compile(rt: &mut Runtime, pattern: &str) -> Value {
    method(rt, "compile", vec![Value::from(pattern)]).unwrap()
}
fn cap(rt: &mut Runtime, name: &str, value: i64) {
    let re = rt.import("re").unwrap();
    rt.setattr(&re, name, Value::from(value)).unwrap();
}
#[test]
fn raised_capacity_retains_identity_and_purge_releases_python_identity() {
    let mut rt = Runtime::default();
    cap(&mut rt, "_MAXCACHE", 16_384);
    let first = compile(&mut rt, "retained-pattern");
    for n in 0..600 {
        compile(&mut rt, &format!("pattern-{n}"));
    }
    assert!(first.is_identical(&compile(&mut rt, "retained-pattern")));
    method(&mut rt, "purge", vec![]).unwrap();
    assert!(!first.is_identical(&compile(&mut rt, "retained-pattern")));
    assert!(first.is_identical(&method(&mut rt, "compile", vec![first.clone()]).unwrap()));
    assert_eq!(
        method(&mut rt, "compile", vec![first, Value::from(2_i64)])
            .unwrap_err()
            .kind,
        "ValueError"
    );
}
#[test]
fn fifo_hits_do_not_refresh_lru_but_lru_hits_do() {
    let mut rt = Runtime::default();
    cap(&mut rt, "_MAXCACHE", 2);
    cap(&mut rt, "_MAXCACHE2", 2);
    let a = compile(&mut rt, "fifo-a");
    let _b = compile(&mut rt, "fifo-b");
    assert!(a.is_identical(&compile(&mut rt, "fifo-a")));
    compile(&mut rt, "fifo-c");
    assert!(!a.is_identical(&compile(&mut rt, "fifo-a")));
    method(&mut rt, "purge", vec![]).unwrap();
    cap(&mut rt, "_MAXCACHE2", 1);
    let a = compile(&mut rt, "lru-a");
    let b = compile(&mut rt, "lru-b");
    assert!(a.is_identical(&compile(&mut rt, "lru-a")));
    compile(&mut rt, "lru-c");
    assert!(a.is_identical(&compile(&mut rt, "lru-a")));
    assert!(!b.is_identical(&compile(&mut rt, "lru-b")));
}
#[test]
fn runtime_isolation_and_matching_preserve_owner() {
    let mut a = Runtime::default();
    let mut b = Runtime::default();
    let one = compile(&mut a, "[a-z]+");
    let two = compile(&mut b, "[a-z]+");
    assert!(!one.is_identical(&two));
    let matched = method(
        &mut a,
        "search",
        vec![Value::from("[a-z]+"), Value::from("xyz")],
    )
    .unwrap();
    assert!(one.is_identical(&a.getattr(&matched, "re").unwrap()));
    method(&mut a, "purge", vec![]).unwrap();
    assert!(two.is_identical(&compile(&mut b, "[a-z]+")));
    assert!(one.is_identical(&a.getattr(&matched, "re").unwrap()));
}

#[test]
fn changing_capacity_does_not_immediately_trim_existing_entries() {
    let mut rt = Runtime::default();
    cap(&mut rt, "_MAXCACHE", 4);
    cap(&mut rt, "_MAXCACHE2", 1);
    let _a = compile(&mut rt, "shrink-a");
    let b = compile(&mut rt, "shrink-b");
    compile(&mut rt, "shrink-c");
    cap(&mut rt, "_MAXCACHE", 1);
    compile(&mut rt, "shrink-d");
    assert!(b.is_identical(&compile(&mut rt, "shrink-b")));
    method(&mut rt, "purge", vec![]).unwrap();
    cap(&mut rt, "_MAXCACHE", 0);
    cap(&mut rt, "_MAXCACHE2", 0);
    let one = compile(&mut rt, "zero-a");
    assert!(one.is_identical(&compile(&mut rt, "zero-a")));
    compile(&mut rt, "zero-b");
    assert!(!one.is_identical(&compile(&mut rt, "zero-a")));
}

#[test]
fn purge_preserves_old_match_and_bound_pattern_owners() {
    let mut rt = Runtime::default();
    let re = rt.import("re").unwrap();
    assert_eq!(
        rt.getattr(&re, "_MAXCACHE").unwrap().as_int().unwrap(),
        512.into()
    );
    assert_eq!(
        rt.getattr(&re, "_MAXCACHE2").unwrap().as_int().unwrap(),
        256.into()
    );
    let old = compile(&mut rt, "owner-[a-z]+");
    let bound = rt.getattr(&old, "search").unwrap();
    let old_match = rt
        .call(&bound, CallArgs::new(vec![Value::from("owner-yes")]))
        .unwrap();
    method(&mut rt, "purge", vec![]).unwrap();
    let new = compile(&mut rt, "owner-[a-z]+");
    assert!(!old.is_identical(&new));
    let module_match = method(
        &mut rt,
        "search",
        vec![Value::from("owner-[a-z]+"), Value::from("owner-yes")],
    )
    .unwrap();
    let bound_match = rt
        .call(&bound, CallArgs::new(vec![Value::from("owner-yes")]))
        .unwrap();
    assert!(new.is_identical(&rt.getattr(&module_match, "re").unwrap()));
    assert!(old.is_identical(&rt.getattr(&bound_match, "re").unwrap()));
    assert!(old.is_identical(&rt.getattr(&old_match, "re").unwrap()));
}

#[test]
fn nan_cache_limits_are_unordered_without_masking_type_errors() {
    let mut rt = Runtime::default();
    let re = rt.import("re").unwrap();
    rt.setattr(&re, "_MAXCACHE", Value::Float(f64::NAN))
        .unwrap();
    rt.setattr(&re, "_MAXCACHE2", Value::Float(f64::NAN))
        .unwrap();
    let first = compile(&mut rt, "nan-first");
    for n in 0..20 {
        compile(&mut rt, &format!("nan-{n}"));
    }
    assert!(first.is_identical(&compile(&mut rt, "nan-first")));
    rt.setattr(&re, "_MAXCACHE", Value::from("invalid"))
        .unwrap();
    assert_eq!(
        method(&mut rt, "compile", vec![Value::from("new-pattern")])
            .unwrap_err()
            .kind,
        "TypeError"
    );
}

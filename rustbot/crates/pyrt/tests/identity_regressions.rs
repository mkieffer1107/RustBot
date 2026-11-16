use pyrt::object::{CallArgs, NativeFunction};
use pyrt::runtime::Runtime;
use pyrt::{Value, stdlib};
use std::cell::Cell;
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
fn string_aliases_choose_the_actual_normalizer_branches() {
    let word = Value::from("hw".to_owned());
    let opener = Value::from("how".to_owned());
    // normalize._slip chooses short/long from its existing arguments, then
    // returns "missing" if short is word, otherwise "extra".
    let (short, _long) = if word.as_str().unwrap().len() < opener.as_str().unwrap().len() {
        (word.clone(), opener.clone())
    } else {
        (opener.clone(), word.clone())
    };
    assert_eq!(
        if short.is_identical(&word) {
            "missing"
        } else {
            "extra"
        },
        "missing"
    );

    let word = Value::from("gland".to_owned());
    let other = Value::from("glad".to_owned());
    let long = word.clone();
    // normalize._one_letter_apart rejects this consonant insertion only
    // when word is long; losing alias identity incorrectly accepts it.
    let accepted = if word.is_identical(&long) {
        "aeiou".contains(long.as_str().unwrap().chars().nth(3).unwrap())
    } else {
        true
    };
    assert!(!accepted);
    assert!(!word.is_identical(&other));
    let equal_but_distinct = Value::from("gland".to_owned());
    assert!(word.equals(&equal_but_distinct).unwrap());
    assert!(!word.is_identical(&equal_but_distinct));
}

#[test]
fn recipe_kept_sentence_retains_its_input_reference() {
    let bits = Value::list(vec![Value::from(
        "Heat the oil gently and fry the cold rice with chopped vegetables until everything is steaming hot.",
    )]);
    let keep = Value::list(bits.sequence().unwrap());
    let kept = keep.sequence().unwrap();
    let original = bits.sequence().unwrap();
    // cooking._the_card_without_that_sentence permits its one-part result
    // only when keep[0] is bits[0].
    assert!(kept[0].is_identical(&original[0]));
    assert_eq!(
        kept[0].as_str().unwrap(),
        "Heat the oil gently and fry the cold rice with chopped vegetables until everything is steaming hot."
    );
}

#[test]
fn match_and_pattern_attributes_retain_their_original_objects() {
    let mut rt = runtime();
    let re = rt.import("re").unwrap();
    let source = Value::from("identity-[ab]+".to_owned());
    let pattern = call(&mut rt, &re, "compile", vec![source.clone()]);
    let same = call(
        &mut rt,
        &re,
        "compile",
        vec![Value::from("identity-[ab]+".to_owned())],
    );
    assert!(pattern.is_identical(&same));
    assert!(source.is_identical(&rt.getattr(&pattern, "pattern").unwrap()));
    assert!(pattern.is_identical(&call(&mut rt, &re, "compile", vec![pattern.clone()])));
    let text = Value::from("identity-ab".to_owned());
    for method in ["match", "search", "fullmatch"] {
        let matched = call(&mut rt, &pattern, method, vec![text.clone()]);
        assert!(pattern.is_identical(&rt.getattr(&matched, "re").unwrap()));
        assert!(text.is_identical(&rt.getattr(&matched, "string").unwrap()));
    }
    let iterator = call(&mut rt, &pattern, "finditer", vec![text]);
    let matched = rt.next(&iterator).unwrap().unwrap();
    let bound = rt.getattr(&pattern, "search").unwrap();
    drop(same);
    drop(pattern);
    let retained = rt.getattr(&matched, "re").unwrap();
    let matched_again = rt
        .call(&bound, CallArgs::new(vec![Value::from("identity-b")]))
        .unwrap();
    assert!(retained.is_identical(&rt.getattr(&matched_again, "re").unwrap()));
}

#[test]
fn verdict_noop_strip_and_sub_preserve_sentence_identity() {
    let mut rt = runtime();
    let re = rt.import("re").unwrap();
    let pattern = call(&mut rt, &re, "compile", vec![Value::from(r"(?i)^then\s+")]);
    let sentence = Value::from("start with the smallest version of it".to_owned());
    let stripped = call(&mut rt, &sentence, "strip", vec![]);
    assert!(stripped.is_identical(&sentence));
    let opened = call(&mut rt, &pattern, "sub", vec![Value::from(""), stripped]);
    assert!(opened.is_identical(&sentence));
    for method in ["strip", "lstrip", "rstrip"] {
        assert!(call(&mut rt, &sentence, method, vec![Value::None]).is_identical(&sentence));
        assert!(call(&mut rt, &sentence, method, vec![Value::from("")]).is_identical(&sentence));
    }
    let padded = Value::from(" start with the smallest version of it ");
    let trimmed = call(&mut rt, &padded, "strip", vec![]);
    assert!(!trimmed.is_identical(&padded));
    assert_eq!(trimmed.as_str().unwrap(), sentence.as_str().unwrap());
    let unchanged = call(
        &mut rt,
        &re,
        "sub",
        vec![pattern.clone(), Value::from(""), sentence.clone()],
    );
    assert!(unchanged.is_identical(&sentence));
    let subn = call(
        &mut rt,
        &pattern,
        "subn",
        vec![Value::from(""), sentence.clone()],
    )
    .sequence()
    .unwrap();
    assert!(subn[0].is_identical(&sentence));
    assert_eq!(subn[1].as_int().unwrap().to_string(), "0");
    let then = Value::from("Then start with the smallest version of it");
    let changed = call(
        &mut rt,
        &pattern,
        "sub",
        vec![Value::from(""), then.clone()],
    );
    assert!(!changed.is_identical(&then));
    assert_eq!(changed.as_str().unwrap(), sentence.as_str().unwrap());
}

#[test]
fn replacement_callbacks_receive_original_pattern_and_subject() {
    let mut rt = runtime();
    let re = rt.import("re").unwrap();
    let pattern = call(
        &mut rt,
        &re,
        "compile",
        vec![Value::from("callback-identity")],
    );
    let text = Value::from("callback-identity".to_owned());
    let expected_pattern = pattern.clone();
    let expected_text = text.clone();
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let callback = NativeFunction::value("replace", move |rt, args| {
        let matched = &args.positional[0];
        assert!(expected_pattern.is_identical(&rt.getattr(matched, "re")?));
        assert!(expected_text.is_identical(&rt.getattr(matched, "string")?));
        observed.set(observed.get() + 1);
        Ok(Value::from("replacement"))
    });
    assert_eq!(
        call(&mut rt, &pattern, "sub", vec![callback.clone(), text])
            .as_str()
            .unwrap(),
        "replacement"
    );
    let absent = Value::from("no occurrence here".to_owned());
    assert!(call(&mut rt, &pattern, "sub", vec![callback, absent.clone()]).is_identical(&absent));
    assert_eq!(calls.get(), 1);
}

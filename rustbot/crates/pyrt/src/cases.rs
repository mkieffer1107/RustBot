//! Executes input-only runtime probes using the same runtime as translated code.
use crate::{
    PyExc, PyResult, Value, canon_json, from_canon_json,
    random::{PyRandom, RandomState},
};
use num_traits::ToPrimitive;
use serde_json::{Value as Json, json};

fn field<'a>(case: &'a Json, name: &str) -> PyResult<&'a Json> {
    case.get(name)
        .ok_or_else(|| PyExc::key_error(format!("missing case field: {name}")))
}

fn args(case: &Json) -> PyResult<Vec<Value>> {
    from_canon_json(field(case, "args")?)?.sequence()
}

pub fn run_case(kind: &str, case: &Json) -> Json {
    let result = if kind == "re" {
        crate::regex::run_case(case)
    } else {
        match kind {
            "str" => (|| {
                let s = field(case, "self")?
                    .as_str()
                    .ok_or_else(|| PyExc::type_error("expected str"))?;
                let method = field(case, "method")?
                    .as_str()
                    .ok_or_else(|| PyExc::type_error("expected method name"))?;
                crate::strings::call_method(s, method, &args(case)?)
            })(),
            "random" => run_random(case),
            "format" => crate::formatting::run_case(case),
            _ => Err(PyExc::not_implemented_error(format!(
                "runtime case kind {kind} is not implemented yet"
            ))),
        }
        .and_then(|value| canon_json(&value))
    };
    match result {
        Ok(value) => json!({"result": value, "exc": null}),
        Err(exc) => json!({"result": null, "exc": exc.kind}),
    }
}

fn arity(args: &[Value], min: usize, max: usize) -> PyResult<()> {
    if (min..=max).contains(&args.len()) {
        Ok(())
    } else {
        Err(PyExc::type_error("incorrect number of arguments"))
    }
}

fn signed_arg(v: &Value) -> PyResult<i64> {
    v.as_int()?
        .to_i64()
        .ok_or_else(|| PyExc::overflow_error("Python int too large to convert to C ssize_t"))
}

fn seed_random(value: &Value) -> PyResult<PyRandom> {
    match value {
        Value::Int(_) | Value::Bool(_) => Ok(PyRandom::from_int(&value.as_int()?)),
        Value::Str(s) => Ok(PyRandom::from_str(s.as_str())),
        Value::Float(x) => PyRandom::from_float(*x),
        Value::None => Err(PyExc::not_implemented_error(
            "entropy seeds cannot be replayed deterministically",
        )),
        _ => Err(PyExc::type_error("unsupported random seed type")),
    }
}

fn run_random(case: &Json) -> PyResult<Value> {
    let mut random = seed_random(&from_canon_json(field(case, "seed")?)?)?;
    if let Some(ops) = case.get("ops") {
        let ops = ops
            .as_array()
            .ok_or_else(|| PyExc::type_error("expected operations list"))?;
        let mut out = Vec::with_capacity(ops.len());
        for op in ops {
            let pair = op
                .as_array()
                .filter(|a| a.len() == 2)
                .ok_or_else(|| PyExc::value_error("invalid random operation"))?;
            let name = pair[0]
                .as_str()
                .ok_or_else(|| PyExc::type_error("expected operation name"))?;
            let args = from_canon_json(&pair[1])?.sequence()?;
            out.push(random_op(&mut random, name, &args)?);
        }
        Ok(Value::list(out))
    } else {
        let name = field(case, "op")?
            .as_str()
            .ok_or_else(|| PyExc::type_error("expected operation name"))?;
        random_op(&mut random, name, &args(case)?)
    }
}

fn state_value(state: RandomState) -> Value {
    let mut words: Vec<_> = state
        .words
        .into_iter()
        .map(|n| Value::from(n as i64))
        .collect();
    words.push(Value::from(state.index));
    Value::tuple(vec![
        Value::from(state.version),
        Value::tuple(words),
        state.gauss_next.map(Value::Float).unwrap_or(Value::None),
    ])
}

fn random_op(random: &mut PyRandom, name: &str, args: &[Value]) -> PyResult<Value> {
    match name {
        "random" => {
            arity(args, 0, 0)?;
            Ok(Value::Float(random.random()))
        }
        "seed" => {
            arity(args, 1, 1)?;
            *random = seed_random(&args[0])?;
            Ok(Value::None)
        }
        "getrandbits" => {
            arity(args, 1, 1)?;
            let n = args[0].as_int()?;
            let bits = n.to_u64().ok_or_else(|| {
                if n < 0.into() {
                    PyExc::value_error("Cannot convert negative int")
                } else {
                    PyExc::overflow_error("int too big to convert")
                }
            })?;
            Ok(Value::Int(random.getrandbits_u64(bits)?))
        }
        "randrange" => {
            arity(args, 1, 3)?;
            let start = args[0].as_int()?;
            let stop = args
                .get(1)
                .filter(|v| !matches!(v, Value::None))
                .map(Value::as_int)
                .transpose()?;
            let step = args.get(2).map(Value::as_int).transpose()?;
            Ok(Value::Int(random.randrange(
                &start,
                stop.as_ref(),
                step.as_ref(),
            )?))
        }
        "randint" => {
            arity(args, 2, 2)?;
            Ok(Value::Int(
                random.randint(&args[0].as_int()?, &args[1].as_int()?)?,
            ))
        }
        "choice" => {
            arity(args, 1, 1)?;
            let sequence = args[0].sequence()?;
            Ok(sequence[random.choice_index(sequence.len())?].clone())
        }
        "shuffle" => {
            arity(args, 1, 1)?;
            if let Value::List(values) = &args[0] {
                random.shuffle(&mut values.borrow_mut());
                Ok(args[0].clone()) // probe protocol reports the mutated list
            } else {
                Err(PyExc::type_error("object does not support item assignment"))
            }
        }
        "sample" => {
            arity(args, 2, 2)?;
            if !matches!(&args[0], Value::List(_) | Value::Tuple(_) | Value::Str(_)) {
                return Err(PyExc::type_error("Population must be a sequence"));
            }
            let population = args[0].sequence()?;
            let k = signed_arg(&args[1])?;
            if k < 0 {
                return Err(PyExc::value_error(
                    "Sample larger than population or is negative",
                ));
            }
            Ok(Value::list(
                random
                    .sample_indices(population.len(), k as usize)?
                    .into_iter()
                    .map(|i| population[i].clone())
                    .collect(),
            ))
        }
        "choices" => {
            arity(args, 1, 2)?;
            let population = args[0].sequence()?;
            let weights = args
                .get(1)
                .filter(|v| !matches!(v, Value::None))
                .map(|v| {
                    v.sequence()?
                        .iter()
                        .map(Value::as_float)
                        .collect::<PyResult<Vec<_>>>()
                })
                .transpose()?;
            let indices = random.choices_indices(population.len(), weights.as_deref(), None, 1)?;
            Ok(Value::list(
                indices.into_iter().map(|i| population[i].clone()).collect(),
            ))
        }
        "getstate" => {
            arity(args, 0, 0)?;
            Ok(state_value(random.getstate()))
        }
        "setstate" => {
            arity(args, 1, 1)?;
            let state = args[0].sequence()?;
            if state.len() != 3 {
                return Err(PyExc::value_error("invalid state"));
            }
            let words = state[1].sequence()?;
            if words.len() != 625 {
                return Err(PyExc::value_error("state vector is the wrong size"));
            }
            let version = signed_arg(&state[0])?;
            let mut mt = [0_u32; 624];
            for (out, v) in mt.iter_mut().zip(&words) {
                let n = v.as_int()?;
                *out = if version == 2 {
                    ((n % (num_bigint::BigInt::from(1_u64) << 32_u32))
                        + (num_bigint::BigInt::from(1_u64) << 32_u32))
                        .to_u32_digits()
                        .1
                        .first()
                        .copied()
                        .unwrap_or(0)
                } else {
                    n.to_u32()
                        .ok_or_else(|| PyExc::overflow_error("state word outside unsigned range"))?
                };
            }
            let index = words[624]
                .as_int()?
                .to_usize()
                .ok_or_else(|| PyExc::value_error("invalid state"))?;
            let gauss_next = if matches!(state[2], Value::None) {
                None
            } else {
                Some(state[2].as_float()?)
            };
            random.setstate(&RandomState {
                version,
                words: mt,
                index,
                gauss_next,
            })?;
            Ok(Value::None)
        }
        _ => Err(PyExc::attribute_error(format!(
            "Random has no method {name}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runner_canonicalizes_results_and_exceptions() {
        assert_eq!(
            run_case("random", &json!({"seed":0,"op":"random","args":[]})),
            json!({"result":{"$float":"0.8444218515250481"},"exc":null})
        );
        assert_eq!(
            run_case("random", &json!({"seed":0,"op":"choice","args":[[]]})),
            json!({"result":null,"exc":"IndexError"})
        );
        assert_eq!(
            run_case("str", &json!({"self":"é😀x","method":"find","args":["x"]})),
            json!({"result":2,"exc":null})
        );
    }
}

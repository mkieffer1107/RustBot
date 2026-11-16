//! Per-conversation random state, including the Space's unseeded-instance scheme.
use crate::{PyExc, PyResult, random::PyRandom};
use num_bigint::BigInt;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;

pub type RandomHandle = Rc<RefCell<PyRandom>>;

pub struct RandomContext {
    seed: BigInt,
    global: RandomHandle,
    instances: Vec<RandomHandle>,
}

impl RandomContext {
    pub fn new(seed: BigInt) -> Self {
        Self {
            global: Rc::new(RefCell::new(PyRandom::from_int(&seed))),
            seed,
            instances: Vec::new(),
        }
    }

    pub fn global(&self) -> RandomHandle {
        self.global.clone()
    }

    /// Explicitly seeded instances use PyRandom constructors directly. Only
    /// Random() / Random(None) advance this creation-order counter.
    pub fn new_unseeded(&mut self) -> RandomHandle {
        let seed = format!("{}:{}", self.seed, self.instances.len());
        let random = Rc::new(RefCell::new(PyRandom::from_str(&seed)));
        self.instances.push(random.clone());
        random
    }

    pub fn instance(&self, index: usize) -> PyResult<RandomHandle> {
        self.instances
            .get(index)
            .cloned()
            .ok_or_else(|| PyExc::index_error("random instance does not exist"))
    }

    pub fn snapshot(&self) -> Value {
        let global = self.global.borrow();
        let mut opaque = global.state_is_opaque();
        let counts: Vec<_> = self
            .instances
            .iter()
            .map(|v| {
                let v = v.borrow();
                opaque |= v.state_is_opaque();
                v.words_consumed()
            })
            .collect();
        let mut state = json!({"global":global.words_consumed(), "instances":counts});
        if opaque {
            state["opaque"] = Value::Bool(true);
        }
        state
    }

    /// Restore a count snapshot by reseeding and advancing while retaining
    /// existing handles captured by module globals and closures.
    pub fn restore(&mut self, snapshot: &Value) -> PyResult<()> {
        if snapshot
            .get("opaque")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(PyExc::not_implemented_error(
                "opaque RNG state requires full getstate data",
            ));
        }
        let global_count = snapshot
            .get("global")
            .and_then(Value::as_u64)
            .ok_or_else(|| PyExc::value_error("invalid global RNG count"))?;
        let counts = snapshot
            .get("instances")
            .and_then(Value::as_array)
            .ok_or_else(|| PyExc::value_error("invalid RNG instances"))?;
        let counts: Vec<_> = counts
            .iter()
            .map(|v| {
                v.as_u64()
                    .ok_or_else(|| PyExc::value_error("invalid instance RNG count"))
            })
            .collect::<PyResult<_>>()?;
        if counts.len() < self.instances.len() {
            return Err(PyExc::value_error(
                "snapshot predates existing random instances; load a fresh conversation",
            ));
        }
        let mut global = PyRandom::from_int(&self.seed);
        global.advance(global_count);
        *self.global.borrow_mut() = global;
        while self.instances.len() < counts.len() {
            self.new_unseeded();
        }
        for (i, count) in counts.into_iter().enumerate() {
            let mut random = PyRandom::from_str(&format!("{}:{i}", self.seed));
            random.advance(count);
            *self.instances[i].borrow_mut() = random;
        }
        Ok(())
    }

    /// The Space resets only its global stream. Fresh native conversations
    /// construct a fresh context instead.
    pub fn reset_global(&mut self) {
        *self.global.borrow_mut() = PyRandom::from_int(&self.seed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_order_matches_cpython_space_seeds() {
        let mut context = RandomContext::new(0.into());
        let first = context.new_unseeded();
        let second = context.new_unseeded();
        assert_eq!(
            first.borrow_mut().random(),
            PyRandom::from_str("0:0").random()
        );
        assert_eq!(
            second.borrow_mut().random(),
            PyRandom::from_str("0:1").random()
        );
        assert_eq!(context.global().borrow_mut().random(), 0.8444218515250481);
        assert_eq!(context.snapshot(), json!({"global":2,"instances":[2,2]}));
    }

    #[test]
    fn restore_reuses_handles_and_global_reset_preserves_instances() {
        let mut context = RandomContext::new(7.into());
        let handle = context.new_unseeded();
        handle.borrow_mut().getrandbits(33).unwrap();
        let snapshot = context.snapshot();
        let expected = handle.borrow_mut().random();
        context.restore(&snapshot).unwrap();
        assert!(Rc::ptr_eq(&handle, &context.instance(0).unwrap()));
        assert_eq!(handle.borrow_mut().random(), expected);
        let count = context.snapshot()["instances"][0].clone();
        context.global().borrow_mut().random();
        context.reset_global();
        assert_eq!(context.snapshot(), json!({"global":0,"instances":[count]}));
        assert_eq!(
            context
                .restore(&json!({"global":0,"instances":[],"opaque":true}))
                .unwrap_err()
                .kind,
            "NotImplementedError"
        );
    }
}

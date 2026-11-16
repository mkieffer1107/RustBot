//! Comprehension loops compiled to native Rust callbacks, with lazy iteration.
//!
//! The first iterable belongs to the enclosing scope and is evaluated when the
//! generator is created. Targets, subsequent iterables, filters and emission
//! run in a separate lexical frame. Walrus stores in generated callbacks must
//! explicitly target their captured enclosing frame, per Python's scope rules.
use crate::{PyExc, PyResult, Value, frame::Frame, object::IteratorObject, runtime::Runtime};
use std::rc::Rc;

pub type Expr = Rc<dyn Fn(&mut Runtime, &Rc<Frame>) -> PyResult<Value>>;
pub type Assign = Rc<dyn Fn(&mut Runtime, &Rc<Frame>, Value) -> PyResult<()>>;
pub type Filter = Rc<dyn Fn(&mut Runtime, &Rc<Frame>) -> PyResult<bool>>;
pub type Resume = Rc<dyn Fn(&mut Runtime) -> PyResult<()>>;

#[derive(Clone)]
pub struct Level {
    pub iterable: Expr,
    pub assign: Assign,
    pub filters: Vec<Filter>,
}

struct State {
    frame: Rc<Frame>,
    levels: Vec<Level>,
    emit: Expr,
    iterators: Vec<Option<Value>>,
    depth: usize,
    closed: bool,
}

impl State {
    fn advance(&mut self, rt: &mut Runtime) -> PyResult<Option<Value>> {
        if self.closed {
            return Ok(None);
        }
        loop {
            let iterator = self.iterators[self.depth]
                .as_ref()
                .expect("active comprehension level");
            let next = match rt.next(iterator) {
                Err(error) if error.kind == "StopIteration" => None,
                result => result?,
            };
            let Some(value) = next else {
                self.iterators[self.depth] = None;
                if self.depth == 0 {
                    return Ok(None);
                }
                self.depth -= 1;
                continue;
            };
            let level = &self.levels[self.depth];
            (level.assign)(rt, &self.frame, value)?;
            let mut passes = true;
            for filter in &level.filters {
                if !filter(rt, &self.frame)? {
                    passes = false;
                    break;
                }
            }
            if !passes {
                continue;
            }
            if self.depth + 1 == self.levels.len() {
                return (self.emit)(rt, &self.frame).map(Some);
            }
            self.depth += 1;
            let iterable = (self.levels[self.depth].iterable)(rt, &self.frame)?;
            self.iterators[self.depth] = Some(rt.iter(&iterable)?);
        }
    }
    fn next(&mut self, rt: &mut Runtime) -> PyResult<Option<Value>> {
        let result = self.advance(rt);
        if !matches!(result, Ok(Some(_))) {
            self.closed = true;
            self.iterators.clear();
        }
        // StopIteration raised inside a generator body is transformed by PEP479;
        // normal exhaustion from a for iterator was consumed in advance().
        match result {
            Err(error) if error.kind == "StopIteration" => {
                Err(PyExc::runtime_error("generator raised StopIteration"))
            }
            other => other,
        }
    }
}

fn state(
    rt: &mut Runtime,
    parent: Rc<Frame>,
    declared: &[&str],
    levels: Vec<Level>,
    emit: Expr,
) -> PyResult<State> {
    let first = levels
        .first()
        .ok_or_else(|| PyExc::value_error("comprehension requires at least one iterable"))?;
    let outer_iterable = (first.iterable)(rt, &parent)?;
    let outer_iterator = rt.iter(&outer_iterable)?;
    let frame = Frame::function(parent, declared, &[], &[], Value::dict());
    let mut iterators = vec![None; levels.len()];
    iterators[0] = Some(outer_iterator);
    Ok(State {
        frame,
        levels,
        emit,
        iterators,
        depth: 0,
        closed: false,
    })
}

pub fn generator(
    rt: &mut Runtime,
    parent: Rc<Frame>,
    declared: &[&str],
    levels: Vec<Level>,
    emit: Expr,
) -> PyResult<Value> {
    generator_traced(rt, parent, declared, levels, emit, Rc::new(|_| Ok(())))
}

/// Generated genexpr code attaches its Python source span to each resume,
/// including the final resume that produces no value. Creation evaluates only
/// the outer iterable and does not execute this hook.
pub fn generator_traced(
    rt: &mut Runtime,
    parent: Rc<Frame>,
    declared: &[&str],
    levels: Vec<Level>,
    emit: Expr,
    on_resume: Resume,
) -> PyResult<Value> {
    let mut state = state(rt, parent, declared, levels, emit)?;
    Ok(IteratorObject::value(move |rt| {
        if state.closed {
            return Ok(None);
        }
        if let Err(error) = on_resume(rt) {
            state.closed = true;
            state.iterators.clear();
            return Err(error);
        }
        state.next(rt)
    }))
}

/// Eager list/set/dict comprehension elements. Unlike a generator expression,
/// their callbacks propagate StopIteration unchanged; using Runtime::collect
/// on generator() would apply PEP479 and change Python's exception behavior.
pub fn collect(
    rt: &mut Runtime,
    parent: Rc<Frame>,
    declared: &[&str],
    levels: Vec<Level>,
    emit: Expr,
) -> PyResult<Vec<Value>> {
    let mut state = state(rt, parent, declared, levels, emit)?;
    let mut values = Vec::new();
    while let Some(value) = state.advance(rt)? {
        values.push(value);
    }
    Ok(values)
}

/// Insert immediately so hashing errors stop before later callbacks run.
pub fn collect_set(
    rt: &mut Runtime,
    parent: Rc<Frame>,
    declared: &[&str],
    levels: Vec<Level>,
    emit: Expr,
) -> PyResult<Value> {
    let mut state = state(rt, parent, declared, levels, emit)?;
    let set = crate::sets::PySet::new();
    while let Some(value) = state.advance(rt)? {
        set.insert(value)?;
    }
    Ok(Value::Set(set))
}

/// The compiler emits each key/value pair as a two-element tuple, evaluating
/// the key before its value. Dictionary insertion happens before the next turn.
pub fn collect_dict(
    rt: &mut Runtime,
    parent: Rc<Frame>,
    declared: &[&str],
    levels: Vec<Level>,
    emit: Expr,
) -> PyResult<Value> {
    let mut state = state(rt, parent, declared, levels, emit)?;
    let dict = Value::dict();
    while let Some(value) = state.advance(rt)? {
        let Value::Tuple(pair) = value else {
            return Err(PyExc::type_error(
                "dict comprehension emitter must return a key/value tuple",
            ));
        };
        if pair.len() != 2 {
            return Err(PyExc::value_error(
                "dict comprehension emitter must return two values",
            ));
        }
        dict.dict_set(pair[0].clone(), pair[1].clone())?;
    }
    Ok(dict)
}

//! Native adapters around generated modules; Python logic stays in its mirrors.
mod generated;
use num_bigint::BigInt;
use pyrt::{PyExc, PyResult, Value, object::CallArgs, runtime::Runtime};

pub fn new_runtime(seed: BigInt, clock: &str) -> PyResult<Runtime> {
    let mut runtime = Runtime::new(seed, clock)?;
    generated::register(&mut runtime);
    runtime.import("bot")?;
    Ok(runtime)
}

pub fn call(runtime: &mut Runtime, name: &str, args: CallArgs) -> PyResult<Value> {
    let function = resolve(runtime, name)?;
    runtime.call(&function, args)
}

pub fn resolve(runtime: &mut Runtime, name: &str) -> PyResult<Value> {
    let (module, attribute) = name
        .rsplit_once('.')
        .ok_or_else(|| PyExc::value_error("expected a dotted function name"))?;
    let module = runtime.import(module)?;
    runtime.getattr(&module, attribute)
}

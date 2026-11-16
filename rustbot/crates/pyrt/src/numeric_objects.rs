//! Exact rational and finite decimal objects used by the translated maths code.
use crate::{
    PyExc, PyResult, Value,
    object::{CallArgs, NativeFunction, NativeType, PyObject},
    runtime::Runtime,
};
use num_bigint::BigInt;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::{any::Any, cmp::Ordering, rc::Rc};

fn normalize_decimal_digits(text: &str) -> String {
    text.chars()
        .map(|c| {
            if !crate::unicode_data::property(c, 1 << 2) {
                return c;
            }
            let code = c as u32;
            let mut first = code;
            while first > 0
                && char::from_u32(first - 1)
                    .is_some_and(|x| crate::unicode_data::property(x, 1 << 2))
            {
                first -= 1;
            }
            char::from(b'0' + ((code - first) % 10) as u8)
        })
        .collect()
}
fn gcd(mut a: BigInt, mut b: BigInt) -> BigInt {
    a = a.abs();
    b = b.abs();
    while !b.is_zero() {
        let r = &a % &b;
        a = b;
        b = r;
    }
    a
}
fn floor_div(a: &BigInt, b: &BigInt) -> BigInt {
    let q = a / b;
    let r = a % b;
    if !r.is_zero() && a.is_negative() != b.is_negative() {
        q - 1
    } else {
        q
    }
}
fn pow10(n: u32) -> PyResult<BigInt> {
    if n > 1_000_000 {
        return Err(PyExc::overflow_error("decimal exponent too large"));
    }
    Ok(BigInt::from(10u8).pow(n))
}
fn invalid_decimal() -> PyExc {
    PyExc {
        kind: "InvalidOperation",
        message: "[<class 'decimal.InvalidOperation'>]".into(),
    }
}
fn conversion_syntax() -> PyExc {
    PyExc {
        kind: "InvalidOperation",
        message: "[<class 'decimal.ConversionSyntax'>]".into(),
    }
}
fn integer(value: &Value) -> PyResult<BigInt> {
    value.as_int()
}
fn obj<T: 'static>(v: &Value) -> Option<&T> {
    if let Value::Object(o) = v {
        o.as_any().downcast_ref()
    } else {
        None
    }
}
fn frac_value(x: Fraction) -> Value {
    Value::Object(Rc::new(x))
}
fn decimal_value(x: Decimal) -> Value {
    Value::Object(Rc::new(x))
}
fn numeric_ratio(v: &Value) -> PyResult<Option<Fraction>> {
    if let Some(f) = obj::<Fraction>(v) {
        return Ok(Some(f.clone()));
    }
    if let Some(d) = obj::<Decimal>(v) {
        return Ok(Some(d.ratio()?));
    }
    match v {
        Value::Int(n) => Ok(Some(Fraction::new(n.clone(), BigInt::one())?)),
        Value::Bool(b) => Ok(Some(Fraction::new(BigInt::from(*b), BigInt::one())?)),
        Value::Float(f) if f.is_finite() => Ok(Some(Fraction::from_float(*f)?)),
        _ => Ok(None),
    }
}
fn mod_pow(mut base: u64, mut exp: u64, modulus: u64) -> u64 {
    let mut result = 1u64;
    while exp > 0 {
        if exp & 1 != 0 {
            result = ((result as u128 * base as u128) % modulus as u128) as u64;
        }
        base = ((base as u128 * base as u128) % modulus as u128) as u64;
        exp >>= 1;
    }
    result
}
fn rational_hash(n: &BigInt, d: &BigInt) -> i64 {
    const P: u64 = (1u64 << 61) - 1;
    let nr = (n.abs() % P).to_u64().unwrap();
    let dr = (d % P).to_u64().unwrap();
    let h = if dr == 0 {
        314159
    } else {
        ((nr as u128 * mod_pow(dr, P - 2, P) as u128) % P as u128) as i64
    };
    let h = if n.is_negative() { -h } else { h };
    if h == -1 { -2 } else { h }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fraction {
    numerator: BigInt,
    denominator: BigInt,
}
impl Fraction {
    fn new(mut n: BigInt, mut d: BigInt) -> PyResult<Self> {
        if d.is_zero() {
            return Err(PyExc::zero_division_error("Fraction denominator is zero"));
        }
        if d.is_negative() {
            n = -n;
            d = -d;
        }
        let g = gcd(n.clone(), d.clone());
        Ok(Self {
            numerator: n / &g,
            denominator: d / g,
        })
    }
    fn from_float(v: f64) -> PyResult<Self> {
        if v.is_nan() {
            return Err(PyExc::value_error("cannot convert NaN to integer ratio"));
        }
        if v.is_infinite() {
            return Err(PyExc::overflow_error(
                "cannot convert Infinity to integer ratio",
            ));
        }
        let bits = v.to_bits();
        let exp = ((bits >> 52) & 0x7ff) as i32;
        let mantissa = bits & ((1u64 << 52) - 1);
        let mut n = BigInt::from(if exp == 0 {
            mantissa
        } else {
            mantissa | (1u64 << 52)
        });
        if bits >> 63 != 0 {
            n = -n;
        }
        let power = if exp == 0 { -1074 } else { exp - 1023 - 52 };
        if power >= 0 {
            Self::new(n << (power as usize), BigInt::one())
        } else {
            Self::new(n, BigInt::one() << ((-power) as usize))
        }
    }
    fn from_str(text: &str) -> PyResult<Self> {
        let normalized = normalize_decimal_digits(text);
        let text = normalized.trim();
        let bytes = text.as_bytes();
        if bytes.iter().enumerate().any(|(i, b)| {
            *b == b'_'
                && (i == 0
                    || i + 1 == bytes.len()
                    || !bytes[i - 1].is_ascii_digit()
                    || !bytes[i + 1].is_ascii_digit())
        }) {
            return Err(PyExc::value_error("invalid Fraction"));
        }
        if let Some((a, b)) = text.split_once('/') {
            if b.contains('/') {
                return Err(PyExc::value_error("invalid Fraction"));
            }
            let a = parse_signed_int(a.trim())?;
            let denominator = b.trim();
            if denominator.starts_with(['+', '-']) {
                return Err(PyExc::value_error("invalid Fraction"));
            }
            let b = parse_signed_int(denominator)?;
            return Self::new(a, b);
        }
        Decimal::parse(text)
            .map_err(|_| PyExc::value_error("invalid Fraction"))?
            .ratio()
    }
    fn limit(&self, max: BigInt) -> PyResult<Self> {
        if max < BigInt::one() {
            return Err(PyExc::value_error("max_denominator should be at least 1"));
        }
        if self.denominator <= max {
            return Ok(self.clone());
        }
        let (mut p0, mut q0, mut p1, mut q1) =
            (BigInt::zero(), BigInt::one(), BigInt::one(), BigInt::zero());
        let (mut n, mut d) = (self.numerator.clone(), self.denominator.clone());
        loop {
            let a = floor_div(&n, &d);
            let q2 = &q0 + &a * &q1;
            if q2 > max {
                break;
            }
            let p2 = &p0 + &a * &p1;
            p0 = p1;
            q0 = q1;
            p1 = p2;
            q1 = q2;
            let r = &n - &a * &d;
            n = d;
            d = r;
        }
        let k = (&max - &q0) / &q1;
        let a = Self::new(&p0 + &k * &p1, &q0 + &k * &q1)?;
        let b = Self::new(p1, q1)?;
        let da = (&self.numerator * &a.denominator - &a.numerator * &self.denominator).abs()
            * &b.denominator;
        let db = (&self.numerator * &b.denominator - &b.numerator * &self.denominator).abs()
            * &a.denominator;
        Ok(if db <= da { b } else { a })
    }
    fn text(&self) -> String {
        if self.denominator.is_one() {
            self.numerator.to_string()
        } else {
            format!("{}/{}", self.numerator, self.denominator)
        }
    }
}
fn parse_signed_int(s: &str) -> PyResult<BigInt> {
    let s = s.replace('_', "");
    if s.is_empty() {
        return Err(PyExc::value_error("invalid integer"));
    }
    s.parse().map_err(|_| PyExc::value_error("invalid integer"))
}
impl PyObject for Fraction {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "Fraction"
    }
    fn str(&self) -> PyResult<String> {
        Ok(self.text())
    }
    fn repr(&self) -> PyResult<String> {
        Ok(format!(
            "Fraction({}, {})",
            self.numerator, self.denominator
        ))
    }
    fn is_truthy(&self) -> bool {
        !self.numerator.is_zero()
    }
    fn hash(&self) -> PyResult<i64> {
        Ok(rational_hash(&self.numerator, &self.denominator))
    }
    fn equals(&self, other: &Value) -> PyResult<bool> {
        if let Some(other) = numeric_ratio(other)? {
            Ok(&self.numerator * other.denominator == other.numerator * &self.denominator)
        } else {
            Ok(false)
        }
    }
    fn get_attr(&self, _: &mut Runtime, name: &str) -> PyResult<Value> {
        match name {
            "numerator" => Ok(Value::Int(self.numerator.clone())),
            "denominator" => Ok(Value::Int(self.denominator.clone())),
            "limit_denominator" => {
                let this = self.clone();
                Ok(NativeFunction::value(
                    "limit_denominator",
                    move |_, args| {
                        args.arity(0, 1)?;
                        let max = if let Some(v) = args.positional.first() {
                            if !args.keywords.is_empty() {
                                return Err(PyExc::type_error("duplicate or unexpected keyword"));
                            }
                            integer(v)?
                        } else if args.keywords.is_empty() {
                            BigInt::from(1_000_000)
                        } else if args.keywords.len() == 1
                            && args.keywords[0].0 == "max_denominator"
                        {
                            integer(&args.keywords[0].1)?
                        } else {
                            return Err(PyExc::type_error("unexpected keyword"));
                        };
                        Ok(frac_value(this.limit(max)?))
                    },
                ))
            }
            _ => Err(PyExc::attribute_error(format!(
                "Fraction has no attribute {name}"
            ))),
        }
    }
}
fn fraction_constructor(_: &mut Runtime, args: CallArgs) -> PyResult<Value> {
    args.arity(0, 2)?;
    let mut numerator = args.positional.first().cloned();
    let mut denominator = args.positional.get(1).cloned();
    for (name, value) in args.keywords {
        let slot = match name.as_str() {
            "numerator" => &mut numerator,
            "denominator" => &mut denominator,
            _ => return Err(PyExc::type_error("unexpected keyword argument")),
        };
        if slot.is_some() {
            return Err(PyExc::type_error("multiple values for argument"));
        }
        *slot = Some(value);
    }
    let mut positional = Vec::new();
    if numerator.is_some() || denominator.is_some() {
        positional.push(numerator.unwrap_or_else(|| Value::from(0i64)));
    }
    if let Some(denominator) = denominator.filter(|v| !matches!(v, Value::None)) {
        positional.push(denominator);
    }
    let f = match positional.as_slice() {
        [] => Fraction::new(BigInt::zero(), BigInt::one())?,
        [v] => match v {
            Value::Str(s) => Fraction::from_str(s.as_str())?,
            Value::Float(f) => Fraction::from_float(*f)?,
            _ => numeric_ratio(v)?.ok_or_else(|| {
                PyExc::type_error("argument should be a string or a Rational instance")
            })?,
        },
        [a, b] => {
            let a = numeric_ratio(a)?
                .ok_or_else(|| PyExc::type_error("both arguments should be Rational"))?;
            let b = numeric_ratio(b)?
                .ok_or_else(|| PyExc::type_error("both arguments should be Rational"))?;
            if matches!(positional[0], Value::Float(_))
                || matches!(positional[1], Value::Float(_))
                || obj::<Decimal>(&positional[0]).is_some()
                || obj::<Decimal>(&positional[1]).is_some()
            {
                return Err(PyExc::type_error("both arguments should be Rational"));
            }
            Fraction::new(a.numerator * b.denominator, a.denominator * b.numerator)?
        }
        _ => unreachable!(),
    };
    Ok(frac_value(f))
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decimal {
    coefficient: BigInt,
    exponent: i32,
    negative_zero: bool,
}
impl Decimal {
    fn parse(text: &str) -> PyResult<Self> {
        let normalized = normalize_decimal_digits(text);
        let cleaned = normalized.trim().replace('_', "");
        let (negative, body) = if let Some(v) = cleaned.strip_prefix('-') {
            (true, v)
        } else if let Some(v) = cleaned.strip_prefix('+') {
            (false, v)
        } else {
            (false, cleaned.as_str())
        };
        let lower = body.to_ascii_lowercase();
        if lower == "inf"
            || lower == "infinity"
            || lower.starts_with("nan")
            || lower.starts_with("snan")
        {
            return Err(PyExc::not_implemented_error("non-finite Decimal values"));
        }
        let (mantissa, exponent) = match body.find(['e', 'E']) {
            Some(i) => (
                &body[..i],
                body[i + 1..]
                    .parse::<i32>()
                    .map_err(|_| conversion_syntax())?,
            ),
            None => (body, 0),
        };
        let mut seen_dot = false;
        let mut fraction = 0i32;
        let mut digits = String::new();
        for c in mantissa.chars() {
            if c == '.' && !seen_dot {
                seen_dot = true;
            } else if c.is_ascii_digit() {
                digits.push(c);
                if seen_dot {
                    fraction = fraction.checked_add(1).ok_or_else(conversion_syntax)?;
                }
            } else {
                return Err(conversion_syntax());
            }
        }
        if digits.is_empty() {
            return Err(conversion_syntax());
        }
        let mut coefficient = digits.parse::<BigInt>().map_err(|_| conversion_syntax())?;
        if negative {
            coefficient = -coefficient;
        }
        let exponent = exponent
            .checked_sub(fraction)
            .ok_or_else(conversion_syntax)?;
        Ok(Self {
            negative_zero: negative && coefficient.is_zero(),
            coefficient,
            exponent,
        })
    }
    fn from_int(n: BigInt) -> Self {
        Self {
            coefficient: n,
            exponent: 0,
            negative_zero: false,
        }
    }
    fn ratio(&self) -> PyResult<Fraction> {
        if self.exponent >= 0 {
            Fraction::new(
                &self.coefficient * pow10(self.exponent as u32)?,
                BigInt::one(),
            )
        } else {
            Fraction::new(
                self.coefficient.clone(),
                pow10(self.exponent.unsigned_abs())?,
            )
        }
    }
    fn fixed(&self) -> PyResult<String> {
        let negative = self.coefficient.is_negative() || self.negative_zero;
        let digits = self.coefficient.abs().to_string();
        let len = digits.len();
        let mut out = String::new();
        if negative {
            out.push('-');
        }
        if self.exponent >= 0 {
            let n = self.exponent as usize;
            if n > 1_000_000 {
                return Err(PyExc::overflow_error("decimal output too large"));
            }
            out.push_str(&digits);
            if !self.coefficient.is_zero() {
                out.push_str(&"0".repeat(n));
            }
        } else {
            let places = self.exponent.unsigned_abs() as usize;
            if places > 1_000_000 {
                return Err(PyExc::overflow_error("decimal output too large"));
            }
            if len > places {
                out.push_str(&digits[..len - places]);
                out.push('.');
                out.push_str(&digits[len - places..]);
            } else {
                out.push_str("0.");
                out.push_str(&"0".repeat(places - len));
                out.push_str(&digits);
            }
        }
        Ok(out)
    }
    fn text(&self) -> PyResult<String> {
        let digits = self.coefficient.abs().to_string();
        let adjusted = self.exponent as i64 + digits.len() as i64 - 1;
        if self.exponent <= 0 && adjusted >= -6 {
            return self.fixed();
        }
        let sign = if self.coefficient.is_negative() || self.negative_zero {
            "-"
        } else {
            ""
        };
        let mantissa = if digits.len() == 1 {
            digits
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        Ok(format!("{sign}{mantissa}E{adjusted:+}"))
    }
    fn quantize(&self, exponent: i32) -> PyResult<Self> {
        if !(-1_000_026..=999_999).contains(&exponent) {
            return Err(invalid_decimal());
        }
        let shift = i64::from(self.exponent) - i64::from(exponent);
        let negative = self.coefficient.is_negative() || self.negative_zero;
        let coefficient = if shift >= 0 {
            if self.coefficient.is_zero() {
                BigInt::zero()
            } else {
                let digits = self.coefficient.abs().to_string().len() as i64;
                if digits + shift > 28 {
                    return Err(invalid_decimal());
                }
                let shift = u32::try_from(shift).map_err(|_| invalid_decimal())?;
                &self.coefficient * pow10(shift).map_err(|_| invalid_decimal())?
            }
        } else {
            let shift = u32::try_from(-shift).map_err(|_| invalid_decimal())?;
            if shift as usize > self.coefficient.abs().to_string().len() {
                BigInt::zero()
            } else {
                let divisor = pow10(shift).map_err(|_| invalid_decimal())?;
                let abs = self.coefficient.abs();
                let mut q = &abs / &divisor;
                let rem = abs % &divisor;
                if &rem * 2u8 >= divisor {
                    q += 1;
                }
                if negative { -q } else { q }
            }
        };
        let digits = coefficient.abs().to_string().len();
        if digits > 28
            || (!coefficient.is_zero() && i64::from(exponent) + digits as i64 - 1 > 999_999)
        {
            return Err(invalid_decimal());
        }
        Ok(Self {
            negative_zero: negative && coefficient.is_zero(),
            coefficient,
            exponent,
        })
    }
}
impl PyObject for Decimal {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &str {
        "Decimal"
    }
    fn str(&self) -> PyResult<String> {
        self.text()
    }
    fn repr(&self) -> PyResult<String> {
        Ok(format!("Decimal('{}')", self.text()?))
    }
    fn format(&self, spec: &str) -> PyResult<String> {
        if spec == "f" || spec == "F" {
            self.fixed()
        } else if spec.is_empty() {
            self.text()
        } else {
            Err(PyExc::not_implemented_error("Decimal format specifier"))
        }
    }
    fn is_truthy(&self) -> bool {
        !self.coefficient.is_zero()
    }
    fn hash(&self) -> PyResult<i64> {
        let r = self.ratio()?;
        Ok(rational_hash(&r.numerator, &r.denominator))
    }
    fn equals(&self, other: &Value) -> PyResult<bool> {
        let this = self.ratio()?;
        if let Some(v) = numeric_ratio(other)? {
            Ok(this.numerator * v.denominator == v.numerator * this.denominator)
        } else {
            Ok(false)
        }
    }
    fn get_attr(&self, _: &mut Runtime, name: &str) -> PyResult<Value> {
        match name {
            "scaleb" => {
                let this = self.clone();
                Ok(NativeFunction::value("scaleb", move |_, args| {
                    args.reject_keywords()?;
                    args.arity(1, 1)?;
                    let shift = integer(&args.positional[0])?
                        .to_i32()
                        .ok_or_else(invalid_decimal)?;
                    let exponent = this
                        .exponent
                        .checked_add(shift)
                        .ok_or_else(invalid_decimal)?;
                    if this.coefficient.abs().to_string().len() > 28 {
                        return Err(invalid_decimal());
                    }
                    Ok(decimal_value(Self {
                        coefficient: this.coefficient.clone(),
                        exponent,
                        negative_zero: this.negative_zero,
                    }))
                }))
            }
            "quantize" => {
                let this = self.clone();
                Ok(NativeFunction::value("quantize", move |_, args| {
                    args.arity(1, 2)?;
                    let rounding = if let Some(v) = args.positional.get(1) {
                        Some(v)
                    } else {
                        args.keywords
                            .iter()
                            .find(|(k, _)| k == "rounding")
                            .map(|(_, v)| v)
                    };
                    if args.keywords.iter().any(|(k, _)| k != "rounding")
                        || args.positional.len() > 1 && !args.keywords.is_empty()
                    {
                        return Err(PyExc::type_error("invalid quantize arguments"));
                    }
                    let Some(mode) = rounding else {
                        return Err(PyExc::not_implemented_error(
                            "default decimal rounding mode",
                        ));
                    };
                    if mode.as_str()? != "ROUND_HALF_UP" {
                        return Err(PyExc::not_implemented_error("decimal rounding mode"));
                    }
                    let target = obj::<Decimal>(&args.positional[0]).ok_or_else(|| {
                        PyExc::type_error("conversion to Decimal is not supported")
                    })?;
                    Ok(decimal_value(this.quantize(target.exponent)?))
                }))
            }
            _ => Err(PyExc::attribute_error(format!(
                "Decimal has no attribute {name}"
            ))),
        }
    }
}
fn decimal_constructor(_: &mut Runtime, args: CallArgs) -> PyResult<Value> {
    args.reject_keywords()?;
    args.arity(0, 1)?;
    let d = match args.positional.first() {
        None => Decimal::from_int(BigInt::zero()),
        Some(Value::Int(n)) => Decimal::from_int(n.clone()),
        Some(Value::Bool(b)) => Decimal::from_int(BigInt::from(*b)),
        Some(Value::Str(s)) => Decimal::parse(s.as_str())?,
        Some(v) => {
            if let Some(d) = obj::<Decimal>(v) {
                d.clone()
            } else {
                return Err(PyExc::not_implemented_error(
                    "Decimal constructor for this type",
                ));
            }
        }
    };
    Ok(decimal_value(d))
}
fn init_fractions(rt: &mut Runtime, module: &Value) -> PyResult<()> {
    rt.setattr(
        module,
        "Fraction",
        NativeType::value(
            "Fraction",
            "fractions",
            |v| obj::<Fraction>(v).is_some(),
            fraction_constructor,
        ),
    )
}
fn init_decimal(rt: &mut Runtime, module: &Value) -> PyResult<()> {
    rt.setattr(
        module,
        "Decimal",
        NativeType::value(
            "Decimal",
            "decimal",
            |v| obj::<Decimal>(v).is_some(),
            decimal_constructor,
        ),
    )?;
    rt.setattr(module, "ROUND_HALF_UP", Value::from("ROUND_HALF_UP"))?;
    for name in crate::exceptions::DECIMAL_EXCEPTIONS {
        let exception = crate::exceptions::exception_type(rt, name)?;
        rt.setattr(module, name, exception)?;
    }
    Ok(())
}
/// Install the decimal and fractions modules into a runtime.
pub fn register(rt: &mut Runtime) {
    rt.register_module("fractions", init_fractions);
    rt.register_module("decimal", init_decimal);
}
/// Binary arithmetic dispatch; None means neither operand is one of our objects.
pub fn binary(op: &str, a: &Value, b: &Value) -> Option<PyResult<Value>> {
    let has_fraction = obj::<Fraction>(a).is_some() || obj::<Fraction>(b).is_some();
    let has_decimal = obj::<Decimal>(a).is_some() || obj::<Decimal>(b).is_some();
    if !has_fraction && !has_decimal {
        return None;
    }
    Some((|| {
        if has_decimal {
            if has_fraction || matches!(a, Value::Float(_)) || matches!(b, Value::Float(_)) {
                return Err(PyExc::type_error("unsupported Decimal operands"));
            }
            return Err(PyExc::not_implemented_error("Decimal arithmetic"));
        }
        if matches!(a, Value::Float(_)) || matches!(b, Value::Float(_)) {
            let x = to_float(a).unwrap_or_else(|| a.as_float())?;
            let y = to_float(b).unwrap_or_else(|| b.as_float())?;
            return match op {
                "add" => Ok(Value::Float(x + y)),
                "sub" => Ok(Value::Float(x - y)),
                "mul" => Ok(Value::Float(x * y)),
                "truediv" if y != 0.0 => Ok(Value::Float(x / y)),
                "truediv" => Err(PyExc::zero_division_error("float division by zero")),
                _ => Err(PyExc::not_implemented_error("fraction/float operator")),
            };
        }
        let x = numeric_ratio(a)?.ok_or_else(|| PyExc::type_error("unsupported operands"))?;
        let y = numeric_ratio(b)?.ok_or_else(|| PyExc::type_error("unsupported operands"))?;
        let r = match op {
            "pow" => {
                if !y.denominator.is_one() {
                    return Err(PyExc::not_implemented_error(
                        "non-integral Fraction exponent",
                    ));
                }
                let exponent = y
                    .numerator
                    .abs()
                    .to_u32()
                    .ok_or_else(|| PyExc::overflow_error("fraction exponent too large"))?;
                if exponent > 1_000_000 {
                    return Err(PyExc::overflow_error("fraction exponent too large"));
                }
                if y.numerator.is_negative() {
                    Fraction::new(x.denominator.pow(exponent), x.numerator.pow(exponent))?
                } else {
                    Fraction::new(x.numerator.pow(exponent), x.denominator.pow(exponent))?
                }
            }
            "add" => Fraction::new(
                &x.numerator * &y.denominator + &y.numerator * &x.denominator,
                &x.denominator * &y.denominator,
            )?,
            "sub" => Fraction::new(
                &x.numerator * &y.denominator - &y.numerator * &x.denominator,
                &x.denominator * &y.denominator,
            )?,
            "mul" => Fraction::new(&x.numerator * &y.numerator, &x.denominator * &y.denominator)?,
            "truediv" => {
                Fraction::new(&x.numerator * &y.denominator, &x.denominator * &y.numerator)?
            }
            "floordiv" | "mod" | "divmod" => {
                if y.numerator.is_zero() {
                    return Err(PyExc::zero_division_error("fraction division by zero"));
                }
                let q = floor_div(
                    &(&x.numerator * &y.denominator),
                    &(&x.denominator * &y.numerator),
                );
                if op == "floordiv" {
                    return Ok(Value::Int(q));
                }
                let remainder = Fraction::new(
                    &x.numerator * &y.denominator - &q * &y.numerator * &x.denominator,
                    &x.denominator * &y.denominator,
                )?;
                if op == "divmod" {
                    return Ok(Value::tuple(vec![Value::Int(q), frac_value(remainder)]));
                }
                remainder
            }
            _ => {
                return Err(PyExc::not_implemented_error(format!(
                    "fraction operator {op}"
                )));
            }
        };
        Ok(frac_value(r))
    })())
}
pub fn compare(a: &Value, b: &Value) -> Option<PyResult<Ordering>> {
    if obj::<Fraction>(a).is_none()
        && obj::<Fraction>(b).is_none()
        && obj::<Decimal>(a).is_none()
        && obj::<Decimal>(b).is_none()
    {
        return None;
    }
    Some((|| {
        if let Value::Float(value) = a
            && value.is_infinite()
        {
            return Ok(if value.is_sign_negative() {
                Ordering::Less
            } else {
                Ordering::Greater
            });
        }
        if let Value::Float(value) = b
            && value.is_infinite()
        {
            return Ok(if value.is_sign_negative() {
                Ordering::Greater
            } else {
                Ordering::Less
            });
        }
        let x = numeric_ratio(a)?.ok_or_else(|| PyExc::type_error("unsupported comparison"))?;
        let y = numeric_ratio(b)?.ok_or_else(|| PyExc::type_error("unsupported comparison"))?;
        Ok((x.numerator * y.denominator).cmp(&(y.numerator * x.denominator)))
    })())
}
pub fn to_int(v: &Value) -> Option<PyResult<BigInt>> {
    if let Some(f) = obj::<Fraction>(v) {
        Some(Ok(&f.numerator / &f.denominator))
    } else {
        obj::<Decimal>(v).map(|d| d.ratio().map(|f| f.numerator / f.denominator))
    }
}
pub fn to_float(v: &Value) -> Option<PyResult<f64>> {
    if let Some(f) = obj::<Fraction>(v) {
        Some(crate::ops::int_ratio_to_float(&f.numerator, &f.denominator))
    } else {
        obj::<Decimal>(v).map(|d| {
            let negative = d.coefficient.is_negative() || d.negative_zero;
            let adjusted = i64::from(d.exponent) + d.coefficient.abs().to_string().len() as i64 - 1;
            if !d.coefficient.is_zero() && adjusted > 309 {
                return Ok(if negative {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                });
            }
            if d.coefficient.is_zero() || adjusted < -325 {
                return Ok(if negative { -0.0 } else { 0.0 });
            }
            match d
                .ratio()
                .and_then(|f| crate::ops::int_ratio_to_float(&f.numerator, &f.denominator))
            {
                Ok(v) => Ok(v),
                Err(e) if e.kind == "OverflowError" => Ok(if negative {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                }),
                Err(e) => Err(e),
            }
        })
    }
}
pub fn unary(op: &str, v: &Value) -> Option<PyResult<Value>> {
    obj::<Fraction>(v).map(|f| match op {
        "neg" => Ok(frac_value(Fraction {
            numerator: -&f.numerator,
            denominator: f.denominator.clone(),
        })),
        "pos" => Ok(v.clone()),
        "abs" => Ok(frac_value(Fraction {
            numerator: f.numerator.abs(),
            denominator: f.denominator.clone(),
        })),
        _ => Err(PyExc::not_implemented_error("fraction unary operator")),
    })
}

fn round_ratio(n: &BigInt, d: &BigInt) -> BigInt {
    let mut q = floor_div(n, d);
    let r = n - &q * d;
    let twice = &r * 2u8;
    if twice > *d || (twice == *d && (&q & BigInt::one()) != BigInt::zero()) {
        q += 1;
    }
    q
}
/// Python round for Fraction, including the integer-returning no-ndigits form.
pub fn round(v: &Value, ndigits: Option<&Value>) -> Option<PyResult<Value>> {
    let f = obj::<Fraction>(v)?;
    Some((|| {
        let Some(ndigits) = ndigits.filter(|v| !matches!(v, Value::None)) else {
            return Ok(Value::Int(round_ratio(&f.numerator, &f.denominator)));
        };
        let n = integer(ndigits)?
            .to_i32()
            .ok_or_else(|| PyExc::overflow_error("ndigits too large"))?;
        if n >= 0 {
            let scale = pow10(n as u32)?;
            Ok(frac_value(Fraction::new(
                round_ratio(&(&f.numerator * &scale), &f.denominator),
                scale,
            )?))
        } else {
            let scale = pow10(n.unsigned_abs())?;
            Ok(frac_value(Fraction::new(
                round_ratio(&f.numerator, &(&f.denominator * &scale)) * scale,
                BigInt::one(),
            )?))
        }
    })())
}

//! CPython 3.14's 64-bit hash and set table algorithms (PYTHONHASHSEED=0).
//!
//! NaNs require CPython object addresses and cannot have a portable matching
//! hash. They raise NotImplementedError rather than silently inventing one.
use crate::{PyExc, PyResult, Value};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use std::cell::RefCell;
use std::rc::Rc;

const MODULUS: u64 = (1 << 61) - 1;

fn fix_hash(n: i64) -> i64 {
    if n == -1 { -2 } else { n }
}

pub fn hash(value: &Value) -> PyResult<i64> {
    hash_at(value, 0)
}

fn hash_at(value: &Value, depth: usize) -> PyResult<i64> {
    if depth > 200 {
        return Err(PyExc::recursion_error(
            "maximum recursion depth exceeded while hashing a tuple",
        ));
    }
    Ok(match value {
        Value::None => 0xfca86420,
        Value::Bool(b) => i64::from(*b),
        Value::Int(n) => fix_hash(
            (n % BigInt::from(MODULUS))
                .to_i64()
                .expect("hash modulus fits i64"),
        ),
        Value::Float(n) => float_hash(*n)?,
        Value::Str(s) => {
            let width = match s.as_str().chars().map(u32::from).max().unwrap_or(0) {
                0..=255 => 1,
                256..=65535 => 2,
                _ => 4,
            };
            let mut bytes = Vec::with_capacity(s.as_str().len());
            for c in s.as_str().chars() {
                bytes.extend_from_slice(&(c as u32).to_le_bytes()[..width]);
            }
            buffer_hash(&bytes)
        }
        Value::Tuple(items) => {
            let mut acc = 2870177450012600261u64;
            for item in items.iter() {
                acc = acc.wrapping_add(
                    (hash_at(item, depth + 1)? as u64).wrapping_mul(14029467366897019727),
                );
                acc = acc.rotate_left(31).wrapping_mul(11400714785074694791);
            }
            acc = acc.wrapping_add(items.len() as u64 ^ (2870177450012600261u64 ^ 3527539));
            if acc == u64::MAX {
                1546275796
            } else {
                acc as i64
            }
        }
        Value::Date(text) => {
            let d = crate::datetime::PyDate::fromisoformat(text)?;
            buffer_hash(&[(d.year() >> 8) as u8, d.year() as u8, d.month(), d.day()])
        }
        Value::DateTime(text) => {
            let d = crate::datetime::PyDateTime::fromisoformat(text)?;
            if let Some(offset) = d.offset_seconds() {
                let seconds =
                    i64::from(d.hour()) * 3600 + i64::from(d.minute()) * 60 + i64::from(d.second())
                        - i64::from(offset);
                let delta = crate::datetime::PyTimedelta::new(
                    d.date().toordinal(),
                    seconds,
                    i64::from(d.microsecond()),
                )?;
                hash_at(
                    &Value::tuple(vec![
                        Value::from(i64::from(delta.days())),
                        Value::from(i64::from(delta.seconds())),
                        Value::from(i64::from(delta.microseconds())),
                    ]),
                    depth + 1,
                )?
            } else {
                let us = d.microsecond();
                buffer_hash(&[
                    (d.year() >> 8) as u8,
                    d.year() as u8,
                    d.month(),
                    d.day(),
                    d.hour(),
                    d.minute(),
                    d.second(),
                    (us >> 16) as u8,
                    (us >> 8) as u8,
                    us as u8,
                ])
            }
        }
        Value::FrozenSet(s) => frozen_set_hash(s)?,
        Value::Object(o) => o.hash()?,
        Value::List(_) | Value::Dict(_) | Value::Set(_) => {
            return Err(PyExc::type_error(format!(
                "unhashable type: '{}'",
                value.type_name()
            )));
        }
        _ => {
            return Err(PyExc::type_error(format!(
                "unhashable type: '{}'",
                value.type_name()
            )));
        }
    })
}

fn float_hash(n: f64) -> PyResult<i64> {
    if n.is_nan() {
        return Err(PyExc::not_implemented_error(
            "NaN hashes depend on CPython object addresses",
        ));
    }
    if n.is_infinite() {
        return Ok(if n.is_sign_negative() {
            -314159
        } else {
            314159
        });
    }
    let bits = n.to_bits();
    let exponent = ((bits >> 52) & 2047) as i32;
    let mantissa = bits & ((1u64 << 52) - 1);
    let (mantissa, exponent) = if exponent == 0 {
        (mantissa, -1074)
    } else {
        (mantissa | (1 << 52), exponent - 1075)
    };
    let shift = exponent.rem_euclid(61) as u32;
    let x = ((mantissa << shift) & MODULUS) | (mantissa >> (61 - shift));
    Ok(fix_hash(if n.is_sign_negative() {
        -(x as i64)
    } else {
        x as i64
    }))
}

fn sip_round(v: &mut [u64; 4]) {
    v[0] = v[0].wrapping_add(v[1]);
    v[1] = v[1].rotate_left(13);
    v[1] ^= v[0];
    v[0] = v[0].rotate_left(32);
    v[2] = v[2].wrapping_add(v[3]);
    v[3] = v[3].rotate_left(16);
    v[3] ^= v[2];
    v[0] = v[0].wrapping_add(v[3]);
    v[3] = v[3].rotate_left(21);
    v[3] ^= v[0];
    v[2] = v[2].wrapping_add(v[1]);
    v[1] = v[1].rotate_left(17);
    v[1] ^= v[2];
    v[2] = v[2].rotate_left(32);
}

pub(crate) fn buffer_hash(bytes: &[u8]) -> i64 {
    if bytes.is_empty() {
        return 0;
    }
    let mut v = [
        0x736f6d6570736575,
        0x646f72616e646f6d,
        0x6c7967656e657261,
        0x7465646279746573,
    ];
    let (chunks, remainder) = bytes.as_chunks::<8>();
    for chunk in chunks {
        let m = u64::from_le_bytes(*chunk);
        v[3] ^= m;
        sip_round(&mut v);
        v[0] ^= m;
    }
    let mut b = (bytes.len() as u64) << 56;
    for (i, byte) in remainder.iter().enumerate() {
        b |= u64::from(*byte) << (i * 8);
    }
    v[3] ^= b;
    sip_round(&mut v);
    v[0] ^= b;
    v[2] ^= 255;
    for _ in 0..3 {
        sip_round(&mut v);
    }
    fix_hash((v[0] ^ v[1] ^ v[2] ^ v[3]) as i64)
}

#[derive(Clone, Debug)]
enum Slot {
    Empty,
    Dummy,
    Active { hash: i64, value: Value },
}

#[derive(Clone, Debug)]
struct Table {
    slots: Vec<Slot>,
    used: usize,
    fill: usize,
    finger: usize,
}

/// Cloning aliases the mutable set. `copy` creates an independent table.
#[derive(Clone, Debug)]
pub struct PySet(Rc<RefCell<Table>>);

impl Default for PySet {
    fn default() -> Self {
        Self::new()
    }
}

impl PySet {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Table {
            slots: vec![Slot::Empty; 8],
            used: 0,
            fill: 0,
            finger: 0,
        })))
    }
    pub fn from_values(iter: impl IntoIterator<Item = Value>) -> PyResult<Self> {
        let s = Self::new();
        for value in iter {
            s.insert(value)?;
        }
        Ok(s)
    }
    pub fn len(&self) -> usize {
        self.0.borrow().used
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn is_identical(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    pub(crate) fn identity(&self) -> usize {
        Rc::as_ptr(&self.0) as usize
    }
    /// Preserve table layout, including deletion markers, for runtime snapshots.
    pub fn copy(&self) -> Self {
        Self(Rc::new(RefCell::new(self.0.borrow().clone())))
    }
    /// CPython's language-level shallow copy compacts through bulk merge.
    pub fn python_copy(&self) -> PyResult<Self> {
        let out = Self::new();
        out.update_set(self)?;
        Ok(out)
    }
    pub fn clear(&self) {
        let mut t = self.0.borrow_mut();
        t.slots = vec![Slot::Empty; 8];
        t.used = 0;
        t.fill = 0;
        // CPython intentionally leaves the pop finger unchanged on clear.
    }
    pub fn pop(&self) -> PyResult<Value> {
        let mut t = self.0.borrow_mut();
        if t.used == 0 {
            return Err(PyExc::key_error("pop from an empty set"));
        }
        let mask = t.slots.len() - 1;
        let mut i = t.finger & mask;
        loop {
            if matches!(t.slots[i], Slot::Active { .. }) {
                let Slot::Active { value, .. } = std::mem::replace(&mut t.slots[i], Slot::Dummy)
                else {
                    unreachable!()
                };
                t.used -= 1;
                t.finger = i + 1;
                return Ok(value);
            }
            i = (i + 1) & mask;
        }
    }
    pub fn replace_contents(&self, other: &Self) {
        if self.is_identical(other) {
            return;
        }
        let mut copied = other.0.borrow().clone();
        let mut t = self.0.borrow_mut();
        copied.finger = t.finger; // set_swap_bodies doesn't swap pop fingers.
        *t = copied;
    }
    pub fn reserve_for_update(&self, count: usize) {
        let mut t = self.0.borrow_mut();
        if (t.fill + count) * 5 >= (t.slots.len() - 1) * 3 {
            let minused = (t.used + count) * 2;
            t.resize(minused);
        }
    }
    pub fn update_set(&self, other: &Self) -> PyResult<()> {
        if self.is_identical(other) || other.is_empty() {
            return Ok(());
        }
        let other = other.0.borrow().clone();
        self.reserve_for_update(other.used);
        {
            let mut t = self.0.borrow_mut();
            if t.fill == 0 {
                if t.slots.len() == other.slots.len() && other.fill == other.used {
                    t.slots = other.slots;
                } else {
                    for slot in other.slots {
                        if let Slot::Active { hash, .. } = &slot {
                            t.insert_clean(*hash, slot);
                        }
                    }
                }
                t.used = other.used;
                t.fill = other.used;
                return Ok(());
            }
        }
        for slot in other.slots {
            if let Slot::Active { value, .. } = slot {
                self.insert(value)?;
            }
        }
        Ok(())
    }
    pub fn compact_after_difference(&self) {
        let mut t = self.0.borrow_mut();
        if t.fill - t.used > (t.slots.len() - 1) / 4 {
            let minused = t.used * if t.used > 50000 { 2 } else { 4 };
            t.resize(minused);
        }
    }
    pub fn values(&self) -> Vec<Value> {
        self.0
            .borrow()
            .slots
            .iter()
            .filter_map(|s| {
                if let Slot::Active { value, .. } = s {
                    Some(value.clone())
                } else {
                    None
                }
            })
            .collect()
    }
    pub fn insert(&self, value: Value) -> PyResult<bool> {
        let h = element_hash(&value)?;
        let mut t = self.0.borrow_mut();
        let (pos, found) = t.lookup(&value, h, true)?;
        if found {
            return Ok(false);
        }
        let unused = matches!(t.slots[pos], Slot::Empty);
        t.slots[pos] = Slot::Active { hash: h, value };
        t.used += 1;
        if unused {
            t.fill += 1;
            if t.fill * 5 >= (t.slots.len() - 1) * 3 {
                let minused = t.used * if t.used > 50000 { 2 } else { 4 };
                t.resize(minused);
            }
        }
        Ok(true)
    }
    pub fn contains(&self, value: &Value) -> PyResult<bool> {
        let h = lookup_hash(value)?;
        Ok(self.0.borrow().lookup(value, h, false)?.1)
    }
    pub fn discard(&self, value: &Value) -> PyResult<bool> {
        let h = lookup_hash(value)?;
        let mut t = self.0.borrow_mut();
        let (pos, found) = t.lookup(value, h, false)?;
        if found {
            t.slots[pos] = Slot::Dummy;
            t.used -= 1;
        }
        Ok(found)
    }
    pub fn remove(&self, value: &Value) -> PyResult<()> {
        if self.discard(value)? {
            Ok(())
        } else {
            Err(PyExc::key_error("key not found"))
        }
    }
}

/// Python 3.14 adds contextual detail to set element hash failures.
pub(crate) fn element_hash(value: &Value) -> PyResult<i64> {
    hash(value).map_err(|error| {
        if error.kind == "TypeError" {
            PyExc::type_error(format!(
                "cannot use '{}' as a set element ({})",
                value.type_name(),
                error.message
            ))
        } else {
            error
        }
    })
}

fn lookup_hash(value: &Value) -> PyResult<i64> {
    // Python accepts mutable sets for membership/removal in sets of frozensets.
    if let Value::Set(s) = value {
        frozen_set_hash(s)
    } else {
        element_hash(value)
    }
}

impl Table {
    fn lookup(&self, value: &Value, hash: i64, inserting: bool) -> PyResult<(usize, bool)> {
        let mask = self.slots.len() - 1;
        let mut i = hash as u64 as usize & mask;
        let mut perturb = hash as u64;
        let mut free = None;
        loop {
            let probes = if i + 9 <= mask { 9 } else { 0 };
            for (offset, slot) in self.slots[i..=i + probes].iter().enumerate() {
                let j = i + offset;
                match slot {
                    Slot::Empty => {
                        return Ok((if inserting { free.unwrap_or(j) } else { j }, false));
                    }
                    Slot::Dummy => {
                        if inserting {
                            free = Some(j);
                        }
                    }
                    Slot::Active { hash: h, value: v } => {
                        if *h == hash && (v.is_identical(value) || v.equals(value)?) {
                            return Ok((j, true));
                        }
                    }
                }
            }
            perturb >>= 5;
            i = i
                .wrapping_mul(5)
                .wrapping_add(1)
                .wrapping_add(perturb as usize)
                & mask;
        }
    }
    fn resize(&mut self, minused: usize) {
        let mut size = 8;
        while size <= minused {
            size *= 2;
        }
        if size == 8 && self.slots.len() == 8 && self.fill == self.used {
            return;
        }
        let old = std::mem::replace(&mut self.slots, vec![Slot::Empty; size]);
        self.fill = self.used;
        for slot in old {
            if let Slot::Active { hash, .. } = &slot {
                self.insert_clean(*hash, slot);
            }
        }
    }
    fn insert_clean(&mut self, hash: i64, slot: Slot) {
        let mask = self.slots.len() - 1;
        let mut i = hash as u64 as usize & mask;
        let mut perturb = hash as u64;
        loop {
            let probes = if i + 9 <= mask { 9 } else { 0 };
            for entry in &mut self.slots[i..=i + probes] {
                if matches!(entry, Slot::Empty) {
                    *entry = slot;
                    return;
                }
            }
            perturb >>= 5;
            i = i
                .wrapping_mul(5)
                .wrapping_add(1)
                .wrapping_add(perturb as usize)
                & mask;
        }
    }
}

pub fn frozen_set_hash(set: &PySet) -> PyResult<i64> {
    let t = set.0.borrow();
    let mut h = 0u64;
    for slot in &t.slots {
        if let Slot::Active { hash, .. } = slot {
            let x = *hash as u64;
            h ^= ((x ^ 89869747) ^ (x << 16)).wrapping_mul(3644798167);
        }
    }
    h ^= (t.used as u64 + 1).wrapping_mul(1927868237);
    h ^= (h >> 11) ^ (h >> 25);
    h = h.wrapping_mul(69069).wrapping_add(907133923);
    Ok(if h == u64::MAX { 590923713 } else { h as i64 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpython_zero_seed_hash_vectors() {
        for (text, expected) in [
            ("", 0),
            ("a", 4644417185603328019),
            ("abc", -4594863902769663758),
            ("é", 6047309291227476195),
            ("Ā", 75343234424780393),
            ("😀", -3536540696076613844),
            ("aé😀z", 1307226641285036864),
        ] {
            assert_eq!(hash(&Value::from(text)).unwrap(), expected, "{text}");
        }
        for (n, expected) in [(-1, -2), (-2, -2), (0, 0), (1, 1), (i64::MIN, -4)] {
            assert_eq!(hash(&Value::from(n)).unwrap(), expected);
        }
        let huge: BigInt = BigInt::from(1) << 100usize;
        assert_eq!(hash(&Value::Int(huge.clone())).unwrap(), 549755813888);
        assert_eq!(hash(&Value::Int(-huge)).unwrap(), -549755813888);
        for (n, expected) in [
            (0.5, 1152921504606846976),
            (-0.25, -576460752303423488),
            (1e100, 1822893315824342674),
            (f64::from_bits(1), 16777216),
            (-1.0, -2),
            (-0.0, 0),
            (f64::INFINITY, 314159),
        ] {
            assert_eq!(hash(&Value::Float(n)).unwrap(), expected, "{n}");
        }
        assert_eq!(hash(&Value::None).unwrap(), 0xfca86420);
        assert_eq!(hash(&Value::tuple(vec![])).unwrap(), 5740354900026072187);
        assert_eq!(
            hash(&Value::tuple(vec![Value::from(1i64), Value::from("a")])).unwrap(),
            3906047847676358913
        );
        assert_eq!(frozen_set_hash(&PySet::new()).unwrap(), 133146708735736);
        let s = PySet::from_values([Value::from(1i64), Value::from("a")]).unwrap();
        assert_eq!(frozen_set_hash(&s).unwrap(), 6198228035823778356);
        assert_eq!(
            hash(&Value::Float(f64::NAN)).unwrap_err().kind,
            "NotImplementedError"
        );
    }

    fn integers(s: &PySet) -> Vec<i64> {
        s.values()
            .iter()
            .map(|v| v.as_int().unwrap().to_i64().unwrap())
            .collect()
    }

    #[test]
    fn cpython_collision_resize_delete_order() {
        let s = PySet::from_values((0i64..1000).step_by(32).map(Value::from)).unwrap();
        assert_eq!(
            integers(&s),
            [
                0, 128, 256, 384, 512, 640, 768, 896, 32, 160, 288, 416, 544, 672, 800, 928, 64,
                192, 320, 448, 576, 704, 832, 960, 96, 224, 352, 480, 608, 736, 864, 992
            ]
        );
        for n in (0i64..500).step_by(64) {
            s.remove(&Value::from(n)).unwrap();
        }
        for n in (1000i64..1400).step_by(32) {
            s.insert(Value::from(n)).unwrap();
        }
        assert_eq!(
            integers(&s),
            [
                512, 640, 768, 896, 1032, 1160, 1288, 32, 160, 288, 416, 544, 672, 800, 928, 1064,
                1192, 1320, 576, 704, 832, 960, 1096, 1224, 1352, 96, 224, 352, 480, 608, 736, 864,
                992, 1000, 1128, 1256, 1384
            ]
        );
        let copied = s.copy();
        assert_eq!(integers(&s), integers(&copied));
        assert!(!s.is_identical(&copied));
        let alias = s.clone();
        assert!(s.is_identical(&alias));
        alias.insert(Value::from(9999i64)).unwrap();
        assert!(s.contains(&Value::from(9999i64)).unwrap());
        assert!(!copied.contains(&Value::from(9999i64)).unwrap());
    }

    #[test]
    fn cpython_strings_and_numeric_tuple_duplicates() {
        let s =
            PySet::from_values("abcdefghij".chars().map(|c| Value::from(c.to_string()))).unwrap();
        assert_eq!(
            s.values()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>(),
            ["d", "f", "g", "h", "b", "i", "c", "a", "j", "e"]
        );
        let s = PySet::from_values([
            Value::tuple(vec![Value::Bool(true), Value::from(-1i64)]),
            Value::tuple(vec![Value::Float(1.0), Value::Float(-1.0)]),
        ])
        .unwrap();
        assert_eq!(s.len(), 1);
        assert!(matches!(&s.values()[0], Value::Tuple(v) if matches!(v[0], Value::Bool(true))));
        assert_eq!(s.insert(Value::list(vec![])).unwrap_err().kind, "TypeError");
        assert!(!s.discard(&Value::from(400i64)).unwrap());
        assert_eq!(s.remove(&Value::from(400i64)).unwrap_err().kind, "KeyError");
    }

    #[test]
    fn deletion_compaction_preserves_members_and_frozen_hash() {
        let s = PySet::from_values((0i64..18).map(Value::from)).unwrap();
        for n in 0i64..17 {
            s.remove(&Value::from(n)).unwrap();
        }
        // A later insertion triggers table rebuild when accumulated fill crosses
        // the load limit; plain deletion itself must leave tombstones intact.
        assert_eq!(s.0.borrow().slots.len(), 32);
        assert_eq!(s.0.borrow().fill, 18);
        for n in 40i64..55 {
            s.insert(Value::from(n)).unwrap();
        }
        let clean = PySet::from_values(s.values()).unwrap();
        assert_eq!(
            frozen_set_hash(&s).unwrap(),
            frozen_set_hash(&clean).unwrap()
        );
        for v in clean.values() {
            assert!(s.contains(&v).unwrap());
        }
        assert_eq!(s.len(), 16);
    }
}

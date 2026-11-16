//! CPython 3.14's seeded MT19937 and sequence operations.
//!
//! Float hashes deliberately use the 64-bit CPython convention on every target,
//! including wasm32, so a browser consumes the native oracle's random stream.

// The MT19937 initialization and recurrence follow the reference algorithm:
// Copyright (C) 1997 - 2002, Makoto Matsumoto and Takuji Nishimura,
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
// 3. The names of its contributors may not be used to endorse or promote products
//    derived from this software without specific prior written permission.
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.

use crate::{PyExc, PyResult};
use num_bigint::{BigInt, BigUint};
use num_traits::{One, Signed, Zero};
use sha2::{Digest, Sha512};
use std::collections::HashSet;

const N: usize = 624;

/// The Python `getstate()` tuple, with the internal tuple split into typed fields.
#[derive(Clone, Debug, PartialEq)]
pub struct RandomState {
    pub version: i64,
    pub words: [u32; N],
    pub index: usize,
    pub gauss_next: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct PyRandom {
    mt: [u32; N],
    index: usize,
    words_consumed: u64,
    gauss_next: Option<f64>,
    opaque: bool,
}

impl PyRandom {
    pub fn from_int(seed: &BigInt) -> Self {
        Self::from_unsigned(seed.magnitude())
    }

    // This seeds a stream from text; it does not parse a serialized RNG state.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(seed: &str) -> Self {
        Self::from_bytes(seed.as_bytes())
    }

    /// Version-2 Python bytes and bytearray seeding.
    pub fn from_bytes(seed: &[u8]) -> Self {
        let mut bytes = Vec::with_capacity(seed.len() + 64);
        bytes.extend_from_slice(seed);
        bytes.extend_from_slice(&Sha512::digest(seed));
        Self::from_unsigned(&BigUint::from_bytes_be(&bytes))
    }

    pub fn from_float(seed: f64) -> PyResult<Self> {
        // NaN's Python hash is an object address, rather than a function of its
        // numeric bits. There is no cross-process seed to reproduce for it.
        if seed.is_nan() {
            return Err(PyExc::value_error(
                "NaN random seeds require an object-identity hash",
            ));
        }
        let hash = float_seed_hash(seed);
        Ok(Self::from_unsigned(&BigUint::from(hash as u64)))
    }

    fn from_unsigned(seed: &BigUint) -> Self {
        let mut key = seed.to_u32_digits();
        if key.is_empty() {
            key.push(0);
        }
        let mut result = Self {
            mt: [0; N],
            index: N,
            words_consumed: 0,
            gauss_next: None,
            opaque: false,
        };
        result.mt[0] = 19_650_218;
        for i in 1..N {
            let prev = result.mt[i - 1];
            result.mt[i] = (prev ^ (prev >> 30))
                .wrapping_mul(1_812_433_253)
                .wrapping_add(i as u32);
        }
        let (mut i, mut j) = (1, 0);
        for _ in 0..N.max(key.len()) {
            let prev = result.mt[i - 1];
            result.mt[i] = (result.mt[i] ^ (prev ^ (prev >> 30)).wrapping_mul(1_664_525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i == N {
                result.mt[0] = result.mt[N - 1];
                i = 1;
            }
            if j == key.len() {
                j = 0;
            }
        }
        for _ in 0..N - 1 {
            let prev = result.mt[i - 1];
            result.mt[i] = (result.mt[i] ^ (prev ^ (prev >> 30)).wrapping_mul(1_566_083_941))
                .wrapping_sub(i as u32);
            i += 1;
            if i == N {
                result.mt[0] = result.mt[N - 1];
                i = 1;
            }
        }
        result.mt[0] = 0x8000_0000;
        result
    }

    fn twist(&mut self) {
        for i in 0..N {
            let joined = (self.mt[i] & 0x8000_0000) | (self.mt[(i + 1) % N] & 0x7fff_ffff);
            self.mt[i] = self.mt[(i + 397) % N]
                ^ (joined >> 1)
                ^ if joined & 1 == 1 { 0x9908_b0df } else { 0 };
        }
        self.index = 0;
    }

    pub fn next_u32(&mut self) -> u32 {
        if self.index == N {
            self.twist();
        }
        let mut word = self.mt[self.index];
        self.index += 1;
        self.words_consumed = self.words_consumed.wrapping_add(1);
        word ^= word >> 11;
        word ^= (word << 7) & 0x9d2c_5680;
        word ^= (word << 15) & 0xefc6_0000;
        word ^ (word >> 18)
    }

    pub fn random(&mut self) -> f64 {
        let high = self.next_u32() >> 5;
        let low = self.next_u32() >> 6;
        (f64::from(high) * 67_108_864.0 + f64::from(low)) * (1.0 / 9_007_199_254_740_992.0)
    }

    pub fn getrandbits(&mut self, k: i64) -> PyResult<BigInt> {
        if k < 0 {
            return Err(PyExc::value_error("Cannot convert negative int"));
        }
        self.getrandbits_u64(k as u64)
    }

    /// CPython 3.14 accepts the entire unsigned 64-bit bit-count range.
    pub fn getrandbits_u64(&mut self, k: u64) -> PyResult<BigInt> {
        if k == 0 {
            return Ok(BigInt::zero());
        }
        let count = usize::try_from(k.div_ceil(32)).map_err(|_| PyExc {
            kind: "MemoryError",
            message: String::new(),
        })?;
        let mut words = Vec::new();
        words.try_reserve_exact(count).map_err(|_| PyExc {
            kind: "MemoryError",
            message: String::new(),
        })?;
        let mut remaining = k;
        while remaining > 0 {
            let word = self.next_u32();
            words.push(if remaining < 32 {
                word >> (32 - remaining)
            } else {
                word
            });
            remaining = remaining.saturating_sub(32);
        }
        Ok(BigUint::new(words).into())
    }

    pub fn randbelow(&mut self, n: &BigInt) -> PyResult<BigInt> {
        if !n.is_positive() {
            return Err(PyExc::value_error("empty range for randrange()"));
        }
        let bits = i64::try_from(n.bits())
            .map_err(|_| PyExc::overflow_error("number of bits is too large"))?;
        loop {
            let value = self.getrandbits(bits)?;
            if &value < n {
                return Ok(value);
            }
        }
    }

    pub fn randrange(
        &mut self,
        start: &BigInt,
        stop: Option<&BigInt>,
        step: Option<&BigInt>,
    ) -> PyResult<BigInt> {
        let Some(stop) = stop else {
            if step.is_some_and(|step| step != &BigInt::one()) {
                return Err(PyExc::type_error("Missing a non-None stop argument"));
            }
            return self.randbelow(start);
        };
        let one = BigInt::one();
        let step = step.unwrap_or(&one);
        if step.is_zero() {
            return Err(PyExc::value_error("zero step for randrange()"));
        }
        let width = stop - start;
        // Compute ceil(width / step) only for matching directions. This avoids
        // Rust's truncation versus Python's floor division for negative values.
        if width.is_zero() || width.is_positive() != step.is_positive() {
            let message = if step == &one {
                format!("empty range in randrange({start}, {stop})")
            } else {
                format!("empty range in randrange({start}, {stop}, {step})")
            };
            return Err(PyExc::value_error(message));
        }
        let stride = step.abs();
        let count = (width.abs() + &stride - 1) / stride;
        Ok(start + step * self.randbelow(&count)?)
    }

    pub fn randint(&mut self, a: &BigInt, b: &BigInt) -> PyResult<BigInt> {
        if b < a {
            return Err(PyExc::value_error(format!(
                "empty range in randint({a}, {b})"
            )));
        }
        Ok(a + self.randbelow(&(b - a + 1))?)
    }

    pub fn choice_index(&mut self, len: usize) -> PyResult<usize> {
        if len == 0 {
            return Err(PyExc::index_error("Cannot choose from an empty sequence"));
        }
        Ok(self.randbelow_usize(len))
    }

    fn randbelow_usize(&mut self, n: usize) -> usize {
        debug_assert!(n > 0);
        let k = usize::BITS - n.leading_zeros();
        loop {
            let value = if k <= 32 {
                u64::from(self.next_u32() >> (32 - k))
            } else {
                u64::from(self.next_u32()) | (u64::from(self.next_u32() >> (64 - k)) << 32)
            };
            if value < n as u64 {
                return value as usize;
            }
        }
    }

    pub fn shuffle<T>(&mut self, sequence: &mut [T]) {
        for i in (1..sequence.len()).rev() {
            sequence.swap(i, self.randbelow_usize(i + 1));
        }
    }

    pub fn sample_indices(&mut self, population: usize, k: usize) -> PyResult<Vec<usize>> {
        if k > population {
            return Err(PyExc::value_error(
                "Sample larger than population or is negative",
            ));
        }
        let mut setsize = 21_usize;
        if k > 5 {
            // random.py uses 4 ** ceil(log(k * 3, 4)).
            let mut size = 1_usize;
            while size < k.saturating_mul(3) {
                size = size.saturating_mul(4);
                if size == usize::MAX {
                    break;
                }
            }
            setsize = setsize.saturating_add(size);
        }
        let mut result = Vec::with_capacity(k);
        if population <= setsize {
            let mut pool: Vec<_> = (0..population).collect();
            for i in 0..k {
                let j = self.randbelow_usize(population - i);
                result.push(pool[j]);
                pool[j] = pool[population - i - 1];
            }
        } else {
            let mut selected = HashSet::with_capacity(k);
            for _ in 0..k {
                let mut j = self.randbelow_usize(population);
                while !selected.insert(j) {
                    j = self.randbelow_usize(population);
                }
                result.push(j);
            }
        }
        Ok(result)
    }

    /// `choices` returns indices so the caller can clone arbitrary Python values.
    pub fn choices_indices(
        &mut self,
        population: usize,
        weights: Option<&[f64]>,
        cumulative_weights: Option<&[f64]>,
        k: usize,
    ) -> PyResult<Vec<usize>> {
        if weights.is_some() && cumulative_weights.is_some() {
            return Err(PyExc::type_error(
                "Cannot specify both weights and cumulative weights",
            ));
        }
        if weights.is_none() && cumulative_weights.is_none() {
            let mut result = Vec::with_capacity(k);
            for _ in 0..k {
                let index = (self.random() * population as f64).floor() as usize;
                if index >= population {
                    return Err(PyExc::index_error("list index out of range"));
                }
                result.push(index);
            }
            return Ok(result);
        }
        let mut accumulated = Vec::new();
        let cumulative = if let Some(weights) = weights {
            let mut sum = 0.0;
            for weight in weights {
                sum += weight;
                accumulated.push(sum);
            }
            accumulated.as_slice()
        } else {
            cumulative_weights.unwrap_or_default()
        };
        if cumulative.len() != population {
            return Err(PyExc::value_error(
                "The number of weights does not match the population",
            ));
        }
        let total = cumulative
            .last()
            .copied()
            .ok_or_else(|| PyExc::index_error("list index out of range"))?;
        if total <= 0.0 {
            return Err(PyExc::value_error(
                "Total of weights must be greater than zero",
            ));
        }
        if !total.is_finite() {
            return Err(PyExc::value_error("Total of weights must be finite"));
        }
        let mut result = Vec::with_capacity(k);
        for _ in 0..k {
            let value = self.random() * total;
            let (mut low, mut high) = (0, population - 1);
            while low < high {
                let mid = low + (high - low) / 2;
                if value < cumulative[mid] {
                    high = mid;
                } else {
                    low = mid + 1;
                }
            }
            result.push(low);
        }
        Ok(result)
    }

    pub fn words_consumed(&self) -> u64 {
        self.words_consumed
    }

    pub fn advance(&mut self, words: u64) {
        for _ in 0..words {
            self.next_u32();
        }
    }

    pub fn getstate(&self) -> RandomState {
        RandomState {
            version: 3,
            words: self.mt,
            index: self.index,
            gauss_next: self.gauss_next,
        }
    }

    /// Successful setstate marks seed/advance restoration as opaque.
    pub fn setstate(&mut self, state: &RandomState) -> PyResult<()> {
        if state.version != 2 && state.version != 3 {
            return Err(PyExc::value_error(format!(
                "state with version {} passed to Random.setstate() of version 3",
                state.version
            )));
        }
        if state.index > N {
            return Err(PyExc::value_error("invalid state"));
        }
        self.mt = state.words;
        self.index = state.index;
        self.gauss_next = state.gauss_next;
        self.opaque = true;
        Ok(())
    }

    pub fn state_is_opaque(&self) -> bool {
        self.opaque
    }
}

fn float_seed_hash(value: f64) -> i64 {
    if value.is_infinite() {
        return if value.is_sign_negative() {
            -314_159
        } else {
            314_159
        };
    }
    let bits = value.to_bits();
    let raw_exp = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (mantissa, exponent) = if raw_exp == 0 {
        (fraction, -1074_i32)
    } else {
        (fraction | (1_u64 << 52), raw_exp - 1023 - 52)
    };
    let modulus = (1_u128 << 61) - 1;
    let magnitude = ((u128::from(mantissa) << exponent.rem_euclid(61)) % modulus) as i64;
    let hash = if value.is_sign_negative() {
        -magnitude
    } else {
        magnitude
    };
    if hash == -1 { -2 } else { hash }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_words(mut rng: PyRandom) -> Vec<u32> {
        (0..4).map(|_| rng.next_u32()).collect()
    }

    #[test]
    fn cpython_integer_seed_vectors() {
        for (seed, expected) in [
            ("0", vec![3626764237, 1654615998, 3255389356, 3823568514]),
            ("-1", vec![577090037, 2444712010, 3639700191, 3445702192]),
            ("42", vec![2746317213, 478163327, 107420369, 3184935163]),
            (
                "1267650600228229401496703205381",
                vec![3020866642, 2006762751, 4231381179, 1936651767],
            ),
        ] {
            assert_eq!(
                first_words(PyRandom::from_int(&seed.parse().unwrap())),
                expected
            );
        }
    }

    #[test]
    fn cpython_string_seed_vectors() {
        for (seed, expected) in [
            ("", vec![4124137760, 1951988028, 2557274880, 888572032]),
            ("abc", vec![3315820543, 4246262336, 2397318194, 1976556168]),
            ("0:0", vec![2511269442, 2900089240, 2461337166, 3782514028]),
            ("🦀", vec![4109670556, 838928369, 2796891137, 1122309934]),
        ] {
            assert_eq!(first_words(PyRandom::from_str(seed)), expected);
        }
    }

    #[test]
    fn cpython_float_seed_vectors() {
        for (seed, expected) in [
            (1.5, vec![2369807148, 3534653101, 1099911448, 624696387]),
            (-1.5, vec![3049922142, 1895599190, 3396863, 1112112312]),
            (
                f64::INFINITY,
                vec![826196214, 1180060831, 1231978967, 423847419],
            ),
            (
                f64::NEG_INFINITY,
                vec![3525451760, 3478824123, 1254833236, 2666173332],
            ),
            (-0.0, vec![3626764237, 1654615998, 3255389356, 3823568514]),
        ] {
            assert_eq!(first_words(PyRandom::from_float(seed).unwrap()), expected);
        }
        for (seed, expected) in [
            (f64::from_bits(1), 2056082574),
            (f64::MAX, 266596682),
            (-1.0, 563479156),
            (-2.0, 563479156),
            (1e-200, 1396800168),
            (-1e200, 1478463967),
        ] {
            assert_eq!(PyRandom::from_float(seed).unwrap().next_u32(), expected);
        }
    }

    #[test]
    fn cpython_mixed_stream_and_sampling() {
        let mut rng = PyRandom::from_int(&0.into());
        assert_eq!(rng.random(), 0.8444218515250481);
        assert_eq!(rng.getrandbits(1).unwrap(), 1.into());
        assert_eq!(rng.getrandbits(33).unwrap(), 3823568514_u64.into());
        assert_eq!(
            rng.randrange(&(BigInt::one() << 70), None, None).unwrap(),
            "949747731584025552482".parse::<BigInt>().unwrap()
        );
        assert_eq!(
            rng.sample_indices(100, 7).unwrap(),
            vec![38, 61, 45, 74, 27, 64, 17]
        );
        assert_eq!(
            rng.sample_indices(10, 7).unwrap(),
            vec![4, 2, 1, 9, 8, 6, 7]
        );
    }

    #[test]
    fn state_restore_and_word_count() {
        let mut rng = PyRandom::from_str("restore");
        assert_eq!(rng.getrandbits(0).unwrap(), BigInt::zero());
        assert_eq!(rng.words_consumed(), 0);
        rng.random();
        rng.getrandbits(65).unwrap();
        assert_eq!(rng.words_consumed(), 5);
        let state = rng.getstate();
        let expected: Vec<_> = (0..1300).map(|_| rng.next_u32()).collect();
        let mut restored = PyRandom::from_str("restore");
        restored.advance(5);
        assert_eq!(restored.getstate(), state);
        assert_eq!(
            (0..1300).map(|_| restored.next_u32()).collect::<Vec<_>>(),
            expected
        );
        restored.setstate(&state).unwrap();
        assert!(restored.state_is_opaque());
        assert_eq!(
            (0..1300).map(|_| restored.next_u32()).collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn invalid_inputs_do_not_consume_words() {
        let mut rng = PyRandom::from_int(&42.into());
        assert!(rng.getrandbits(-1).is_err());
        assert!(rng.randrange(&0.into(), None, None).is_err());
        assert!(
            rng.randrange(&1.into(), Some(&5.into()), Some(&0.into()))
                .is_err()
        );
        assert!(rng.randint(&5.into(), &1.into()).is_err());
        assert!(rng.choice_index(0).is_err());
        assert!(rng.sample_indices(1, 2).is_err());
        assert_eq!(rng.words_consumed(), 0);
    }

    #[test]
    fn cpython_twist_boundaries() {
        let mut rng = PyRandom::from_str("boundary");
        let expected = [
            (0, 511214987),
            (622, 656416600),
            (623, 1682511375),
            (624, 3012886688),
            (625, 665577342),
            (1246, 1550826939),
            (1247, 2514005694),
            (1248, 575262063),
            (1299, 772851786),
        ];
        let actual: Vec<_> = (0..1300).map(|_| rng.next_u32()).collect();
        for (index, word) in expected {
            assert_eq!(actual[index], word);
        }
    }

    #[test]
    fn cpython_shuffle_and_weighted_choices() {
        let mut rng = PyRandom::from_int(&0.into());
        let mut sequence: Vec<_> = (0..10).collect();
        rng.shuffle(&mut sequence);
        assert_eq!(sequence, vec![7, 8, 1, 5, 3, 4, 2, 0, 9, 6]);
        assert_eq!(
            rng.choices_indices(5, Some(&[1., 0., 2., 3., 4.]), None, 8)
                .unwrap(),
            vec![3, 4, 2, 2, 2, 0, 4, 4]
        );
    }
}

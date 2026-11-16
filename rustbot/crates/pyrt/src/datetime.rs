//! Gregorian dates and fixed-offset datetimes with an explicitly injected clock.
//!
//! No API here reads the machine clock. Named timezones, DST, leap seconds and
//! subsecond timezone offsets are outside this module's current scope.
use crate::{PyExc, PyResult, Value};
use num_bigint::BigInt;
use num_traits::{FromPrimitive, ToPrimitive};
use std::cmp::Ordering;
use std::fmt;

const SECOND: i128 = 1_000_000;
const DAY: i128 = 86_400 * SECOND;
const MAX_ORDINAL: i64 = 3_652_059;

pub fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_before_year(year: i64) -> i128 {
    let y = i128::from(year) - 1;
    365 * y + y.div_euclid(4) - y.div_euclid(100) + y.div_euclid(400)
}

fn month_days(year: i64, month: i64) -> PyResult<u8> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Ok(31),
        4 | 6 | 9 | 11 => Ok(30),
        2 => Ok(if is_leap(year) { 29 } else { 28 }),
        _ => Err(PyExc::value_error(format!(
            "bad month number {month}; must be 1-12"
        ))),
    }
}

/// Monday=0 through Sunday=6; calendar accepts proleptic years outside date's range.
pub fn monthrange(year: i64, month: i64) -> PyResult<(u8, u8)> {
    let count = month_days(year, month)?;
    let previous: i128 = (1..month)
        .map(|m| i128::from(month_days(year, m).expect("valid month")))
        .sum();
    Ok((
        (days_before_year(year) + previous).rem_euclid(7) as u8,
        count,
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PyDate {
    year: i32,
    month: u8,
    day: u8,
}

impl PyDate {
    pub fn new(year: i64, month: i64, day: i64) -> PyResult<Self> {
        if !(1..=9999).contains(&year) {
            return Err(PyExc::value_error(format!("year {year} is out of range")));
        }
        if !(1..=12).contains(&month) {
            return Err(PyExc::value_error("month must be in 1..12"));
        }
        if !(1..=i64::from(month_days(year, month)?)).contains(&day) {
            return Err(PyExc::value_error("day is out of range for month"));
        }
        Ok(Self {
            year: year as i32,
            month: month as u8,
            day: day as u8,
        })
    }
    pub fn year(self) -> i32 {
        self.year
    }
    pub fn month(self) -> u8 {
        self.month
    }
    pub fn day(self) -> u8 {
        self.day
    }
    pub fn toordinal(self) -> i64 {
        let previous: i128 = (1..i64::from(self.month))
            .map(|m| i128::from(month_days(i64::from(self.year), m).expect("valid month")))
            .sum();
        (days_before_year(i64::from(self.year)) + previous + i128::from(self.day)) as i64
    }
    pub fn fromordinal(ordinal: i64) -> PyResult<Self> {
        if !(1..=MAX_ORDINAL).contains(&ordinal) {
            return Err(PyExc::value_error("ordinal must be >= 1 and <= 3652059"));
        }
        let (mut lo, mut hi) = (1i64, 10000i64);
        while lo + 1 < hi {
            let mid = (lo + hi) / 2;
            if days_before_year(mid) < i128::from(ordinal) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let mut day = i128::from(ordinal) - days_before_year(lo);
        let mut month = 1;
        while day > i128::from(month_days(lo, month)?) {
            day -= i128::from(month_days(lo, month)?);
            month += 1;
        }
        Self::new(lo, month, day as i64)
    }
    pub fn weekday(self) -> u8 {
        ((self.toordinal() - 1) % 7) as u8
    }
    pub fn isoweekday(self) -> u8 {
        self.weekday() + 1
    }
    pub fn isocalendar(self) -> (i32, u8, u8) {
        let mut year = self.year;
        let ordinal = self.toordinal();
        let mut monday = iso_week_one(i64::from(year));
        if ordinal < monday {
            year -= 1;
            monday = iso_week_one(i64::from(year));
        } else if ordinal >= iso_week_one(i64::from(year) + 1) {
            year += 1;
            monday = iso_week_one(i64::from(year));
        }
        (year, ((ordinal - monday) / 7 + 1) as u8, self.isoweekday())
    }
    pub fn fromisocalendar(year: i64, week: i64, weekday: i64) -> PyResult<Self> {
        if !(1..=9999).contains(&year) {
            return Err(PyExc::value_error("year out of range"));
        }
        if !(1..=7).contains(&weekday) {
            return Err(PyExc::value_error("Invalid day: must be 1..7"));
        }
        let start = iso_week_one(year);
        let weeks = (iso_week_one(year + 1) - start) / 7;
        if !(1..=weeks).contains(&week) {
            return Err(PyExc::value_error(format!("Invalid week: {week}")));
        }
        Self::fromordinal(start + (week - 1) * 7 + weekday - 1)
    }
    pub fn add_days(self, days: i64) -> PyResult<Self> {
        let ordinal = i128::from(self.toordinal()) + i128::from(days);
        if !(1..=i128::from(MAX_ORDINAL)).contains(&ordinal) {
            return Err(PyExc::overflow_error("date value out of range"));
        }
        Self::fromordinal(ordinal as i64)
    }
    pub fn sub_days(self, days: i64) -> PyResult<Self> {
        let days = days
            .checked_neg()
            .ok_or_else(|| PyExc::overflow_error("date value out of range"))?;
        self.add_days(days)
    }
    pub fn add_timedelta(self, delta: PyTimedelta) -> PyResult<Self> {
        self.add_days(i64::from(delta.days))
    }
    pub fn sub_timedelta(self, delta: PyTimedelta) -> PyResult<Self> {
        self.sub_days(i64::from(delta.days))
    }
    pub fn difference(self, other: Self) -> PyTimedelta {
        PyTimedelta::new(self.toordinal() - other.toordinal(), 0, 0).expect("date difference fits")
    }
    pub fn replace(
        self,
        year: Option<i64>,
        month: Option<i64>,
        day: Option<i64>,
    ) -> PyResult<Self> {
        Self::new(
            year.unwrap_or(i64::from(self.year)),
            month.unwrap_or(i64::from(self.month)),
            day.unwrap_or(i64::from(self.day)),
        )
    }
    pub fn isoformat(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
    pub fn fromisoformat(text: &str) -> PyResult<Self> {
        let invalid = || PyExc::value_error(format!("Invalid isoformat string: {text:?}"));
        if !text.is_ascii() {
            return Err(invalid());
        }
        let digits = |s: &str| parse_digits(s).ok_or_else(invalid);
        if text.len() == 10 && &text[4..5] == "-" && &text[7..8] == "-" {
            return Self::new(
                digits(&text[..4])?,
                digits(&text[5..7])?,
                digits(&text[8..])?,
            );
        }
        if text.len() == 8 && text.bytes().all(|c| c.is_ascii_digit()) {
            return Self::new(
                digits(&text[..4])?,
                digits(&text[4..6])?,
                digits(&text[6..])?,
            );
        }
        let (year, week, day) = match text.len() {
            10 if &text[4..6] == "-W" && &text[8..9] == "-" => {
                (&text[..4], &text[6..8], &text[9..])
            }
            8 if &text[4..5] == "W" => (&text[..4], &text[5..7], &text[7..]),
            8 if &text[4..6] == "-W" => (&text[..4], &text[6..8], "1"),
            7 if &text[4..5] == "W" => (&text[..4], &text[5..7], "1"),
            _ => return Err(invalid()),
        };
        Self::fromisocalendar(digits(year)?, digits(week)?, digits(day)?)
    }
    pub fn to_value(self) -> Value {
        Value::Date(self.isoformat())
    }
}

fn iso_week_one(year: i64) -> i64 {
    let jan4 = days_before_year(year) + 4;
    (jan4 - (jan4 - 1).rem_euclid(7)) as i64
}

impl fmt::Display for PyDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.isoformat())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PyTimedelta {
    days: i32,
    seconds: u32,
    microseconds: u32,
}

impl PyTimedelta {
    pub fn new(days: i64, seconds: i64, microseconds: i64) -> PyResult<Self> {
        Self::from_microseconds(
            i128::from(days) * DAY + i128::from(seconds) * SECOND + i128::from(microseconds),
        )
    }
    pub fn from_parts(
        days: i64,
        seconds: i64,
        microseconds: i64,
        milliseconds: i64,
        minutes: i64,
        hours: i64,
        weeks: i64,
    ) -> PyResult<Self> {
        Self::from_microseconds(
            (i128::from(days) + i128::from(weeks) * 7) * DAY
                + (i128::from(seconds) + i128::from(minutes) * 60 + i128::from(hours) * 3600)
                    * SECOND
                + i128::from(milliseconds) * 1000
                + i128::from(microseconds),
        )
    }
    pub fn from_microseconds(total: i128) -> PyResult<Self> {
        let days = total.div_euclid(DAY);
        if !(-999_999_999..=999_999_999).contains(&days) {
            return Err(PyExc::overflow_error(
                "timedelta days must have magnitude <= 999999999",
            ));
        }
        let rest = total.rem_euclid(DAY);
        Ok(Self {
            days: days as i32,
            seconds: (rest / SECOND) as u32,
            microseconds: (rest % SECOND) as u32,
        })
    }
    /// Float arguments follow CPython's per-component accumulation and one
    /// final rounding step. Fractions of days occur in PyBot's moon calendar.
    pub fn from_float_parts(
        days: f64,
        seconds: f64,
        microseconds: f64,
        milliseconds: f64,
        minutes: f64,
        hours: f64,
        weeks: f64,
    ) -> PyResult<Self> {
        let mut total = BigInt::from(0);
        let mut leftover = 0.0;
        for (value, factor) in [
            (microseconds, 1i128),
            (milliseconds, 1000),
            (seconds, SECOND),
            (minutes, 60 * SECOND),
            (hours, 3600 * SECOND),
            (days, DAY),
            (weeks, 7 * DAY),
        ] {
            if value.is_nan() {
                return Err(PyExc::value_error("cannot convert float NaN to integer"));
            }
            if !value.is_finite() {
                return Err(PyExc::overflow_error(
                    "cannot convert float infinity to integer",
                ));
            }
            let whole = BigInt::from_f64(value.trunc()).expect("finite float integral part");
            let fraction = value.fract() * factor as f64;
            total += whole * BigInt::from(factor) + BigInt::from(fraction.trunc() as i128);
            leftover += fraction.fract();
        }
        let mut rounded = leftover.round_ties_even() as i64;
        // At an exact half, parity includes the accumulated integer part.
        if (leftover - leftover.trunc()).abs() == 0.5 {
            let base = leftover.trunc() as i64;
            let parity = (&total + BigInt::from(base)) % BigInt::from(2);
            rounded = base
                + if parity != BigInt::from(0) {
                    if leftover < 0.0 { -1 } else { 1 }
                } else {
                    0
                };
        }
        total += rounded;
        Self::from_microseconds(
            total
                .to_i128()
                .ok_or_else(|| PyExc::overflow_error("timedelta out of range"))?,
        )
    }
    pub fn from_seconds(seconds: f64) -> PyResult<Self> {
        Self::from_float_parts(0.0, seconds, 0.0, 0.0, 0.0, 0.0, 0.0)
    }
    pub fn days(self) -> i32 {
        self.days
    }
    pub fn seconds(self) -> u32 {
        self.seconds
    }
    pub fn microseconds(self) -> u32 {
        self.microseconds
    }
    pub fn total_microseconds(self) -> i128 {
        i128::from(self.days) * DAY
            + i128::from(self.seconds) * SECOND
            + i128::from(self.microseconds)
    }
    pub fn total_seconds(self) -> f64 {
        self.total_microseconds() as f64 / SECOND as f64
    }
    pub fn checked_add(self, other: Self) -> PyResult<Self> {
        Self::from_microseconds(self.total_microseconds() + other.total_microseconds())
    }
    pub fn checked_sub(self, other: Self) -> PyResult<Self> {
        Self::from_microseconds(self.total_microseconds() - other.total_microseconds())
    }
    pub fn negated(self) -> PyResult<Self> {
        Self::from_microseconds(-self.total_microseconds())
    }
    pub fn is_zero(self) -> bool {
        self.total_microseconds() == 0
    }
}

impl fmt::Display for PyTimedelta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.days != 0 {
            write!(
                f,
                "{} day{}, ",
                self.days,
                if self.days.abs() == 1 { "" } else { "s" }
            )?;
        }
        write!(
            f,
            "{}:{:02}:{:02}",
            self.seconds / 3600,
            self.seconds / 60 % 60,
            self.seconds % 60
        )?;
        if self.microseconds != 0 {
            write!(f, ".{:06}", self.microseconds)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PyDateTime {
    date: PyDate,
    hour: u8,
    minute: u8,
    second: u8,
    microsecond: u32,
    offset: Option<i32>,
}

impl PyDateTime {
    pub fn new(
        date: PyDate,
        hour: i64,
        minute: i64,
        second: i64,
        microsecond: i64,
        offset_seconds: Option<i32>,
    ) -> PyResult<Self> {
        for (name, value, max) in [
            ("hour", hour, 23),
            ("minute", minute, 59),
            ("second", second, 59),
            ("microsecond", microsecond, 999999),
        ] {
            if !(0..=max).contains(&value) {
                return Err(PyExc::value_error(format!("{name} must be in 0..{max}")));
            }
        }
        check_offset(offset_seconds)?;
        Ok(Self {
            date,
            hour: hour as u8,
            minute: minute as u8,
            second: second as u8,
            microsecond: microsecond as u32,
            offset: offset_seconds,
        })
    }
    pub fn date(self) -> PyDate {
        self.date
    }
    pub fn year(self) -> i32 {
        self.date.year
    }
    pub fn month(self) -> u8 {
        self.date.month
    }
    pub fn day(self) -> u8 {
        self.date.day
    }
    pub fn hour(self) -> u8 {
        self.hour
    }
    pub fn minute(self) -> u8 {
        self.minute
    }
    pub fn second(self) -> u8 {
        self.second
    }
    pub fn microsecond(self) -> u32 {
        self.microsecond
    }
    pub fn offset_seconds(self) -> Option<i32> {
        self.offset
    }
    pub fn weekday(self) -> u8 {
        self.date.weekday()
    }
    pub fn isocalendar(self) -> (i32, u8, u8) {
        self.date.isocalendar()
    }
    fn local_microseconds(self) -> i128 {
        i128::from(self.date.toordinal() - 1) * DAY
            + (i128::from(self.hour) * 3600
                + i128::from(self.minute) * 60
                + i128::from(self.second))
                * SECOND
            + i128::from(self.microsecond)
    }
    fn from_local_microseconds(total: i128, offset: Option<i32>) -> PyResult<Self> {
        if !(0..i128::from(MAX_ORDINAL) * DAY).contains(&total) {
            return Err(PyExc::overflow_error("date value out of range"));
        }
        let date = PyDate::fromordinal((total / DAY) as i64 + 1)?;
        let rest = total % DAY;
        let seconds = rest / SECOND;
        Self::new(
            date,
            (seconds / 3600) as i64,
            (seconds / 60 % 60) as i64,
            (seconds % 60) as i64,
            (rest % SECOND) as i64,
            offset,
        )
    }
    pub fn add_timedelta(self, delta: PyTimedelta) -> PyResult<Self> {
        Self::from_local_microseconds(
            self.local_microseconds() + delta.total_microseconds(),
            self.offset,
        )
    }
    pub fn sub_timedelta(self, delta: PyTimedelta) -> PyResult<Self> {
        Self::from_local_microseconds(
            self.local_microseconds() - delta.total_microseconds(),
            self.offset,
        )
    }
    pub fn difference(self, other: Self) -> PyResult<PyTimedelta> {
        if self.offset.is_some() != other.offset.is_some() {
            return Err(PyExc::type_error(
                "can't subtract offset-naive and offset-aware datetimes",
            ));
        }
        PyTimedelta::from_microseconds(
            self.local_microseconds()
                - other.local_microseconds()
                - i128::from(self.offset.unwrap_or(0) - other.offset.unwrap_or(0)) * SECOND,
        )
    }
    pub fn compare(self, other: Self) -> PyResult<Ordering> {
        if self.offset.is_some() != other.offset.is_some() {
            return Err(PyExc::type_error(
                "can't compare offset-naive and offset-aware datetimes",
            ));
        }
        let instant = |v: Self| v.local_microseconds() - i128::from(v.offset.unwrap_or(0)) * SECOND;
        Ok(instant(self).cmp(&instant(other)))
    }
    /// Convert an aware instant to another fixed offset; never infer a local zone.
    pub fn astimezone(self, offset_seconds: i32) -> PyResult<Self> {
        check_offset(Some(offset_seconds))?;
        let source = self
            .offset
            .ok_or_else(|| PyExc::value_error("naive datetime has no injected local timezone"))?;
        Self::from_local_microseconds(
            self.local_microseconds() + i128::from(offset_seconds - source) * SECOND,
            Some(offset_seconds),
        )
    }
    /// Nested options distinguish preserving tzinfo from explicitly removing it.
    pub fn replace(
        self,
        date: Option<PyDate>,
        hour: Option<i64>,
        minute: Option<i64>,
        second: Option<i64>,
        microsecond: Option<i64>,
        offset_seconds: Option<Option<i32>>,
    ) -> PyResult<Self> {
        Self::new(
            date.unwrap_or(self.date),
            hour.unwrap_or(i64::from(self.hour)),
            minute.unwrap_or(i64::from(self.minute)),
            second.unwrap_or(i64::from(self.second)),
            microsecond.unwrap_or(i64::from(self.microsecond)),
            offset_seconds.unwrap_or(self.offset),
        )
    }
    pub fn isoformat(self) -> String {
        self.isoformat_with('T', "auto").expect("valid timespec")
    }
    pub fn isoformat_with(self, separator: char, timespec: &str) -> PyResult<String> {
        let timespec = if timespec == "auto" {
            if self.microsecond == 0 {
                "seconds"
            } else {
                "microseconds"
            }
        } else {
            timespec
        };
        let mut out = format!("{}{separator}{:02}", self.date, self.hour);
        match timespec {
            "hours" => (),
            "minutes" => out.push_str(&format!(":{:02}", self.minute)),
            "seconds" | "milliseconds" | "microseconds" => {
                out.push_str(&format!(":{:02}:{:02}", self.minute, self.second));
                if timespec == "milliseconds" {
                    out.push_str(&format!(".{:03}", self.microsecond / 1000));
                }
                if timespec == "microseconds" {
                    out.push_str(&format!(".{:06}", self.microsecond));
                }
            }
            _ => return Err(PyExc::value_error("Unknown timespec value")),
        }
        if let Some(offset) = self.offset {
            let magnitude = offset.unsigned_abs();
            out.push_str(&format!(
                "{}{:02}:{:02}",
                if offset < 0 { '-' } else { '+' },
                magnitude / 3600,
                magnitude / 60 % 60
            ));
            if magnitude % 60 != 0 {
                out.push_str(&format!(":{:02}", magnitude % 60));
            }
        }
        Ok(out)
    }
    pub fn fromisoformat(text: &str) -> PyResult<Self> {
        let invalid = || PyExc::value_error(format!("Invalid isoformat string: {text:?}"));
        if let Ok(date) = PyDate::fromisoformat(text) {
            return Self::new(date, 0, 0, 0, 0, None);
        }
        // Date prefixes are ASCII; the one-character date/time separator may
        // be any Unicode character, as accepted by Python fromisoformat.
        let bytes = text.as_bytes();
        let date_len = if bytes.get(4..6) == Some(b"-W") {
            if bytes.get(8) == Some(&b'-') { 10 } else { 8 }
        } else if bytes.get(4) == Some(&b'W') {
            if bytes.get(7).is_some_and(u8::is_ascii_digit) {
                8
            } else {
                7
            }
        } else if bytes.get(4) == Some(&b'-') {
            10
        } else {
            8
        };
        let prefix = text.get(..date_len).ok_or_else(invalid)?;
        let date = PyDate::fromisoformat(prefix)?;
        let rest = text.get(date_len..).ok_or_else(invalid)?;
        let separator = rest.chars().next().ok_or_else(invalid)?;
        let rest = &rest[separator.len_utf8()..];
        let (time, offset) = if let Some(time) = rest.strip_suffix('Z') {
            (time, Some(0))
        } else if let Some(pos) = rest.find(['+', '-']) {
            let sign = if rest.as_bytes()[pos] == b'-' { -1 } else { 1 };
            let (hour, minute, second, micros) = parse_time(&rest[pos + 1..])?;
            if micros != 0 {
                return Err(PyExc::not_implemented_error(
                    "subsecond timezone offsets are not implemented",
                ));
            }
            (
                &rest[..pos],
                Some(sign * (hour as i32 * 3600 + minute as i32 * 60 + second as i32)),
            )
        } else {
            (rest, None)
        };
        let (hour, minute, second, micros) = parse_time(time)?;
        Self::new(date, hour, minute, second, micros, offset)
    }
    pub fn to_value(self) -> Value {
        Value::DateTime(self.isoformat())
    }
}

impl PartialEq for PyDateTime {
    fn eq(&self, other: &Self) -> bool {
        self.compare(*other)
            .is_ok_and(|order| order == Ordering::Equal)
    }
}
impl Eq for PyDateTime {}

impl fmt::Display for PyDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.isoformat_with(' ', "auto").expect("valid timespec"))
    }
}

fn check_offset(offset: Option<i32>) -> PyResult<()> {
    if offset.is_some_and(|v| !(-86400..86400).contains(&v)) {
        return Err(PyExc::value_error(
            "offset must be strictly between -timedelta(hours=24) and timedelta(hours=24)",
        ));
    }
    Ok(())
}

fn parse_digits(text: &str) -> Option<i64> {
    if text.is_empty() || !text.bytes().all(|c| c.is_ascii_digit()) {
        None
    } else {
        text.parse().ok()
    }
}

fn parse_time(text: &str) -> PyResult<(i64, i64, i64, i64)> {
    let invalid = || PyExc::value_error(format!("Invalid isoformat time: {text:?}"));
    if !text.is_ascii() {
        return Err(invalid());
    }
    let (whole, fraction) = text
        .split_once(['.', ','])
        .map_or((text, None), |(a, b)| (a, Some(b)));
    let parts: Vec<&str> = if whole.contains(':') {
        whole.split(':').collect()
    } else {
        match whole.len() {
            2 => vec![whole],
            4 => vec![&whole[..2], &whole[2..]],
            6 => vec![&whole[..2], &whole[2..4], &whole[4..]],
            _ => return Err(invalid()),
        }
    };
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|p| p.len() != 2) {
        return Err(invalid());
    }
    let hour = parse_digits(parts[0]).ok_or_else(invalid)?;
    let minute = parts
        .get(1)
        .map(|s| parse_digits(s).ok_or_else(invalid))
        .transpose()?
        .unwrap_or(0);
    let second = parts
        .get(2)
        .map(|s| parse_digits(s).ok_or_else(invalid))
        .transpose()?
        .unwrap_or(0);
    let microseconds = if let Some(fraction) = fraction {
        if parts.len() != 3 || fraction.is_empty() || !fraction.bytes().all(|c| c.is_ascii_digit())
        {
            return Err(invalid());
        }
        let first = &fraction[..fraction.len().min(6)];
        parse_digits(first).ok_or_else(invalid)? * 10i64.pow(6 - first.len() as u32)
    } else {
        0
    };
    Ok((hour, minute, second, microseconds))
}

/// The oracle's frozen instant, never an implicit wall-clock fallback.
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    instant: PyDateTime,
}

impl Default for Clock {
    fn default() -> Self {
        Self::parse("2026-06-15T12:00:00").expect("default frozen clock")
    }
}

impl Clock {
    pub fn parse(iso: &str) -> PyResult<Self> {
        Ok(Self {
            instant: PyDateTime::fromisoformat(iso)?,
        })
    }
    pub fn today(self) -> PyDate {
        self.instant.date
    }
    pub fn now(self, offset_seconds: Option<i32>) -> PyResult<PyDateTime> {
        if let Some(offset) = offset_seconds {
            // Match oracle.py exactly: replace the parsed tzinfo with UTC
            // before conversion, even when the injected ISO string was aware.
            self.instant
                .replace(None, None, None, None, None, Some(Some(0)))?
                .astimezone(offset)
        } else {
            Ok(self.instant)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinals_weekdays_and_iso_year_boundaries() {
        for (iso, ordinal, weekday, calendar) in [
            ("0001-01-01", 1, 0, (1, 1, 1)),
            ("2000-02-29", 730179, 1, (2000, 9, 2)),
            ("2019-12-30", 737423, 0, (2020, 1, 1)),
            ("2021-01-01", 737791, 4, (2020, 53, 5)),
            ("2026-06-15", 739782, 0, (2026, 25, 1)),
            ("9999-12-31", 3652059, 4, (9999, 52, 5)),
        ] {
            let date = PyDate::fromisoformat(iso).unwrap();
            assert_eq!(date.toordinal(), ordinal, "{iso}");
            assert_eq!(date.weekday(), weekday);
            assert_eq!(date.isocalendar(), calendar);
            assert_eq!(PyDate::fromordinal(ordinal).unwrap(), date);
            assert_eq!(
                PyDate::fromisocalendar(
                    i64::from(calendar.0),
                    i64::from(calendar.1),
                    i64::from(calendar.2)
                )
                .unwrap(),
                date
            );
        }
        assert_eq!(PyDate::new(1900, 2, 29).unwrap_err().kind, "ValueError");
        assert!(PyDate::new(2000, 2, 29).is_ok());
        assert_eq!(
            PyDate::new(1, 1, 1).unwrap().sub_days(1).unwrap_err().kind,
            "OverflowError"
        );
        assert_eq!(
            PyDate::new(9999, 12, 31)
                .unwrap()
                .add_days(1)
                .unwrap_err()
                .kind,
            "OverflowError"
        );
        assert!(PyDate::fromisocalendar(2021, 53, 1).is_err());
    }

    #[test]
    fn date_arithmetic_replace_and_proleptic_calendar() {
        let february = PyDate::new(2024, 2, 28).unwrap();
        assert_eq!(february.add_days(2).unwrap().isoformat(), "2024-03-01");
        assert!(february.replace(None, None, Some(30)).is_err());
        assert_eq!(
            february.difference(PyDate::new(2024, 3, 1).unwrap()).days(),
            -2
        );
        for (year, month, expected) in [
            (0, 2, (1, 29)),
            (-400, 2, (1, 29)),
            (2024, 2, (3, 29)),
            (1900, 2, (3, 28)),
            (2100, 2, (0, 28)),
        ] {
            assert_eq!(monthrange(year, month).unwrap(), expected);
        }
        assert!(monthrange(2026, 13).is_err());
        // Date +/- timedelta uses only normalized days, ignoring sub-day units.
        let minus_one_microsecond = PyTimedelta::new(0, 0, -1).unwrap();
        assert_eq!(
            february.add_timedelta(minus_one_microsecond).unwrap().day(),
            27
        );
        assert_eq!(
            february.sub_timedelta(minus_one_microsecond).unwrap().day(),
            29
        );
    }

    #[test]
    fn timedelta_normalization_rounding_and_overflow() {
        let tiny = PyTimedelta::new(0, 0, -1).unwrap();
        assert_eq!(
            (tiny.days(), tiny.seconds(), tiny.microseconds()),
            (-1, 86399, 999999)
        );
        assert_eq!(tiny.to_string(), "-1 day, 23:59:59.999999");
        assert_eq!(tiny.negated().unwrap().microseconds(), 1);
        assert_eq!(
            PyTimedelta::from_parts(1, 0, 0, 0, 2, 3, 1)
                .unwrap()
                .to_string(),
            "8 days, 3:02:00"
        );
        for (seconds, micros) in [
            (0.0000005, 0),
            (0.0000015, 2),
            (1.0000005, 1000001),
            (1.0000015, 1000001),
            (-1.0000005, -1000001),
        ] {
            assert_eq!(
                PyTimedelta::from_seconds(seconds)
                    .unwrap()
                    .total_microseconds(),
                micros
            );
        }
        assert_eq!(
            PyTimedelta::from_float_parts(14.765294, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0).unwrap(),
            PyTimedelta::new(14, 66121, 401600).unwrap()
        );
        assert_eq!(
            PyTimedelta::from_float_parts(0.0, 0.0, 0.0, 0.0, 0.0, 5.5, 0.0)
                .unwrap()
                .seconds(),
            19800
        );
        assert_eq!(
            PyTimedelta::from_float_parts(0.0, 0.0, 1.0, 0.0005, 0.0, 0.0, 0.0)
                .unwrap()
                .total_microseconds(),
            2
        );
        assert_eq!(
            PyTimedelta::from_float_parts(0.0, 0.0, -1.0, -0.0005, 0.0, 0.0, 0.0)
                .unwrap()
                .total_microseconds(),
            -2
        );
        let max = PyTimedelta::new(999999999, 86399, 999999).unwrap();
        assert!(max.negated().is_err());
        assert!(max.checked_add(PyTimedelta::new(0, 0, 1).unwrap()).is_err());
        assert!(PyTimedelta::from_microseconds(i128::MAX).is_err());
        assert_eq!(
            PyTimedelta::from_seconds(f64::NAN).unwrap_err().kind,
            "ValueError"
        );
    }

    #[test]
    fn iso_parser_extended_basic_week_and_fraction() {
        for (text, expected) in [
            ("20260615T123456", "2026-06-15T12:34:56"),
            (
                "2026-06-15 12:34:56.123456789",
                "2026-06-15T12:34:56.123456",
            ),
            ("2026-06-15T12:34:56,1Z", "2026-06-15T12:34:56.100000+00:00"),
            ("2026-06-15😀12:34+0530", "2026-06-15T12:34:00+05:30"),
            ("2020-W01-1T00:00", "2019-12-30T00:00:00"),
            ("2020-W01T12:30", "2019-12-30T12:30:00"),
            ("2020W01T12:30", "2019-12-30T12:30:00"),
            ("2026-06-15", "2026-06-15T00:00:00"),
            ("2026-06-15T12:34+01:99", "2026-06-15T12:34:00+02:39"),
        ] {
            assert_eq!(
                PyDateTime::fromisoformat(text).unwrap().isoformat(),
                expected,
                "{text}"
            );
        }
        for text in [
            "2026-06-15T24:00",
            "2026-06-15T12:60",
            "2026-06-15T12:34:60",
            "2026-06-15T12.5",
            "2026-06-15T12:34.5",
            "2026-06-15T12:34+24",
            "2026-02-29",
            "ééééé",
        ] {
            assert!(PyDateTime::fromisoformat(text).is_err(), "{text}");
        }
        assert_eq!(
            PyDate::fromisoformat("2020-W01").unwrap().isoformat(),
            "2019-12-30"
        );
    }

    #[test]
    fn datetime_arithmetic_fixed_offsets_and_timespec() {
        let dt = PyDateTime::fromisoformat("2024-03-01T00:00:00.000001+05:30").unwrap();
        let previous = dt
            .sub_timedelta(PyTimedelta::new(0, 0, 2).unwrap())
            .unwrap();
        assert_eq!(previous.isoformat(), "2024-02-29T23:59:59.999999+05:30");
        let utc = dt.astimezone(0).unwrap();
        assert_eq!(utc.isoformat(), "2024-02-29T18:30:00.000001+00:00");
        assert_eq!(utc, dt);
        assert!(utc.difference(dt).unwrap().is_zero());
        assert_eq!(dt.difference(previous).unwrap().total_microseconds(), 2);
        assert_eq!(
            dt.isoformat_with(' ', "milliseconds").unwrap(),
            "2024-03-01 00:00:00.000+05:30"
        );
        let naive = dt
            .replace(None, None, None, None, None, Some(None))
            .unwrap();
        assert_ne!(naive, dt);
        assert_eq!(naive.difference(dt).unwrap_err().kind, "TypeError");
        assert!(naive.astimezone(0).is_err());
        let minimum = PyDateTime::fromisoformat("0001-01-01T00:00:00+01:00").unwrap();
        assert!(minimum.astimezone(0).is_err());
    }

    #[test]
    fn frozen_clock_matches_oracle_relabel_then_convert() {
        let clock = Clock::parse("2026-06-15T23:30:00.123456+05:00").unwrap();
        assert_eq!(clock.today().isoformat(), "2026-06-15");
        assert_eq!(
            clock.now(None).unwrap().isoformat(),
            "2026-06-15T23:30:00.123456+05:00"
        );
        assert_eq!(
            clock.now(Some(0)).unwrap().isoformat(),
            "2026-06-15T23:30:00.123456+00:00"
        );
        assert_eq!(
            clock.now(Some(7200)).unwrap().isoformat(),
            "2026-06-16T01:30:00.123456+02:00"
        );
        assert_eq!(
            Clock::default().now(None).unwrap().isoformat(),
            "2026-06-15T12:00:00"
        );
        assert!(clock.now(Some(86400)).is_err());
    }
}

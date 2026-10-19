//! Терпимый разбор чисел: API может прислать целое, дробное или `null`.

use serde::{Deserialize, Deserializer};

/// Целое число; дробное округляется, `null` превращается в 0.
pub fn int<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(opt_int(d)?.unwrap_or(0))
}

/// Необязательное целое число; дробное округляется.
pub fn opt_int<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    let v = Option::<f64>::deserialize(d)?;
    // Значения из API (счётчики, миллисекунды) далеко от границ i64.
    #[allow(clippy::cast_possible_truncation)]
    Ok(v.filter(|x| x.is_finite()).map(|x| x.round() as i64))
}

/// Дробное число; `null` превращается в 0.
pub fn float<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(d)?.unwrap_or(0.0))
}

/// Строка; `null` превращается в пустую строку.
pub fn string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

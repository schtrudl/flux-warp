//! Provides functionality for reading and writing marker labels in the Audacity label file format,
//! which are tab-separated text files containing start and end times along with labels.
//!
//! Firstly we parse them as string CSV and then read them directly into Duration

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::error::Error;
use std::fmt;
use std::io::{Read, Write};
use std::time::Duration;

#[derive(Serialize, Deserialize)]
struct Label {
    start: Seconds,
    end: Seconds,
    #[serde(default)]
    text: String,
}

struct Seconds(Duration);

impl Serialize for Seconds {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seconds = format!("{}.{:09}", self.0.as_secs(), self.0.subsec_nanos());
        while seconds.ends_with('0') {
            seconds.pop();
        }
        serializer.serialize_str(&seconds)
    }
}

impl<'de> Deserialize<'de> for Seconds {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SecondsVisitor;

        impl Visitor<'_> for SecondsVisitor {
            type Value = Seconds;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a non-negative time in seconds")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                parse_seconds(value).map(Seconds).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(SecondsVisitor)
    }
}

fn parse_seconds(value: &str) -> Result<Duration, &'static str> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 9
    {
        return Err("invalid time value");
    }

    let seconds = whole.parse().map_err(|_| "time value is too large")?;
    let nanos = if fraction.is_empty() {
        0
    } else {
        let padded = format!("{fraction:0<9}");
        padded
            .parse()
            .map_err(|_| "invalid fractional time value")?
    };

    Ok(Duration::new(seconds, nanos))
}

pub(super) fn read<R: Read>(
    reader: R,
) -> Result<Vec<(String, Duration, Duration)>, Box<dyn Error>> {
    let mut csv = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_reader(reader);
    let mut labels = Vec::new();

    for record in csv.records() {
        let record = record?;
        if record.get(0) == Some("\\") {
            continue;
        }

        let label: Label = record.deserialize(None)?;
        labels.push((label.text, label.start.0, label.end.0));
    }

    Ok(labels)
}

pub(super) fn write<W, I>(writer: W, labels: I) -> Result<(), Box<dyn Error>>
where
    W: Write,
    I: IntoIterator<Item = (String, Duration, Duration)>,
{
    let mut csv = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_writer(writer);

    for (text, start, end) in labels {
        csv.serialize(Label {
            start: Seconds(start),
            end: Seconds(end),
            text,
        })?;
    }

    csv.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_extended_frequency_records() {
        let labels = read("1\t1\tpoint\n\\\t100\t200\n".as_bytes()).unwrap();

        assert_eq!(
            labels,
            vec![(
                "point".to_string(),
                Duration::from_secs(1),
                Duration::from_secs(1)
            )]
        );
    }

    #[test]
    fn rejects_invalid_times() {
        assert!(read("not-a-time\t1\tlabel\n".as_bytes()).is_err());
    }
}

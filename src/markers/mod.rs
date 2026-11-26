use std::fs::File;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkerType {
    Point,
    RegionStart,
    RegionEnd,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeMarker {
    name: String,
    t: Duration,
    ty: MarkerType,
}

impl TimeMarker {
    fn from(name: &str, start: Duration, end: Duration) -> (Self, Option<Self>) {
        if start == end {
            (
                Self {
                    name: name.to_string(),
                    t: start,
                    ty: MarkerType::Point,
                },
                None,
            )
        } else {
            (
                Self {
                    name: name.to_string(),
                    t: start,
                    ty: MarkerType::RegionStart,
                },
                Some(Self {
                    name: name.to_string(),
                    t: end,
                    ty: MarkerType::RegionEnd,
                }),
            )
        }
    }
}

struct IntoTimeMarkersIterator<I> {
    cur: Option<TimeMarker>,
    iter: I,
}

impl<I> IntoTimeMarkersIterator<I> {
    const fn new(iter: I) -> Self {
        Self { cur: None, iter }
    }
}

impl<I: Iterator<Item = (String, Duration, Duration)>> Iterator for IntoTimeMarkersIterator<I> {
    type Item = TimeMarker;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.cur.take().or_else(|| {
            self.iter.next().map(|(name, start, end)| {
                let (tm, cur) = TimeMarker::from(&name, start, end);
                self.cur = cur;
                tm
            })
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.iter.size_hint();
        (
            lower,                // all are Points
            upper.map(|x| x * 2), // all are Intervals
        )
    }
}

pub fn read_marker_file(path: &Path) -> Result<Vec<TimeMarker>, Box<dyn std::error::Error>> {
    match path.extension() {
        Some(ext) if ext == "vtt" => {
            let file = File::open(path)?;
            let web_vtt = vtt::WebVtt::from_reader(file)?;
            Ok(IntoTimeMarkersIterator::new(web_vtt.cues.iter().map(|c| {
                (
                    c.payload.clone(),
                    c.start.as_duration(),
                    c.end.as_duration(),
                )
            }))
            .collect())
        }
        Some(ext) if ext == "srt" => {
            fn srt_timestamp_to_time_marker(t: &srtlib::Timestamp) -> Duration {
                let (hh, mm, ss, millis) = t.get();
                Duration::from_hours(hh as u64)
                    + Duration::from_mins(mm as u64)
                    + Duration::from_secs(ss as u64)
                    + Duration::from_millis(millis as u64)
            }
            let subtitles = srtlib::Subtitles::parse_from_file(path, None)?;
            Ok(IntoTimeMarkersIterator::new(subtitles.into_iter().map(|x| {
                (
                    x.text,
                    srt_timestamp_to_time_marker(&x.start_time),
                    srt_timestamp_to_time_marker(&x.end_time),
                )
            }))
            .collect())
        }
        Some(ext) if ext == "txt" => {
            todo!("Handle TXT marker file");
        }
        _ => Err(format!("Unsupported marker file {}", path.display()).into()),
    }
}

struct FromTimeMarkersIterator<I> {
    iter: I,
}

impl<I> FromTimeMarkersIterator<I> {
    const fn new(iter: I) -> Self {
        Self { iter }
    }
}

impl<'a, I: Iterator<Item = &'a TimeMarker>> Iterator for FromTimeMarkersIterator<I> {
    type Item = (&'a str, Duration, Duration);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|tm| {
            let end = if tm.ty == MarkerType::RegionStart {
                let next_tm = self.iter.next().expect("Expected interval end marker");
                assert!(
                    next_tm.ty == MarkerType::RegionEnd,
                    "Expected interval end marker"
                );
                next_tm.t
            } else {
                tm.t
            };
            (tm.name.as_str(), tm.t, end)
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.iter.size_hint();
        (lower / 2, upper)
    }
}

pub fn write_marker_file(
    path: &Path,
    markers: &[TimeMarker],
) -> Result<(), Box<dyn std::error::Error>> {
    match path.extension() {
        Some(ext) if ext == "vtt" => {
            let web_vtt = vtt::WebVtt {
                header: vtt::VttHeader::default(),
                cues: FromTimeMarkersIterator::new(markers.iter())
                    .map(|(name, start, end)| vtt::VttCue {
                        identifier: None,
                        start: vtt::VttTimestamp::new(start),
                        end: vtt::VttTimestamp::new(end),
                        settings: None,
                        payload: name.to_string(),
                    })
                    .collect(),
            };

            // deserializer of vtt is wrong
            let cues = web_vtt
                .cues
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n\n");

            std::fs::write(path, format!("WEBVTT\n\n{cues}"))?;
            Ok(())
        }
        Some(ext) if ext == "srt" => {
            srtlib::Subtitles::new_from_vec(
                FromTimeMarkersIterator::new(markers.iter())
                    .map(|(name, start, end)| srtlib::Subtitle {
                        num: 0,
                        start_time: srtlib::Timestamp::from_milliseconds(start.as_millis() as u32),
                        end_time: srtlib::Timestamp::from_milliseconds(end.as_millis() as u32),
                        text: name.to_string(),
                    })
                    .collect(),
            )
            .write_to_file(path, None)?;
            Ok(())
        }
        Some(ext) if ext == "txt" => {
            todo!("Handle TXT marker file");
        }
        _ => Err(format!("Unsupported marker file {}", path.display()).into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn example_markers() -> [TimeMarker; 4] {
        [
            TimeMarker {
                name: "1".to_string(),
                t: Duration::from_millis(209),
                ty: MarkerType::Point,
            },
            TimeMarker {
                name: "23".to_string(),
                t: Duration::from_millis(917),
                ty: MarkerType::RegionStart,
            },
            TimeMarker {
                name: "23".to_string(),
                t: Duration::from_millis(2287),
                ty: MarkerType::RegionEnd,
            },
            TimeMarker {
                name: "4".to_string(),
                t: Duration::from_millis(3251),
                ty: MarkerType::Point,
            },
        ]
    }

    #[test]
    fn test_unsupported_read_marker_file() {
        let path = Path::new("file.png");
        assert!(read_marker_file(path).is_err());
    }

    #[test]
    fn test_read_marker_file_vtt() {
        let path = Path::new("./src/markers/ex_markers.vtt");
        assert_eq!(read_marker_file(path).unwrap(), example_markers());
    }

    #[test]
    fn test_read_marker_file_srt() {
        let path = Path::new("./src/markers/ex_markers.srt");
        assert_eq!(read_marker_file(path).unwrap(), example_markers());
    }

    #[test]
    fn test_unsupported_write_marker_file() {
        let path = Path::new("ex_markers.png");
        assert!(write_marker_file(path, &example_markers()).is_err());
    }

    #[test]
    fn test_write_srt_marker_file() {
        let path = tempfile::NamedTempFile::new()
            .unwrap()
            .path()
            .with_extension("srt");
        assert!(write_marker_file(&path, &example_markers()).is_ok());
        assert_eq!(read_marker_file(&path).unwrap(), example_markers());
    }

    #[test]
    fn test_write_vtt_marker_file() {
        let path = tempfile::NamedTempFile::new()
            .unwrap()
            .path()
            .with_extension("vtt");
        assert!(write_marker_file(&path, &example_markers()).is_ok());
        assert_eq!(read_marker_file(&path).unwrap(), example_markers());
    }
}

#![allow(dead_code)]

use std::fmt;
use std::str;
/// A dogstatsd metric is stored internally as the original line of bytes that went over UDP.
///
/// Parsing methods are added as needed, and they operate lazily.
///
/// We aim to avoid emitting parsing errors or converting the metric into any "AST", as statsd has
/// a lot of proprietary extensions and any conversion into any AST would be lossy. This is only
/// possible to some extent, but at the very least, running no middlewares should not lose any data
/// at all and should be as fast as possible.
///
/// Reference for the format we care about:
/// https://docs.datadoghq.com/developers/dogstatsd/datagram_shell/?tab=metrics
///
/// ```text
/// <METRIC_NAME>:<VALUE>|<TYPE>|@<SAMPLE_RATE>|#<TAG_KEY_1>:<TAG_VALUE_1>,<TAG_2>
/// ```
#[derive(Clone, PartialEq)]
pub struct Metric {
    // TODO: use global arena to allocate strings?
    //
    pub raw: Vec<u8>,
    tags_pos: Option<(usize, usize)>,
}

impl fmt::Debug for Metric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Metric")
            .field("raw", &str::from_utf8(&self.raw))
            .finish()
    }
}

#[derive(PartialEq)]
pub struct MetricTag<'a> {
    // Tags are always represented as a byte array, and may have a name and value if their format matches
    // our expectations.
    pub raw: &'a [u8],
    pub name_value_sep_pos: Option<usize>,
}

impl<'a> MetricTag<'a> {
    pub fn new(bytes: &[u8]) -> MetricTag {
        MetricTag {
            raw: bytes,
            name_value_sep_pos: bytes.iter().position(|&b| b == b':'),
        }
    }

    pub fn name(&self) -> Option<&[u8]> {
        self.name_value_sep_pos.map(|i| &self.raw[..i])
    }

    pub fn value(&self) -> Option<&[u8]> {
        self.name_value_sep_pos.map(|i| &self.raw[i + 1..])
    }
}

impl<'a> fmt::Debug for MetricTag<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.name_value_sep_pos.is_none() {
            f.debug_struct("MetricTag")
                .field("bytes", &str::from_utf8(self.raw))
                .finish()
        } else {
            f.debug_struct("MetricTag")
                .field("name", &str::from_utf8(self.name().unwrap()))
                .field("value", &str::from_utf8(self.value().unwrap()))
                .finish()
        }
    }
}

pub struct MetricTagIterator<'a> {
    pub remaining_tags: Option<&'a [u8]>,
}

impl<'a> Iterator for MetricTagIterator<'a> {
    type Item = MetricTag<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let remaining_tags = self.remaining_tags?;
        let mut tag_pos_iter = remaining_tags.iter();
        let next_tag_sep_pos = tag_pos_iter.position(|&b| b == b',');

        return if let Some(tag_sep_pos) = next_tag_sep_pos {
            // Got a tag and more tags remain
            let tag = MetricTag::new(&remaining_tags[..tag_sep_pos]);
            self.remaining_tags = Some(&remaining_tags[tag_sep_pos + 1..]);

            Some(tag)
        } else {
            // Got a tag and no more tags remain
            let tag = MetricTag::new(remaining_tags);
            self.remaining_tags = None;

            Some(tag)
        };
    }
}

impl Metric {
    pub fn new(raw: Vec<u8>) -> Self {
        let tags_pos = raw.windows(2).position(|x| x == [b'|', b'#']).map(|i| {
            (
                i + 2,
                raw.iter()
                    .skip(i + 2)
                    .position(|&x| x == b'|')
                    .map(|x| x + i + 2)
                    .unwrap_or(raw.len()),
            )
        });
        Metric { raw, tags_pos }
    }

    pub fn name(&self) -> Option<&[u8]> {
        self.raw.splitn(2, |&x| x == b':').next()
    }

    pub fn tags(&self) -> Option<&[u8]> {
        self.tags_pos.map(|(i, j)| &self.raw[i..j])
    }

    pub fn tags_iter(&self) -> MetricTagIterator {
        MetricTagIterator {
            remaining_tags: self.tags(),
        }
    }

    pub fn set_tags(&mut self, tags: &[u8]) {
        if tags.is_empty() {
            if let Some((i, j)) = self.tags_pos {
                self.raw.drain(i - 2..j);
                self.tags_pos = None;
            }
        } else {
            match self.tags_pos {
                Some((i, j)) => {
                    self.raw.splice(i..j, tags.iter().cloned());
                    self.tags_pos = Some((i, i + tags.len()));
                }
                None => {
                    self.raw.extend(b"|#");
                    let start = self.raw.len();
                    self.tags_pos = Some((start, start + tags.len()));
                    self.raw.extend(tags);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_tags() {
        let metric = Metric::new(b"users.online:1|c|@0.5".to_vec());
        assert_eq!(metric.tags(), None);
        assert_eq!(metric.tags_iter().collect::<Vec<MetricTag>>(), []);
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(metric.raw, b"users.online:1|c|@0.5");
    }

    #[test]
    fn some_tags_end() {
        let metric = Metric::new(b"users.online:1|c|@0.5|#instance:foobar,country:china".to_vec());
        assert_eq!(metric.tags().unwrap(), b"instance:foobar,country:china");
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(
            metric.raw,
            b"users.online:1|c|@0.5|#instance:foobar,country:china"
        );
    }

    #[test]
    fn some_tags_middle() {
        let metric = Metric::new(
            b"users.online:1|c|@0.5|#instance:foobar,country:china|T1692653389".to_vec(),
        );
        assert_eq!(metric.tags().unwrap(), b"instance:foobar,country:china");
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(
            metric.raw,
            b"users.online:1|c|@0.5|#instance:foobar,country:china|T1692653389"
        );
    }

    #[test]
    fn add_none_tags_to_none() {
        let mut metric = Metric::new(b"users.online:1|c|@0.5".to_vec());

        metric.set_tags(b"");
        assert_eq!(metric.tags(), None);
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(metric.raw, b"users.online:1|c|@0.5");
    }

    #[test]
    fn add_some_tags_to_none() {
        let mut metric = Metric::new(b"users.online:1|c|@0.5".to_vec());

        metric.set_tags(b"country:japan");
        assert_eq!(metric.tags().unwrap(), b"country:japan");
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(metric.raw, b"users.online:1|c|@0.5|#country:japan");
    }

    #[test]
    fn remove_tags_end() {
        let mut metric =
            Metric::new(b"users.online:1|c|@0.5|#instance:foobar,country:china".to_vec());

        metric.set_tags(b"");
        assert_eq!(metric.tags(), None);
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(metric.raw, b"users.online:1|c|@0.5");
    }

    #[test]
    fn remove_tags_middle() {
        let mut metric = Metric::new(
            b"users.online:1|c|@0.5|#instance:foobar,country:china|T1692653389".to_vec(),
        );

        metric.set_tags(b"");
        assert_eq!(metric.tags(), None);
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(metric.raw, b"users.online:1|c|@0.5|T1692653389");
    }

    #[test]
    fn change_tags_end() {
        let mut metric =
            Metric::new(b"users.online:1|c|@0.5|#instance:foobar,country:china".to_vec());

        metric.set_tags(b"country:japan");
        assert_eq!(metric.tags().unwrap(), b"country:japan");
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(metric.raw, b"users.online:1|c|@0.5|#country:japan");
    }

    #[test]
    fn change_tags_middle() {
        let mut metric = Metric::new(
            b"users.online:1|c|@0.5|#instance:foobar,country:china|T1692653389".to_vec(),
        );

        metric.set_tags(b"country:japan");
        assert_eq!(metric.tags().unwrap(), b"country:japan");
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(
            metric.raw,
            b"users.online:1|c|@0.5|#country:japan|T1692653389"
        );
    }

    #[test]
    fn tag_iter() {
        let metric =
            Metric::new(b"users.online:1|c|@0.5|#instance:foobar,ohyeah,,country:china,".to_vec());

        let mut tag_iter = metric.tags_iter();

        {
            let first = tag_iter.next().unwrap();
            assert_eq!(first.name(), Some(b"instance".as_slice()));
            assert_eq!(first.value(), Some(b"foobar".as_slice()));
            assert_eq!(first.raw, b"instance:foobar".as_slice());
        }

        {
            let second = tag_iter.next().unwrap();
            assert_eq!(second.name(), None);
            assert_eq!(second.value(), None);
            assert_eq!(second.raw, b"ohyeah".as_slice());
        }

        {
            let third = tag_iter.next().unwrap();
            assert_eq!(third.name(), None);
            assert_eq!(third.value(), None);
            assert_eq!(third.raw, b"".as_slice());
        }

        {
            let fourth = tag_iter.next().unwrap();
            assert_eq!(fourth.name(), Some(b"country".as_slice()));
            assert_eq!(fourth.value(), Some(b"china".as_slice()));
            assert_eq!(fourth.raw, b"country:china".as_slice());
        }

        {
            let fifth = tag_iter.next().unwrap();
            assert_eq!(fifth.name(), None);
            assert_eq!(fifth.value(), None);
            assert_eq!(fifth.raw, b"".as_slice());
        }

        assert_eq!(tag_iter.next(), None);
    }
}

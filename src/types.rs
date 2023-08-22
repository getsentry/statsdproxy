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
#[derive(Clone, Debug)]
pub struct Metric {
    // TODO: use global arena to allocate strings?
    //
    pub raw: Vec<u8>,
    tags_pos: Option<(usize, usize)>,
}

#[derive(PartialEq)]
pub struct MetricTag<'a> {
    pub name: &'a[u8],
    pub value: &'a[u8],
}

impl<'a> MetricTag<'a> {
    pub fn new(bytes: &[u8]) -> MetricTag {
        let parts: Vec<&[u8]> = bytes.split(|&b| b == b':').collect();
        assert!(parts.len() == 2);

        MetricTag { name: parts[0], value: parts[1] }
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(self.name);
        bytes.push(b':');
        bytes.extend(self.value);

        bytes
    }
}

impl<'a> fmt::Debug for MetricTag<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MetricTag")
            .field("name", &str::from_utf8(self.name))
            .field("value", &str::from_utf8(self.value))
            .finish()
    }
}

pub struct MetricTagIterator<'a> {
    pub metric: &'a Metric,
    pub next_tag_pos: Option<usize>,
}

impl<'a> Iterator for MetricTagIterator<'a> {
    type Item = MetricTag<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_tag_pos.is_none() {
            return None;
        }
        
        let next_tag_pos = self.next_tag_pos.unwrap();
        let mut tag_pos_iter = self.metric.raw[next_tag_pos..].iter();

        let next_name_value_sep_pos = tag_pos_iter.position(|&b| b == b':');
        let next_metric_sep_pos = tag_pos_iter.position(|&b| b == b',');

        return match (next_name_value_sep_pos, next_metric_sep_pos) {
            // Got a tag and more tags remain
            (Some(x), Some(y)) => {
                let tag = MetricTag {
                    name: &self.metric.raw[next_tag_pos..next_tag_pos + x], 
                    value: &self.metric.raw[next_tag_pos + x + 1..next_tag_pos + x + y + 1] };
                
                // In total, consumed two separator characters plus the characters for the name and value,
                // so advance the pointer by that amount.
                self.next_tag_pos = Some(next_tag_pos + x + y + 2);
                
                Some(tag)
            }
            
            // Got a tag and no more tags remain
            (Some(x), None) => {
                let tag = MetricTag  {
                    name: &self.metric.raw[next_tag_pos..next_tag_pos + x],
                    value: &self.metric.raw[next_tag_pos + x + 1..]
                };
                self.next_tag_pos = None;

                Some(tag)
            }
            
            // No more tags
            (None, ..) => {
                None
            }
        }
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
        MetricTagIterator { metric: &self, next_tag_pos: self.tags_start_pos() }
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
            Metric::new(b"users.online:1|c|@0.5|#instance:foobar,country:china".to_vec());
        
        let mut tag_iter = metric.tags_iter();

        assert_eq!(tag_iter.next(), Some(MetricTag { name: b"instance".as_slice(), value: b"foobar".as_slice() }));
        assert_eq!(tag_iter.next(), Some(MetricTag { name: b"country".as_slice(), value: b"china".as_slice() }));
        assert_eq!(tag_iter.next(), None);
    }
}

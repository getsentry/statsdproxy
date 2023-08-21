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
pub struct Metric {
    // TODO: use global arena to allocate strings?
    //
    pub raw: Vec<u8>,
}

impl Metric {
    pub fn new(raw: Vec<u8>) -> Self {
        Metric { raw }
    }

    pub fn name(&self) -> Option<&[u8]> {
        self.raw.splitn(2, |&x| x == b':').next()
    }

    fn tags_start_pos(&self) -> Option<usize> {
        self.raw
            .iter()
            .enumerate()
            .rfind(|(_, &x)| x == b'#')
            .map(|(i, _)| i + 1)
    }

    pub fn tags(&self) -> Option<&[u8]> {
        Some(&self.raw[self.tags_start_pos()?..])
    }

    pub fn set_tags(&mut self, tags: &[u8]) {
        if let Some(tags) = self.tags() {
            self.raw
                .truncate(tags.as_ptr() as usize - self.raw.as_slice().as_ptr() as usize);
        } else {
            self.raw.extend(b"|#");
        }

        self.raw.extend(tags);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let mut metric =
            Metric::new(b"users.online:1|c|@0.5|#instance:foobar,country:china".to_vec());
        assert_eq!(metric.tags().unwrap(), b"instance:foobar,country:china");
        assert_eq!(metric.name().unwrap(), b"users.online");

        metric.set_tags(b"country:japan");
        assert_eq!(metric.tags().unwrap(), b"country:japan");
        assert_eq!(metric.name().unwrap(), b"users.online");
    }

    #[test]
    fn set_tags() {
        let mut metric = Metric::new(b"users.online:1|c|@0.5".to_vec());
        assert_eq!(metric.tags(), None);

        metric.set_tags(b"");
        assert_eq!(metric.tags(), Some(b"".as_slice()));

        metric.set_tags(b"country:japan");
        assert_eq!(metric.tags(), Some(b"country:japan".as_slice()));

        assert_eq!(
            metric.raw,
            b"users.online:1|c|@0.5|#country:japan".as_slice()
        );
    }
}

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
    tags_pos: Option<(usize, usize)>,
}

impl Metric {
    pub fn new(raw: Vec<u8>) -> Self {
        let tags_pos = raw.windows(2).position(|x| x == &[b'|', b'#']).map(|i| {
            (
                i + 2,
                raw.iter().skip(i + 2).position(|&x| x == b'|').unwrap_or(raw.len())
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

    pub fn set_tags(&mut self, tags: &[u8]) {
        if tags.is_empty() {
            if let Some((i, j)) = self.tags_pos {
                self.raw.drain(i - 2..j);
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
        let metric =
            Metric::new(b"users.online:1|c|@0.5".to_vec());
        assert_eq!(metric.tags(), None);
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(
            metric.raw,
            b"users.online:1|c|@0.5"
        );
    }

    #[test]
    fn some_tags_end() {
        let metric =
            Metric::new(b"users.online:1|c|@0.5|#instance:foobar,country:china".to_vec());
        assert_eq!(metric.tags().unwrap(), b"instance:foobar,country:china");
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(
            metric.raw,
            b"users.online:1|c|@0.5|#instance:foobar,country:china"
        );
    }

    #[test]
    fn some_tags_middle() {
        let metric =
            Metric::new(b"users.online:1|c|@0.5|#instance:foobar,country:china|T1692653389".to_vec());
        assert_eq!(metric.tags().unwrap(), b"instance:foobar,country:china");
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(
            metric.raw,
            b"users.online:1|c|@0.5|#instance:foobar,country:china|T1692653389"
        );
    }

    #[test]
    fn add_none_tags_to_none() {
        let mut metric =
            Metric::new(b"users.online:1|c|@0.5".to_vec());

        metric.set_tags(b"");
        assert_eq!(metric.tags(), None);
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(
            metric.raw,
            b"users.online:1|c|@0.5"
        );
    }

    #[test]
    fn add_tags_to_none() {
        let mut metric =
            Metric::new(b"users.online:1|c|@0.5".to_vec());

        metric.set_tags(b"country:japan");
        assert_eq!(metric.tags().unwrap(), b"country:japan");
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(
            metric.raw,
            b"users.online:1|c|@0.5|#country:japan"
        );
    }

    #[test]
    fn remove_tags_end() {
        let mut metric = Metric::new(b"users.online:1|c|@0.5|#instance:foobar,country:china".to_vec());

        metric.set_tags(b"");
        assert_eq!(metric.tags(), None);
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(
            metric.raw,
            b"users.online:1|c|@0.5"
        );
    }

    #[test]
    fn remove_tags_middle() {
        let mut metric = Metric::new(b"users.online:1|c|@0.5|#instance:foobar,country:china|T1692653389".to_vec());

        metric.set_tags(b"");
        assert_eq!(metric.tags(), None);
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(
            metric.raw,
            b"users.online:1|c|@0.5|T1692653389"
        );
    }

    #[test]
    fn change_tags_end() {
        let mut metric =
            Metric::new(b"users.online:1|c|@0.5|#instance:foobar,country:china".to_vec());

        metric.set_tags(b"country:japan");
        assert_eq!(metric.tags().unwrap(), b"country:japan");
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(
            metric.raw,
            b"users.online:1|c|@0.5|#country:japan"
        );
    }

    #[test]
    fn change_tags_middle() {
        let mut metric =
            Metric::new(b"users.online:1|c|@0.5|#instance:foobar,country:china|T1692653389".to_vec());

        metric.set_tags(b"country:japan");
        assert_eq!(metric.tags().unwrap(), b"country:japan");
        assert_eq!(metric.name().unwrap(), b"users.online");
        assert_eq!(
            metric.raw,
            b"users.online:1|c|@0.5|#country:japan|T1692653389"
        );
    }
}

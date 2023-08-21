pub struct Metric {
    // TODO: we should actually start parsing metrics, but I think it could be done lazily only if
    // necessary
    //
    // for example, tags do not need to be parsed and a hashmap/btreemap allocated if no middleware
    // is looking at tags
    //
    // it would also be neat if we could support "trailing characters" on the payload, so we can
    // transparently deal with statsd extensions we don't understand
    //
    // prior art: Rust URL crate
    //
    // TODO: use global arena to allocate strings?
    pub raw: Vec<u8>,
}

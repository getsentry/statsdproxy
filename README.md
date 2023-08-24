# statsdproxy

A proxy for transforming, pre-aggregating and routing statsd metrics, like
[Veneur](https://github.com/stripe/veneur), [Vector](https://vector.dev/) or
[Brubeck](https://github.com/github/brubeck).

Currently supports the following transformations:

* Deny- or allow-listing of specific tag keys or metric names
* Adding hardcoded tags to all metrics
* Basic cardinality limiting, tracking the number of distinct tag values per
  key or the number of overall timeseries (=combinations of metrics and tags).

See `example.yml` for details.

A major goal is minimal overhead and **no loss of information** due to
unnecessarily strict parsing. Statsdproxy intends to orient itself around
[dogstatsd](https://docs.datadoghq.com/developers/dogstatsd/datagram_shell/?tab=metrics)
protocol but should gracefully degrade for other statsd dialects, in that those
metrics and otherwise unparseable bytes will be forwarded as-is.

**This is not a Sentry product**, not deployed in any sort of production
environment, but a side-project done during Hackweek.


## Basic usage

1. Run a "statsd server" on port 8081 that just prints metrics

   ```
   socat -u UDP-RECVFROM:8081,fork SYSTEM:"cat; echo"
   ```

2. Copy `example.yaml` to `config.yaml` and edit it
3. Run statsdproxy to read metrics from port 8080, transform them using the
   middleware in `config.yaml` and forward the new metrics to port 8081:

   ```
   cargo run --release -- --listen 127.0.0.1:8080 --upstream 127.0.0.1:8081 -c config.yaml
   ```

5. Send metrics to statsdproxy:

   ```
   yes 'users.online:1|c|@0.5' | nc -u 127.0.0.1 8080
   ```

4. You should see new metrics in `socat` with your middlewares applied.

## Usage with Snuba

Patch the following settings in `snuba/settings/__init__.py`:

```python
DOGSTATSD_HOST = "127.0.0.1"
DOGSTATSD_PORT = "8080"
```

This will send metrics to port 8080.

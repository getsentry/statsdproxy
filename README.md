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

## Processing model

This is the processing model used by the provided server. It should be respected
by any usage of this software as a library.

* The server receives metrics as bytes over udp, either singly or several joined
  with `\n`.
* For every metric received, the server invokes the `poll` method of the topmost
  middleware.
    * The middleware may use this invocation to do any needed internal
      bookkeeping.
    * The middleware should then invoke the `poll` method of the next
      middleware, if any.
* Once `poll` returns, the server invokes the `submit` method of the topmost
  middleware with a mutable reference to the current metric.
    * The middleware should process the metric.
        * If processing was successful, and if appropriate to its function
          (eg. a metric aggregator might hold onto metrics), the middleware
          should `submit` the processed metric to the next middleware, returning
          the result of this call.
        * If processing was unsuccessful (eg. unknown StatsD dialect), the
          unchanged metric should be treated as the processed metric, and passed
          on or held as above.
        * If a middleware becomes unable to handle more metrics during
          processing, such that it cannot handle the current metric, it should
          return `Overloaded`.
    * If an overload is indicated, the server shall pause (TODO: how long)
      before calling `submit` again with the same metric. (If an overload is
      indicated too many times, maybe drop the metric?)
* Separately, if no metric is received by the server for 1 second, it will
  invoke the `poll` method of the topmost middleware. This invocation of `poll`
  should be handled the same as above.

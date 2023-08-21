# statsdproxy

A proxy for transforming, pre-aggregating and routing statsd metrics.

This is not a Sentry product, not deployed in any sort of production
environment, but a side-project done during Hackweek.

If you want a production-ready thing like this, consider using
[Vector](https://vector.dev/).

It intends to implement
[dogstatsd](https://docs.datadoghq.com/developers/dogstatsd/datagram_shell/?tab=metrics)
protocol but should gracefully degrade for other statsd dialects.

## Usage with Snuba

Patch the following settings in `snuba/settings/__init__.py`:

```python
DOGSTATSD_HOST = "127.0.0.1"
DOGSTATSD_PORT = "8080"
```

Listen on port 8080 using `statsdproxy`:

```bash
cargo run --release -- --listen 127.0.0.1:8080 --upstream 127.0.0.1:8081
```

Run `socat` to dump all metrics to stdout:

```bash
socat -u UDP-RECVFROM:8081,fork SYSTEM:"cat; echo"
```

Run `snuba devserver` to see metrics.

## Netcat

You can also use `nc -ul 8081` for testing, but it will not work with multiple
processes (such as the many spawned with `snuba devserver`) and the output is
not as pretty as with socat (as sometimes multiple metrics are printed on a
single line).

It is however useful for benchmarking, such as:

```bash
nc -ul 8081 | pv > /dev/null
cargo run --release -- --listen 127.0.0.1:8080 --upstream 127.0.0.1:8081
yes 'users.online:1|c|@0.5|#country:china' | nc -u 127.0.0.1 8080
```

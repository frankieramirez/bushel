# telemetry strip — static dumps

## 55×20 logs, 3-row strip

stacked  rail 55×9  detail 55×9  collapse=tight  cap=36  stack<80  log_cols=53  active=containers

```
 bushel   1 containers · 2 images · 3 volumes
╭ containers 8 ───────────────────────────────────────╮
│● redis              1.2%    48M  redis:7            │
│● postgres           4.8%   256M  postgres:16        │
│● caddy              0.3%    12M  caddy:2            │
│● bushel-smoke       0.1%     4M  alpine:latest      │
│● github-actions-ru 12.4%   512M  ghcr.io/example/wor│
╰─────────────────────────────────────────────────────╯
 2 images 5
 3 volumes 3
╭ detail ─────────────────────────────────────────────╮
│cpu  33.0% ▂▂▂▂▂▂▂▆▂▂▂▂▂▂▂▂▂▂▂▂▃▃▃▃▃▃▃▃▃▃▂▂▂▂▂▂▂▂▂▂▂▂│
│mem    42% ▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃│
│net ↑11.7K/s ↓3.9K/s   dsk r 2.0K/s  w 7.8K/s        │
│2026-08-24T18:02:11.441Z INFO  postgres  checkpoint c│
│2026-08-24T18:02:12.018Z INFO  redis     replica 192.│
│2026-08-24T18:02:12.441Z INFO  caddy     192.168.64.1│
│2026-08-24T18:02:13.102Z WARN  postgres  could not se│
╰ 53 cols · strip 3 · last 53s of 5m ─────────────────╯
 1/2/3 expand  j/k move  / filter  f zoom
```

## 55×20 inspect, 3-row strip

stacked  rail 55×9  detail 55×9  collapse=tight  cap=36  stack<80  log_cols=53  active=containers

```
 bushel   1 containers · 2 images · 3 volumes
╭ containers 8 ───────────────────────────────────────╮
│● redis              1.2%    48M  redis:7            │
│● postgres           4.8%   256M  postgres:16        │
│● caddy              0.3%    12M  caddy:2            │
│● bushel-smoke       0.1%     4M  alpine:latest      │
│● github-actions-ru 12.4%   512M  ghcr.io/example/wor│
╰─────────────────────────────────────────────────────╯
 2 images 5
 3 volumes 3
╭ detail ─────────────────────────────────────────────╮
│cpu  33.0% ▂▂▂▂▂▂▂▆▂▂▂▂▂▂▂▂▂▂▂▂▃▃▃▃▃▃▃▃▃▃▂▂▂▂▂▂▂▂▂▂▂▂│
│mem    42% ▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃│
│net ↑11.7K/s ↓3.9K/s   dsk r 2.0K/s  w 7.8K/s        │
│{                                                    │
│  "id": "redis",                                     │
│  "image": "postgres:16",                            │
│  "state": "running"                                 │
╰ 53 cols · strip 3 · last 53s of 5m ─────────────────╯
 1/2/3 expand  j/k move  / filter  f zoom
```

## 100×30 logs, 3-row strip

beside  rail 36×27  detail 64×27  collapse=roomy  cap=36  stack<80  log_cols=62  active=containers

```
 bushel   1 containers · 2 images · 3 volumes

╭ containers 8 ────────────────────╮╭ detail ──────────────────────────────────────────────────────╮
│name                 cpu    mem   ││  Logs [l]  │  Inspect [i]                                    │
│● redis               1.2%    48M ││cpu  33.0% ▁▁▁▁▁▁▂▂▂▂▂▂▂▂▂▂▆▂▂▂▂▂▂▂▂▂▂▂▂▃▃▃▃▃▃▃▃▃▃▂▂▂▂▂▂▂▂▂▂▂▂│
│● postgres            4.8%   256M ││mem    42% ▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃│
│● caddy               0.3%    12M ││net ↑11.7K/s ↓3.9K/s   dsk r 2.0K/s  w 7.8K/s                 │
│● bushel-smoke        0.1%     4M ││2026-08-24T18:02:11.441Z INFO  postgres  checkpoint complete: │
│● github-actions-run 12.4%   512M ││2026-08-24T18:02:12.018Z INFO  redis     replica 192.168.64.4:│
│○ worker-a            0.0%     0M ││2026-08-24T18:02:12.441Z INFO  caddy     192.168.64.1 - GET /h│
│○ worker-b            0.0%     0M ││2026-08-24T18:02:13.102Z WARN  postgres  could not serialize a│
│○ old-migrate         0.0%     0M ││2026-08-24T18:02:13.880Z INFO  runner    job 1842 queued: buil│
│                                  ││2026-08-24T18:02:14.201Z INFO  redis     10000 changes in 60 s│
│                                  ││2026-08-24T18:02:14.990Z INFO  postgres  statement: SELECT * F│
│                                  ││2026-08-24T18:02:15.441Z INFO  caddy     192.168.64.8 - POST /│
│                                  ││2026-08-24T18:02:16.002Z INFO  runner    cloning github.com/fr│
╰──────────────────────────────────╯│2026-08-24T18:02:16.771Z ERROR postgres  FATAL:  remaining con│
╭ images 5 ────────────────────────╮│2026-08-24T18:02:17.110Z INFO  redis     DB saved on disk     │
│docker.io/library/postgres:16     ││2026-08-24T18:02:17.880Z INFO  caddy     logger: flushed 128 e│
│docker.io/library/redis:7         ││2026-08-24T18:02:18.441Z INFO  postgres  automatic vacuum of t│
│docker.io/library/caddy:2         ││2026-08-24T18:02:19.002Z INFO  runner    cargo test --offline │
│docker.io/library/alpine:latest   ││2026-08-24T18:02:19.660Z INFO  redis     client closed connect│
│ghcr.io/example/worker:1.4.2      ││2026-08-24T18:02:20.101Z INFO  postgres  duration: 812.441 ms │
╰──────────────────────────────────╯│2026-08-24T18:02:20.880Z INFO  caddy     tls: remaining 2 cert│
╭ volumes 3 ───────────────────────╮│2026-08-24T18:02:21.441Z INFO  runner    test engine::poll::co│
│pg-data                   in use  ││2026-08-24T18:02:22.018Z WARN  postgres  log line too long to │
│redis-data                in use  ││2026-08-24T18:02:22.441Z INFO  redis     1.2µs per op, 800k op│
│leftover                  -       ││── following (F to pause) ──                                  │
╰──────────────────────────────────╯╰ 62 cols · strip 3 · last 62s of 5m ──────────────────────────╯
 1/2/3 expand  j/k move  enter focus  / filter  s strip  a ascii  ? help
```

## 200×50 logs, 3-row strip

beside  rail 36×47  detail 164×47  collapse=roomy  cap=36  stack<80  log_cols=162  active=containers

```
 bushel   1 containers · 2 images · 3 volumes

╭ containers 8 ────────────────────╮╭ detail ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
│name                 cpu    mem   ││  Logs [l]  │  Inspect [i]                                                                                                                                        │
│● redis               1.2%    48M ││cpu  33.0% ▂▂▂▂▃▃▃▃▃▃▆▃▃▃▂▂▂▂▂▂▂▂▂▂▂▂▂▂▂▂▂▂▂▂▂▂▂▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁          ▄                          ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▂▂▂▂▂▂▂▂▂▂▆▂▂▂▂▂▂▂▂▂▂▂▂▃▃▃▃▃▃▃▃▃▃▂▂▂▂▂▂▂▂▂▂▂▂│
│● postgres            4.8%   256M ││mem    42% ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃│
│● caddy               0.3%    12M ││net ↑11.7K/s ↓3.9K/s   dsk r 2.0K/s  w 7.8K/s                                                                                                                     │
│● bushel-smoke        0.1%     4M ││2026-08-24T18:02:11.441Z INFO  postgres  checkpoint complete: wrote 214 buffers (1.6%); 0 WAL file(s) added, 0 removed, 1 recycled; write=0.215 s, sync=0.031 s, t│
│● github-actions-run 12.4%   512M ││2026-08-24T18:02:12.018Z INFO  redis     replica 192.168.64.4:6379 asks for sync, starting BGSAVE                                                                 │
│○ worker-a            0.0%     0M ││2026-08-24T18:02:12.441Z INFO  caddy     192.168.64.1 - GET /health 200 1.2ms                                                                                     │
│○ worker-b            0.0%     0M ││2026-08-24T18:02:13.102Z WARN  postgres  could not serialize access due to concurrent update                                                                      │
│○ old-migrate         0.0%     0M ││2026-08-24T18:02:13.880Z INFO  runner    job 1842 queued: build / test (aarch64-apple-darwin)                                                                     │
│                                  ││2026-08-24T18:02:14.201Z INFO  redis     10000 changes in 60 seconds. Saving...                                                                                   │
│                                  ││2026-08-24T18:02:14.990Z INFO  postgres  statement: SELECT * FROM orders WHERE created_at > now() - interval '5 minutes' AND status IN ('pending','paid')         │
│                                  ││2026-08-24T18:02:15.441Z INFO  caddy     192.168.64.8 - POST /webhooks/github 204 42.8ms                                                                          │
│                                  ││2026-08-24T18:02:16.002Z INFO  runner    cloning github.com/frankieramirez/bushel@frankieramirez/issue-14-wayfinder                                               │
│                                  ││2026-08-24T18:02:16.771Z ERROR postgres  FATAL:  remaining connection slots are reserved for non-replication superuser connections                                │
│                                  ││2026-08-24T18:02:17.110Z INFO  redis     DB saved on disk                                                                                                         │
│                                  ││2026-08-24T18:02:17.880Z INFO  caddy     logger: flushed 128 entries                                                                                              │
│                                  ││2026-08-24T18:02:18.441Z INFO  postgres  automatic vacuum of table "public.orders": index scans: 1, pages: 0 removed, 184 remain                                  │
│                                  ││2026-08-24T18:02:19.002Z INFO  runner    cargo test --offline                                                                                                     │
│                                  ││2026-08-24T18:02:19.660Z INFO  redis     client closed connection                                                                                                 │
│                                  ││2026-08-24T18:02:20.101Z INFO  postgres  duration: 812.441 ms  execute <unnamed>: SELECT n FROM generate_series(1,1000000) n                                      │
│                                  ││2026-08-24T18:02:20.880Z INFO  caddy     tls: remaining 2 certificates                                                                                            │
│                                  ││2026-08-24T18:02:21.441Z INFO  runner    test engine::poll::confirms_pending ... ok                                                                               │
│                                  ││2026-08-24T18:02:22.018Z WARN  postgres  log line too long to read at 55 columns — this is the unreadability complaint                                            │
│                                  ││2026-08-24T18:02:22.441Z INFO  redis     1.2µs per op, 800k ops/s                                                                                                 │
│                                  ││── following (F to pause) ──                                                                                                                                      │
│                                  ││                                                                                                                                                                  │
│                                  ││                                                                                                                                                                  │
│                                  ││                                                                                                                                                                  │
│                                  ││                                                                                                                                                                  │
│                                  ││                                                                                                                                                                  │
│                                  ││                                                                                                                                                                  │
│                                  ││                                                                                                                                                                  │
│                                  ││                                                                                                                                                                  │
╰──────────────────────────────────╯│                                                                                                                                                                  │
╭ images 5 ────────────────────────╮│                                                                                                                                                                  │
│docker.io/library/postgres:16     ││                                                                                                                                                                  │
│docker.io/library/redis:7         ││                                                                                                                                                                  │
│docker.io/library/caddy:2         ││                                                                                                                                                                  │
│docker.io/library/alpine:latest   ││                                                                                                                                                                  │
│ghcr.io/example/worker:1.4.2      ││                                                                                                                                                                  │
╰──────────────────────────────────╯│                                                                                                                                                                  │
╭ volumes 3 ───────────────────────╮│                                                                                                                                                                  │
│pg-data                   in use  ││                                                                                                                                                                  │
│redis-data                in use  ││                                                                                                                                                                  │
│leftover                  -       ││                                                                                                                                                                  │
╰──────────────────────────────────╯╰ 162 cols · strip 3 · last 162s of 5m ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
 1/2/3 expand  j/k move  enter focus  / filter  s strip  a ascii  ? help                                                                                                 ● service  container 1.2.0  ⠋
```

## 100×30 ascii glyphs

beside  rail 36×27  detail 64×27  collapse=roomy  cap=36  stack<80  log_cols=62  active=containers

```
 bushel   1 containers · 2 images · 3 volumes

╭ containers 8 ────────────────────╮╭ detail ──────────────────────────────────────────────────────╮
│name                 cpu    mem   ││  Logs [l]  │  Inspect [i]                                    │
│● redis               1.2%    48M ││cpu  33.0% ......::::::::::*::::::::::::----------::::::::::::│
│● postgres            4.8%   256M ││mem    42% ---------------------------------------------------│
│● caddy               0.3%    12M ││net ^11.7K/s v3.9K/s   dsk r 2.0K/s  w 7.8K/s                 │
│● bushel-smoke        0.1%     4M ││2026-08-24T18:02:11.441Z INFO  postgres  checkpoint complete: │
│● github-actions-run 12.4%   512M ││2026-08-24T18:02:12.018Z INFO  redis     replica 192.168.64.4:│
│○ worker-a            0.0%     0M ││2026-08-24T18:02:12.441Z INFO  caddy     192.168.64.1 - GET /h│
│○ worker-b            0.0%     0M ││2026-08-24T18:02:13.102Z WARN  postgres  could not serialize a│
│○ old-migrate         0.0%     0M ││2026-08-24T18:02:13.880Z INFO  runner    job 1842 queued: buil│
│                                  ││2026-08-24T18:02:14.201Z INFO  redis     10000 changes in 60 s│
│                                  ││2026-08-24T18:02:14.990Z INFO  postgres  statement: SELECT * F│
│                                  ││2026-08-24T18:02:15.441Z INFO  caddy     192.168.64.8 - POST /│
│                                  ││2026-08-24T18:02:16.002Z INFO  runner    cloning github.com/fr│
╰──────────────────────────────────╯│2026-08-24T18:02:16.771Z ERROR postgres  FATAL:  remaining con│
╭ images 5 ────────────────────────╮│2026-08-24T18:02:17.110Z INFO  redis     DB saved on disk     │
│docker.io/library/postgres:16     ││2026-08-24T18:02:17.880Z INFO  caddy     logger: flushed 128 e│
│docker.io/library/redis:7         ││2026-08-24T18:02:18.441Z INFO  postgres  automatic vacuum of t│
│docker.io/library/caddy:2         ││2026-08-24T18:02:19.002Z INFO  runner    cargo test --offline │
│docker.io/library/alpine:latest   ││2026-08-24T18:02:19.660Z INFO  redis     client closed connect│
│ghcr.io/example/worker:1.4.2      ││2026-08-24T18:02:20.101Z INFO  postgres  duration: 812.441 ms │
╰──────────────────────────────────╯│2026-08-24T18:02:20.880Z INFO  caddy     tls: remaining 2 cert│
╭ volumes 3 ───────────────────────╮│2026-08-24T18:02:21.441Z INFO  runner    test engine::poll::co│
│pg-data                   in use  ││2026-08-24T18:02:22.018Z WARN  postgres  log line too long to │
│redis-data                in use  ││2026-08-24T18:02:22.441Z INFO  redis     1.2µs per op, 800k op│
│leftover                  -       ││── following (F to pause) ──                                  │
╰──────────────────────────────────╯╰ 62 cols · strip 3 · last 62s of 5m ──────────────────────────╯
 1/2/3 expand  j/k move  enter focus  / filter  s strip  a ascii  ? help
```

## 55×20 4-row (should collapse)

stacked  rail 55×9  detail 55×9  collapse=tight  cap=36  stack<80  log_cols=53  active=containers

```
 bushel   1 containers · 2 images · 3 volumes
╭ containers 8 ───────────────────────────────────────╮
│● redis              1.2%    48M  redis:7            │
│● postgres           4.8%   256M  postgres:16        │
│● caddy              0.3%    12M  caddy:2            │
│● bushel-smoke       0.1%     4M  alpine:latest      │
│● github-actions-ru 12.4%   512M  ghcr.io/example/wor│
╰─────────────────────────────────────────────────────╯
 2 images 5
 3 volumes 3
╭ detail ─────────────────────────────────────────────╮
│2026-08-24T18:02:11.441Z INFO  postgres  checkpoint c│
│2026-08-24T18:02:12.018Z INFO  redis     replica 192.│
│2026-08-24T18:02:12.441Z INFO  caddy     192.168.64.1│
│2026-08-24T18:02:13.102Z WARN  postgres  could not se│
│2026-08-24T18:02:13.880Z INFO  runner    job 1842 que│
│2026-08-24T18:02:14.201Z INFO  redis     10000 change│
│2026-08-24T18:02:14.990Z INFO  postgres  statement: S│
╰ 53 cols · strip collapsed ──────────────────────────╯
 1/2/3 expand  j/k move  / filter  f zoom
```

## 100×30 2-row layout

beside  rail 36×27  detail 64×27  collapse=roomy  cap=36  stack<80  log_cols=62  active=containers

```
 bushel   1 containers · 2 images · 3 volumes

╭ containers 8 ────────────────────╮╭ detail ──────────────────────────────────────────────────────╮
│name                 cpu    mem   ││  Logs [l]  │  Inspect [i]                                    │
│● redis               1.2%    48M ││cpu  33.0% ▃▃▃▃▃▃▃▃▂▂▂▂▂▂▂▂▂▂▂▂mem    42% ▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃│
│● postgres            4.8%   256M ││net ↑11.7K/s ↓3.9K/s   dsk r 2.0K/s  w 7.8K/s                 │
│● caddy               0.3%    12M ││2026-08-24T18:02:11.441Z INFO  postgres  checkpoint complete: │
│● bushel-smoke        0.1%     4M ││2026-08-24T18:02:12.018Z INFO  redis     replica 192.168.64.4:│
│● github-actions-run 12.4%   512M ││2026-08-24T18:02:12.441Z INFO  caddy     192.168.64.1 - GET /h│
│○ worker-a            0.0%     0M ││2026-08-24T18:02:13.102Z WARN  postgres  could not serialize a│
│○ worker-b            0.0%     0M ││2026-08-24T18:02:13.880Z INFO  runner    job 1842 queued: buil│
│○ old-migrate         0.0%     0M ││2026-08-24T18:02:14.201Z INFO  redis     10000 changes in 60 s│
│                                  ││2026-08-24T18:02:14.990Z INFO  postgres  statement: SELECT * F│
│                                  ││2026-08-24T18:02:15.441Z INFO  caddy     192.168.64.8 - POST /│
│                                  ││2026-08-24T18:02:16.002Z INFO  runner    cloning github.com/fr│
│                                  ││2026-08-24T18:02:16.771Z ERROR postgres  FATAL:  remaining con│
╰──────────────────────────────────╯│2026-08-24T18:02:17.110Z INFO  redis     DB saved on disk     │
╭ images 5 ────────────────────────╮│2026-08-24T18:02:17.880Z INFO  caddy     logger: flushed 128 e│
│docker.io/library/postgres:16     ││2026-08-24T18:02:18.441Z INFO  postgres  automatic vacuum of t│
│docker.io/library/redis:7         ││2026-08-24T18:02:19.002Z INFO  runner    cargo test --offline │
│docker.io/library/caddy:2         ││2026-08-24T18:02:19.660Z INFO  redis     client closed connect│
│docker.io/library/alpine:latest   ││2026-08-24T18:02:20.101Z INFO  postgres  duration: 812.441 ms │
│ghcr.io/example/worker:1.4.2      ││2026-08-24T18:02:20.880Z INFO  caddy     tls: remaining 2 cert│
╰──────────────────────────────────╯│2026-08-24T18:02:21.441Z INFO  runner    test engine::poll::co│
╭ volumes 3 ───────────────────────╮│2026-08-24T18:02:22.018Z WARN  postgres  log line too long to │
│pg-data                   in use  ││2026-08-24T18:02:22.441Z INFO  redis     1.2µs per op, 800k op│
│redis-data                in use  ││── following (F to pause) ──                                  │
│leftover                  -       ││                                                              │
╰──────────────────────────────────╯╰ 62 cols · strip 2 · last 62s of 5m ──────────────────────────╯
 1/2/3 expand  j/k move  enter focus  / filter  s strip  a ascii  ? help
```


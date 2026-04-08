# Context Diagram

```text
+------------------+
|      User        |
| reads / dictates |
+---------+--------+
          |
          v
+--------------------------+
| Host Shells              |
| - desktop                |
| - iOS                    |
| - Android                |
| - tui-rs tool            |
+------------+-------------+
             |
             | contracts / FFI / local bridge
             v
+------------------------------+
| Shared Marginalia Engine     |
| - application services       |
| - contracts                  |
| - state projections          |
+-------------+----------------+
              |
              v
+------------------------------+
| Core Domain + State Machine  |
| - document                   |
| - reading session            |
| - voice note                 |
| - rewrite draft              |
| - events                     |
+----+------------+------------+
     |            |
     | ports      | ports
     v            v
+-----------+  +--------------------+
| SQLite    |  | Provider Adapters  |
| storage   |  | TTS / STT / LLM    |
| sessions  |  | playback / fakes   |
| notes     |  | host bridges       |
+-----------+  +--------------------+
```

## Boundary Notes

- the shared engine does not know about UI frameworks or host-specific shells
- providers are swapped by replacing adapters behind ports
- hosts depend on exported contracts, not on internal services
- Alpha's Python backend process was an earlier host shape, not the Beta target

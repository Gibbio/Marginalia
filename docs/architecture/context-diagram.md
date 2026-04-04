# Context Diagram

```text
+------------------+
|      User        |
| reads / dictates |
+---------+--------+
          |
          v
+------------------+
|   Marginalia CLI |
+---------+--------+
          |
          v
+------------------------------+
| Application Services         |
| - reader                     |
| - note                       |
| - rewrite                    |
| - summary                    |
| - search                     |
+---------+--------------------+
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
+-----------+  +----------------+
| SQLite    |  | Local Adapters |
| storage   |  | fake STT/TTS   |
| sessions  |  | fake playback  |
| notes     |  | fake LLM       |
+-----------+  +----------------+

Future, not implemented now:
- desktop shell over the same application services
- local API surface over the same core
- editor adapters that depend on exported contracts, not on the domain internals
```

## Boundary Notes

- the core does not know about CLI, desktop, or editors
- providers are swapped by replacing adapters behind ports
- editor integration remains outside the core until there is a stable contract

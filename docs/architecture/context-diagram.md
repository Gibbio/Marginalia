# Context Diagram

```text
+------------------+
|      User        |
| reads / dictates |
+---------+--------+
          |
          v
+--------------------------+
| Frontends                |
| - TUI                    |
| - Desktop GUI            |
| - Obsidian plugin        |
| - Future mobile client   |
+------------+-------------+
             |
             | commands / queries / events
             v
+------------------------------+
| Local Marginalia Backend     |
| - frontend gateway           |
| - runtime supervision        |
| - state projections          |
+-------------+----------------+
              |
              v
+------------------------------+
| Application Services         |
| - reader                     |
| - note                       |
| - rewrite                    |
| - summary                    |
| - search                     |
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
| SQLite    |  | Local Adapters     |
| storage   |  | Kokoro / Piper TTS |
| sessions  |  | Vosk command STT   |
| notes     |  | subprocess playback|
+-----------+  | fake fallbacks     |
               +--------------------+
```

## Boundary Notes

- the core does not know about TUI, GUI, mobile, or editor frameworks
- providers are swapped by replacing adapters behind ports
- frontends depend on exported contracts, not on internal services
- editor integration remains outside the core until there is a stable contract

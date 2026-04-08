use marginalia_core::frontend::{AppSnapshot, SessionSnapshot};
use marginalia_runtime::{FakeRuntime, SqliteRuntime};
use marginalia_tts_kokoro::KokoroConfig;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    FakePlay { document_path: PathBuf },
    KokoroDoctor { assets_root: PathBuf },
    SqliteIngest { db_path: PathBuf, document_path: PathBuf },
    SqliteListDocuments { db_path: PathBuf },
    SqlitePlay { db_path: PathBuf, document_path: PathBuf },
    SqlitePlayTarget { db_path: PathBuf, target: String },
    SqlitePause { db_path: PathBuf },
    SqliteResume { db_path: PathBuf },
    SqliteStop { db_path: PathBuf },
    SqliteRepeat { db_path: PathBuf },
    SqliteNextChunk { db_path: PathBuf },
    SqlitePreviousChunk { db_path: PathBuf },
    SqliteNextChapter { db_path: PathBuf },
    SqlitePreviousChapter { db_path: PathBuf },
    SqliteRestartChapter { db_path: PathBuf },
    SqliteNote { db_path: PathBuf, text: String },
    SqliteStatus { db_path: PathBuf },
}

fn main() -> ExitCode {
    match parse_args(env::args().skip(1)) {
        Ok(command) => match run(command) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::from(1)
            }
        },
        Err(error) => {
            eprintln!("error: {error}");
            eprintln!();
            eprintln!("{}", usage());
            ExitCode::from(2)
        }
    }
}

fn parse_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let collected = args.into_iter().map(Into::into).collect::<Vec<_>>();
    match collected.as_slice() {
        [command, document_path] if command == "fake-play" => Ok(Command::FakePlay {
            document_path: PathBuf::from(document_path),
        }),
        [command] if command == "kokoro-doctor" => Ok(Command::KokoroDoctor {
            assets_root: PathBuf::from("models/tts/kokoro"),
        }),
        [command, assets_root] if command == "kokoro-doctor" => Ok(Command::KokoroDoctor {
            assets_root: PathBuf::from(assets_root),
        }),
        [command, db_path, document_path] if command == "sqlite-ingest" => {
            Ok(Command::SqliteIngest {
                db_path: PathBuf::from(db_path),
                document_path: PathBuf::from(document_path),
            })
        }
        [command, db_path, document_path] if command == "sqlite-play" => Ok(Command::SqlitePlay {
            db_path: PathBuf::from(db_path),
            document_path: PathBuf::from(document_path),
        }),
        [command, db_path, target] if command == "sqlite-play-target" => {
            Ok(Command::SqlitePlayTarget {
                db_path: PathBuf::from(db_path),
                target: target.to_string(),
            })
        }
        [command, db_path] if command == "sqlite-list-documents" => {
            Ok(Command::SqliteListDocuments {
                db_path: PathBuf::from(db_path),
            })
        }
        [command, db_path] if command == "sqlite-pause" => Ok(Command::SqlitePause {
            db_path: PathBuf::from(db_path),
        }),
        [command, db_path] if command == "sqlite-resume" => Ok(Command::SqliteResume {
            db_path: PathBuf::from(db_path),
        }),
        [command, db_path] if command == "sqlite-stop" => Ok(Command::SqliteStop {
            db_path: PathBuf::from(db_path),
        }),
        [command, db_path] if command == "sqlite-repeat" => Ok(Command::SqliteRepeat {
            db_path: PathBuf::from(db_path),
        }),
        [command, db_path] if command == "sqlite-next-chunk" => Ok(Command::SqliteNextChunk {
            db_path: PathBuf::from(db_path),
        }),
        [command, db_path] if command == "sqlite-previous-chunk" => {
            Ok(Command::SqlitePreviousChunk {
                db_path: PathBuf::from(db_path),
            })
        }
        [command, db_path] if command == "sqlite-next-chapter" => {
            Ok(Command::SqliteNextChapter {
                db_path: PathBuf::from(db_path),
            })
        }
        [command, db_path] if command == "sqlite-previous-chapter" => {
            Ok(Command::SqlitePreviousChapter {
                db_path: PathBuf::from(db_path),
            })
        }
        [command, db_path] if command == "sqlite-restart-chapter" => {
            Ok(Command::SqliteRestartChapter {
                db_path: PathBuf::from(db_path),
            })
        }
        [command, db_path, text @ ..] if command == "sqlite-note" && !text.is_empty() => {
            Ok(Command::SqliteNote {
                db_path: PathBuf::from(db_path),
                text: text.join(" "),
            })
        }
        [command, db_path] if command == "sqlite-status" => Ok(Command::SqliteStatus {
            db_path: PathBuf::from(db_path),
        }),
        [] => Err("missing command".to_string()),
        _ => Err("invalid arguments".to_string()),
    }
}

fn run(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Command::FakePlay { document_path } => {
            let mut runtime = FakeRuntime::new();
            let outcome = runtime.ingest_path(&document_path)?;
            let session = runtime.start_session(&outcome.document.document_id)?;
            let app_snapshot = runtime.app_snapshot();
            let session_snapshot = runtime.session_snapshot()?.expect("active session snapshot");

            println!("runtime=fake");
            println!("document_id={}", outcome.document.document_id);
            println!("session_id={}", session.session_id);
            print_app_snapshot(&app_snapshot);
            print_session_snapshot(&session_snapshot);
            print_events(runtime.published_events().len());
            Ok(())
        }
        Command::KokoroDoctor { assets_root } => {
            let config = KokoroConfig::from_assets_root(&assets_root);
            let report = config.doctor_report();
            let capabilities = report.readiness.provider_capabilities();

            println!("provider={}", capabilities.provider_name);
            println!("provider.ready={}", report.is_ready());
            println!(
                "provider.assets_ready={}",
                report.readiness.is_ready()
            );
            println!(
                "provider.onnx_ready={}",
                report.onnx_probe.is_ready()
            );
            println!(
                "provider.assets_root={}",
                report.readiness.assets_root.display()
            );
            println!(
                "provider.model_path={}",
                report
                    .readiness
                    .model_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "-".to_string())
            );
            println!(
                "provider.voice_path={}",
                report
                    .readiness
                    .voice_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "-".to_string())
            );
            println!(
                "provider.onnx_runtime_path={}",
                report
                    .onnx_probe
                    .runtime_library_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "-".to_string())
            );
            println!(
                "provider.onnx_input_count={}",
                report.onnx_probe.input_count
            );
            println!(
                "provider.onnx_output_count={}",
                report.onnx_probe.output_count
            );
            println!(
                "provider.onnx_error={}",
                report
                    .onnx_probe
                    .error
                    .as_deref()
                    .unwrap_or("-")
            );
            println!(
                "provider.default_language={}",
                report.readiness.default_language
            );
            println!(
                "provider.sample_rate_hz={}",
                report.readiness.sample_rate_hz
            );
            if report.readiness.missing.is_empty() {
                println!("provider.missing=none");
            } else {
                for (index, item) in report.readiness.missing.iter().enumerate() {
                    println!("provider.missing[{index}]={item}");
                }
            }
            Ok(())
        }
        Command::SqliteIngest {
            db_path,
            document_path,
        } => {
            let mut runtime = SqliteRuntime::open(&db_path)?;
            let outcome = runtime.ingest_path(&document_path)?;
            let app_snapshot = runtime.app_snapshot();

            println!("runtime=sqlite");
            println!("db_path={}", db_path.display());
            println!("document_id={}", outcome.document.document_id);
            print_app_snapshot(&app_snapshot);
            print_events(runtime.published_events().len());
            Ok(())
        }
        Command::SqlitePlay {
            db_path,
            document_path,
        } => {
            let mut runtime = SqliteRuntime::open(&db_path)?;
            let outcome = runtime.ingest_path(&document_path)?;
            let session = runtime.start_session(&outcome.document.document_id)?;
            let app_snapshot = runtime.app_snapshot();
            let session_snapshot = runtime.session_snapshot()?.expect("active session snapshot");

            println!("runtime=sqlite");
            println!("db_path={}", db_path.display());
            println!("document_id={}", outcome.document.document_id);
            println!("session_id={}", session.session_id);
            print_app_snapshot(&app_snapshot);
            print_session_snapshot(&session_snapshot);
            print_events(runtime.published_events().len());
            Ok(())
        }
        Command::SqlitePlayTarget { db_path, target } => {
            let mut runtime = SqliteRuntime::open(&db_path)?;
            let document_id = if Path::new(&target).exists() {
                runtime.ingest_path(Path::new(&target))?.document.document_id
            } else {
                target
            };
            let session = runtime.start_session(&document_id)?;
            let app_snapshot = runtime.app_snapshot();
            let session_snapshot = runtime.session_snapshot()?.expect("active session snapshot");

            println!("runtime=sqlite");
            println!("db_path={}", db_path.display());
            println!("document_id={document_id}");
            println!("session_id={}", session.session_id);
            print_app_snapshot(&app_snapshot);
            print_session_snapshot(&session_snapshot);
            print_events(runtime.published_events().len());
            Ok(())
        }
        Command::SqliteListDocuments { db_path } => {
            let runtime = SqliteRuntime::open(&db_path)?;
            let documents = runtime.list_documents();

            println!("runtime=sqlite");
            println!("db_path={}", db_path.display());
            println!("documents.count={}", documents.len());
            for (index, document) in documents.iter().enumerate() {
                println!("documents[{index}].document_id={}", document.document_id);
                println!("documents[{index}].title={}", document.title);
                println!("documents[{index}].chapter_count={}", document.chapter_count);
                println!("documents[{index}].chunk_count={}", document.chunk_count);
            }
            Ok(())
        }
        Command::SqlitePause { db_path } => {
            let mut runtime = SqliteRuntime::open(&db_path)?;
            runtime.pause_session()?;
            print_runtime_state(&mut runtime, &db_path)?;
            Ok(())
        }
        Command::SqliteResume { db_path } => {
            let mut runtime = SqliteRuntime::open(&db_path)?;
            runtime.resume_session()?;
            print_runtime_state(&mut runtime, &db_path)?;
            Ok(())
        }
        Command::SqliteStop { db_path } => {
            let mut runtime = SqliteRuntime::open(&db_path)?;
            runtime.stop_session()?;
            print_runtime_state(&mut runtime, &db_path)?;
            Ok(())
        }
        Command::SqliteRepeat { db_path } => {
            let mut runtime = SqliteRuntime::open(&db_path)?;
            runtime.repeat_chunk()?;
            print_runtime_state(&mut runtime, &db_path)?;
            Ok(())
        }
        Command::SqliteNextChunk { db_path } => {
            let mut runtime = SqliteRuntime::open(&db_path)?;
            runtime.next_chunk()?;
            print_runtime_state(&mut runtime, &db_path)?;
            Ok(())
        }
        Command::SqlitePreviousChunk { db_path } => {
            let mut runtime = SqliteRuntime::open(&db_path)?;
            runtime.previous_chunk()?;
            print_runtime_state(&mut runtime, &db_path)?;
            Ok(())
        }
        Command::SqliteNextChapter { db_path } => {
            let mut runtime = SqliteRuntime::open(&db_path)?;
            runtime.next_chapter()?;
            print_runtime_state(&mut runtime, &db_path)?;
            Ok(())
        }
        Command::SqlitePreviousChapter { db_path } => {
            let mut runtime = SqliteRuntime::open(&db_path)?;
            runtime.previous_chapter()?;
            print_runtime_state(&mut runtime, &db_path)?;
            Ok(())
        }
        Command::SqliteRestartChapter { db_path } => {
            let mut runtime = SqliteRuntime::open(&db_path)?;
            runtime.restart_chapter()?;
            print_runtime_state(&mut runtime, &db_path)?;
            Ok(())
        }
        Command::SqliteNote { db_path, text } => {
            let mut runtime = SqliteRuntime::open(&db_path)?;
            let note = runtime.create_note(&text)?;

            println!("runtime=sqlite");
            println!("db_path={}", db_path.display());
            println!("note.note_id={}", note.note_id);
            println!("note.document_id={}", note.document_id);
            println!("note.anchor={}", note.anchor());
            println!("note.transcript={}", note.transcript);
            print_events(runtime.published_events().len());
            Ok(())
        }
        Command::SqliteStatus { db_path } => {
            let mut runtime = SqliteRuntime::open(&db_path)?;
            print_runtime_state(&mut runtime, &db_path)?;
            Ok(())
        }
    }
}

fn print_runtime_state(
    runtime: &mut SqliteRuntime,
    db_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let app_snapshot = runtime.app_snapshot();
    let session_snapshot = runtime.session_snapshot()?;

    println!("runtime=sqlite");
    println!("db_path={}", db_path.display());
    print_app_snapshot(&app_snapshot);
    if let Some(session_snapshot) = session_snapshot {
        print_session_snapshot(&session_snapshot);
    } else {
        println!("session=none");
    }
    print_events(runtime.published_events().len());
    Ok(())
}

fn print_app_snapshot(snapshot: &AppSnapshot) {
    println!("app.state={}", snapshot.state);
    println!("app.document_count={}", snapshot.document_count);
    println!(
        "app.active_session_id={}",
        snapshot.active_session_id.as_deref().unwrap_or("-")
    );
    println!(
        "app.latest_document_id={}",
        snapshot.latest_document_id.as_deref().unwrap_or("-")
    );
    println!(
        "app.playback_state={}",
        snapshot.playback_state.as_deref().unwrap_or("-")
    );
}

fn print_session_snapshot(snapshot: &SessionSnapshot) {
    println!("session.state={}", snapshot.state);
    println!("session.document_id={}", snapshot.document_id);
    println!("session.anchor={}", snapshot.anchor);
    println!("session.section_title={}", snapshot.section_title);
    println!("session.chunk_text={}", snapshot.chunk_text);
    println!("session.playback_state={}", snapshot.playback_state);
    println!("session.notes_count={}", snapshot.notes_count);
}

fn print_events(count: usize) {
    println!("events.published_count={count}");
}

fn usage() -> &'static str {
    "Usage:
  cargo run -p marginalia-devtools -- fake-play <document>
  cargo run -p marginalia-devtools -- kokoro-doctor [assets_root]
  cargo run -p marginalia-devtools -- sqlite-ingest <db> <document>
  cargo run -p marginalia-devtools -- sqlite-list-documents <db>
  cargo run -p marginalia-devtools -- sqlite-play <db> <document>
  cargo run -p marginalia-devtools -- sqlite-play-target <db> <path|document_id>
  cargo run -p marginalia-devtools -- sqlite-pause <db>
  cargo run -p marginalia-devtools -- sqlite-resume <db>
  cargo run -p marginalia-devtools -- sqlite-stop <db>
  cargo run -p marginalia-devtools -- sqlite-repeat <db>
  cargo run -p marginalia-devtools -- sqlite-next-chunk <db>
  cargo run -p marginalia-devtools -- sqlite-previous-chunk <db>
  cargo run -p marginalia-devtools -- sqlite-next-chapter <db>
  cargo run -p marginalia-devtools -- sqlite-previous-chapter <db>
  cargo run -p marginalia-devtools -- sqlite-restart-chapter <db>
  cargo run -p marginalia-devtools -- sqlite-note <db> <text>
  cargo run -p marginalia-devtools -- sqlite-status <db>"
}

#[cfg(test)]
mod tests {
    use super::{parse_args, Command};
    use std::path::PathBuf;

    #[test]
    fn parse_args_accepts_fake_play() {
        let command = parse_args(["fake-play", "/tmp/doc.md"]).unwrap();
        assert_eq!(
            command,
            Command::FakePlay {
                document_path: PathBuf::from("/tmp/doc.md"),
            }
        );
    }

    #[test]
    fn parse_args_accepts_kokoro_doctor_default() {
        let command = parse_args(["kokoro-doctor"]).unwrap();
        assert_eq!(
            command,
            Command::KokoroDoctor {
                assets_root: PathBuf::from("models/tts/kokoro"),
            }
        );
    }

    #[test]
    fn parse_args_accepts_sqlite_status() {
        let command = parse_args(["sqlite-status", "/tmp/marginalia.db"]).unwrap();
        assert_eq!(
            command,
            Command::SqliteStatus {
                db_path: PathBuf::from("/tmp/marginalia.db"),
            }
        );
    }

    #[test]
    fn parse_args_accepts_sqlite_note_with_spaces() {
        let command = parse_args(["sqlite-note", "/tmp/marginalia.db", "remember", "this"]).unwrap();
        assert_eq!(
            command,
            Command::SqliteNote {
                db_path: PathBuf::from("/tmp/marginalia.db"),
                text: "remember this".to_string(),
            }
        );
    }

    #[test]
    fn parse_args_accepts_sqlite_play_target() {
        let command = parse_args(["sqlite-play-target", "/tmp/db.sqlite", "doc-1"]).unwrap();
        assert_eq!(
            command,
            Command::SqlitePlayTarget {
                db_path: PathBuf::from("/tmp/db.sqlite"),
                target: "doc-1".to_string(),
            }
        );
    }

    #[test]
    fn parse_args_rejects_invalid_input() {
        let error = parse_args(["sqlite-play", "/tmp/db.sqlite"]).unwrap_err();
        assert_eq!(error, "invalid arguments");
    }
}

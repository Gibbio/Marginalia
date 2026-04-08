use marginalia_core::frontend::{AppSnapshot, SessionSnapshot};
use marginalia_runtime::{FakeRuntime, SqliteRuntime};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    FakePlay { document_path: PathBuf },
    SqliteIngest { db_path: PathBuf, document_path: PathBuf },
    SqlitePlay { db_path: PathBuf, document_path: PathBuf },
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
        Command::SqliteStatus { db_path } => {
            let mut runtime = SqliteRuntime::open(&db_path)?;
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
    }
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
  cargo run -p marginalia-devtools -- sqlite-ingest <db> <document>
  cargo run -p marginalia-devtools -- sqlite-play <db> <document>
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
    fn parse_args_rejects_invalid_input() {
        let error = parse_args(["sqlite-play", "/tmp/db.sqlite"]).unwrap_err();
        assert_eq!(error, "invalid arguments");
    }
}

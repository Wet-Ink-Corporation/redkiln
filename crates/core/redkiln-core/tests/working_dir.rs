//! The one test that changes the process's working directory, in a test binary of its own: under
//! `cargo test` the tests of one binary share a process and run in parallel, so no other test may
//! run while this one has moved it.

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::path::Path;
    use std::time::SystemTime;

    use redkiln_core::envelope::{Actor, Author, Class, Envelope, Payload, Timestamp, Writer};
    use redkiln_core::store::{Filter, Page, Recorded, Store, TelemetryEvent, Wait};
    use redkiln_core::trust::Trust;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    struct ToolTimed {
        millis: u64,
    }

    impl Payload for ToolTimed {
        const TYPE: &'static str = "core.tool_timed";
        const VERSION: u32 = 1;
        const CLASS: Class = Class::Telemetry;
    }

    fn timing() -> TelemetryEvent {
        let author = Author {
            actor: Actor::Hook,
            trust: Trust::asserted(),
            writer: Writer::new("weighin", "2.0.0").expect("valid writer"),
        };
        Envelope::new(
            Timestamp::parse("2026-10-31T04:20:11Z").expect("valid ts"),
            author,
            ToolTimed { millis: 1 },
        )
        .to_telemetry([])
        .expect("a telemetry type")
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("read the directory")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn the_telemetry_connection_opens_the_store_whatever_the_working_directory_becomes() {
        let first = tempfile::tempdir().expect("tempdir");
        let second = tempfile::tempdir().expect("tempdir");
        let original = std::env::current_dir().expect("working directory");
        std::env::set_current_dir(first.path()).expect("enter the first");
        let opened = Store::open(Path::new("events.sqlite3"), Wait::Command);
        std::env::set_current_dir(second.path()).expect("enter the second");
        let recorded =
            opened.map(|mut store| (store.record_telemetry(timing(), SystemTime::now()), store));
        // Left before any assertion, so neither directory is in use when it is removed.
        std::env::set_current_dir(original).expect("restore the working directory");

        let (recorded, store) = recorded.expect("opens");
        assert_eq!(recorded.expect("written"), Recorded::At(1));
        let read = store
            .read_telemetry(
                &Filter::all(),
                Page {
                    after: 0,
                    limit: NonZeroUsize::new(10).expect("non-zero"),
                },
                SystemTime::now(),
            )
            .expect("read");
        assert_eq!(read.len(), 1);
        assert!(
            names(first.path()).contains(&"events.sqlite3.telemetry".to_owned()),
            "the side store sits beside the store: {:?}",
            names(first.path())
        );
        assert!(
            names(second.path()).is_empty(),
            "nothing at the relative path in the new working directory"
        );
    }
}

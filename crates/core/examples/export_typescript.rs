#[cfg(feature = "ts-export")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use diffthing_core::hunk::{FileDiff, FileStatus, Hunk, HunkId};
    use diffthing_core::protocol::{ClientMsg, ErrorCode, JobStatus, ServerMsg, PROTOCOL_VERSION};
    use diffthing_core::reconcile::{Lineage, ReconcileReport};
    use diffthing_core::review::{Flag, FlagEntry, FlagEntryKind, HunkStatus, ReviewState};
    use diffthing_core::schema::{Impact, ImpactScore, Scope, Step, Walkthrough};
    use std::fs;
    use std::path::PathBuf;
    use ts_rs::TS;

    let web = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/src/libs");
    let generated = web.join("generated");
    if generated.exists() {
        fs::remove_dir_all(&generated)?;
    }
    fs::create_dir_all(&generated)?;

    macro_rules! export {
        ($($ty:ty),+ $(,)?) => { $(
            <$ty as TS>::export_to(generated.join(concat!(stringify!($ty), ".ts")))?;
        )+ };
    }
    export!(
        HunkId,
        Hunk,
        FileDiff,
        FileStatus,
        Impact,
        ImpactScore,
        Step,
        Scope,
        Walkthrough,
        HunkStatus,
        FlagEntryKind,
        FlagEntry,
        Flag,
        ReviewState,
        Lineage,
        ReconcileReport,
        ClientMsg,
        ServerMsg,
        JobStatus,
        ErrorCode,
    );

    let names = [
        "HunkId",
        "Hunk",
        "FileDiff",
        "FileStatus",
        "Impact",
        "ImpactScore",
        "Step",
        "Scope",
        "Walkthrough",
        "HunkStatus",
        "FlagEntryKind",
        "FlagEntry",
        "Flag",
        "ReviewState",
        "Lineage",
        "ReconcileReport",
        "ClientMsg",
        "ServerMsg",
        "JobStatus",
        "ErrorCode",
    ];
    let mut barrel = String::from(
        "// Generated from diffthing-core by `pnpm protocol:generate`. Do not edit.\n\n",
    );
    barrel.push_str(&format!("export const PROTOCOL_VERSION = {PROTOCOL_VERSION};\n\n"));
    for name in names {
        barrel.push_str(&format!("export type {{ {name} }} from \"./generated/{name}\";\n"));
    }
    fs::write(web.join("protocol.ts"), barrel)?;
    println!("generated TypeScript protocol v{PROTOCOL_VERSION}");
    Ok(())
}

#[cfg(not(feature = "ts-export"))]
fn main() {
    eprintln!("enable --features ts-export");
    std::process::exit(1);
}

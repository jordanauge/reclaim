/// Docker target — surfaces stopped containers, dangling images, and unused volumes.
///
/// Docker integration requires the Docker daemon to be running.  This module
/// delegates to the `docker` CLI rather than importing `bollard` so that the
/// core library stays free of async dependencies.  The actual `docker system prune`
/// call lives in the CLI/TUI layer; this module only produces `Candidate` entries
/// from `docker system df --format json` output.
use crate::candidate::Candidate;
use crate::profile::Profile;
use anyhow::Result;
use std::path::Path;

pub fn scan(_root: &Path, profile: &Profile) -> Result<Vec<Candidate>> {
    let config = profile.target_config("docker");
    if !config.enabled {
        return Ok(vec![]);
    }

    // TODO (milestone 0.5): spawn `docker system df --format json` and parse output
    // into Candidate entries for images, volumes, and stopped containers.
    //
    // Example structure:
    //   Candidate { kind: TargetKind::DockerImage, path: PathBuf::from("<image-id>"), ... }
    //
    // For now return empty — docker candidates are not yet implemented.
    Ok(vec![])
}

//! Explicit native verification is available where provider process trees can be bounded.
use super::*;
#[cfg(unix)]
mod unix;

pub(super) fn run(ctx: &Context, opt: &Options, roots: &[Root], response: &mut Response) -> i32 {
    if opt.selected.is_empty() {
        response.status = "complete".into();
        response
            .warnings
            .push("No agents selected or detected; no verification performed".into());
        return 0;
    }
    #[cfg(unix)]
    {
        unix::run(ctx, opt, roots, response)
    }
    #[cfg(not(unix))]
    {
        let _ = (ctx, roots);
        for agent in &mut response.agents {
            if opt.selected.contains(&agent.id) {
                agent.compatibility = "unverified";
                agent.catalog_status = "unsupported_platform";
                agent.reason = "Native verification is unavailable on this platform because bounded provider process-tree execution is not implemented; no provider was executed".into();
            }
        }
        response.status = "partial".into();
        3
    }
}

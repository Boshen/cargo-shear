// Uses `anyhow`, but never imports `workspace-hack` — the hack crate is depended
// on purely so its feature unification applies to this member.
pub fn example() -> anyhow::Result<()> {
    Ok(())
}

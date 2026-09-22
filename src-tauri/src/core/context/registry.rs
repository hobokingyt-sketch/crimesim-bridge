use super::policy::*;
use crate::core::{error::BridgeResult, types::BridgeProject};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, io::Read, path::Path};

pub const REGISTRY_PATH: &str = "project_control/game_context.json";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Module {
    pub id: String,
    pub title: String,
    pub paths: Vec<String>,
    pub card: String,
    pub depends_on: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub schema: u32,
    pub project_kind: String,
    pub project_id: String,
    pub modules: Vec<Module>,
}

pub fn read_bounded(path: &Path, limit: u64) -> BridgeResult<Vec<u8>> {
    if linked(path)? { return Err(invalid("Linked context file is not allowed")); }
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit { return Err(invalid(format!("Context file exceeds {limit} byte bound: {}", path.display()))); }
    Ok(bytes)
}

impl Registry {
    pub fn validate(&self, project_id: &str) -> BridgeResult<()> {
        if self.schema != 1 || self.project_kind != "game" || self.project_id != project_id {
            return Err(invalid("Game context identity/schema mismatch; Bridge-repository memory is not game memory"));
        }
        if self.modules.is_empty() || self.modules.len() > 32 { return Err(invalid("Game module registry must contain 1..32 areas")); }
        let mut ids = BTreeSet::new();
        for m in &self.modules {
            if m.id.is_empty() || m.id.len() > 48 || !m.id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                || !ids.insert(m.id.as_str()) || m.title.trim().is_empty() || m.title.len() > 100 || m.paths.len() > 32 || m.depends_on.len() > 32 {
                return Err(invalid("Invalid/duplicate game module identifier, title or limits"));
            }
            safe_path(&m.card)?;
            if excluded(&m.card) { return Err(invalid("Module card points to an excluded path")); }
            for prefix in &m.paths {
                safe_path(prefix.trim_end_matches('/'))?;
                if excluded(prefix) { return Err(invalid("Module ownership points to excluded data")); }
            }
        }
        for m in &self.modules {
            if m.depends_on.iter().any(|id| !ids.contains(id.as_str())) { return Err(invalid("Unknown module dependency")); }
            self.visit(&m.id, &mut BTreeSet::new(), &mut BTreeSet::new())?;
        }
        Ok(())
    }
    fn visit(&self, id: &str, stack: &mut BTreeSet<String>, done: &mut BTreeSet<String>) -> BridgeResult<()> {
        if done.contains(id) { return Ok(()); }
        if !stack.insert(id.into()) { return Err(invalid("Cyclic game module dependency")); }
        for d in &self.module(id)?.depends_on { self.visit(d, stack, done)?; }
        stack.remove(id);
        done.insert(id.into());
        Ok(())
    }
    pub fn module(&self, id: &str) -> BridgeResult<&Module> {
        self.modules.iter().find(|m| m.id == id).ok_or_else(|| invalid(format!("Unknown game area '{id}'; no broad fallback was used")))
    }
    pub fn owner(&self, path: &str) -> BridgeResult<Option<&str>> {
        let owners: Vec<_> = self.modules.iter().filter(|m| m.paths.iter().any(|rule|
            (path == rule || path.strip_suffix(".uid") == Some(rule.as_str()) || path.strip_suffix(".import") == Some(rule.as_str())) || rule.ends_with('/') && path.starts_with(rule))).collect();
        if owners.len() > 1 { return Err(invalid(format!("Ambiguous module ownership: {path}"))); }
        Ok(owners.first().map(|m| m.id.as_str()))
    }
}

pub fn defaults(meta: &BridgeProject) -> Registry {
    let specs = [
        ("foundation", "Game foundation", vec!["game/", "tests/bridge_smoke.gd", "project.godot", "export_presets.cfg"], vec![]),
        ("ui", "Interface", vec!["ui/", "tests/ui/"], vec!["foundation"]),
        ("map", "City and map", vec!["map/", "world/", "tests/map/"], vec!["foundation"]),
        ("simulation", "Simulation", vec!["simulation/", "tests/simulation/"], vec!["foundation", "map", "data"]),
        ("data", "Game data and assets", vec!["data/", "assets/", "tests/data/"], vec!["foundation"]),
    ];
    Registry { schema: 1, project_kind: "game".into(), project_id: meta.project_id.clone(),
        modules: specs.into_iter().map(|(id,title,paths,deps)| Module {
            id: id.into(), title: title.into(), paths: paths.into_iter().map(String::from).collect(),
            card: format!("project_control/modules/{id}.md"), depends_on: deps.into_iter().map(String::from).collect()
        }).collect() }
}

pub fn load(root: &Path, meta: &BridgeProject) -> BridgeResult<(Registry, bool)> {
    if linked(root)? || linked(&root.join("project_control"))? { return Err(invalid("Linked game-control directory")); }
    let p = root.join(REGISTRY_PATH);
    let legacy = !p.try_exists()?;
    let registry: Registry = if legacy { defaults(meta) } else { serde_json::from_slice(&read_bounded(&p, CONTROL_LIMIT)?)? };
    registry.validate(&meta.project_id)?;
    Ok((registry, legacy))
}

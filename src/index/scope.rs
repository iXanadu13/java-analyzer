#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleId(pub u32);

impl ModuleId {
    pub const ROOT: ModuleId = ModuleId(0);
}

#[derive(Debug, Clone, Copy)]
pub struct IndexScope {
    pub module: ModuleId,
}

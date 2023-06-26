pub mod combined;

use revm::inspectors::NoOpInspector;

/// NoInspector is used as a placeholder for type parameters when no inspector is needed.
pub type NoInspector = NoOpInspector;

pub static mut NO_INSPECTOR: NoInspector = NoInspector {};

pub fn no_inspector() -> &'static mut NoInspector {
    // unsafe is ok here since NoInspector is essential a no-op inspector
    unsafe { &mut NO_INSPECTOR }
}

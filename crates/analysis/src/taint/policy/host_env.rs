pub struct HostEnv {}

impl<S: BcState> PropagationPolicy<S> for HostEnv {
    fn before_step(
        &mut self,
        taint_tracker: &mut crate::taint::TaintTracker,
        interp: &mut libsofl_core::engine::types::Interpreter<'_>,
        data: &mut libsofl_core::engine::types::EVMData<'_, S>,
    ) -> Vec<Option<bool>> {
        match interp.current_opcode() {
            opcode::ORIGIN => {
                vec![Some(false)]
            }
            _ => Vec::new(),
        }
    }
}

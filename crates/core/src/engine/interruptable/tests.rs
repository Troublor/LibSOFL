#[cfg(test)]
mod tests {
    use crate::engine::memory::MemoryBcState;

    /// Test call a existing contract without breakpoint
    #[test]
    fn test_call_contract() {
        let mut state = MemoryBcState::fresh();
        let contract = r#"
        contract C {
            function foo() public returns (uint) {
                // do nothing
                return 0;
            }
        }
        "#;
    }
}

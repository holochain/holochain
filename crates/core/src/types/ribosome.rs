
/// Total hack just to have something to look at
pub struct Ribosome;
impl Ribosome {
    pub fn new(dna: Dna) -> Self {
        Self
    }

    pub fn run_validation<C: CursorR>(self, cursor: &C, entry: Entry) -> ValidationResult {
        unimplemented!()
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    pub fn call_zome_function<C: CursorRw>(
        self,
        cursor: C,
        invocation: ZomeInvocation,
        source_chain: SourceChain,
    ) -> SkunkResult<(ZomeInvocationResult, C)> {
        unimplemented!()
    }
}

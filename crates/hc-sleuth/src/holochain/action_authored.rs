use super::*;

#[derive(Clone, Debug, derive_more::Constructor)]
pub struct ActionAuthored {
    pub by: NodeId,
    pub action: ActionHash,
}
impl Fact for ActionAuthored {
    fn cause(&self, ctx: &Context) -> ACause {
        ().into()
    }

    fn explain(&self) -> String {
        format!("Agent {} authored Action {}", self.by, self.action)
    }

    fn check(&self, ctx: &Context) -> bool {
        let env = ctx.nodes.envs.get(self.by).unwrap();
        let Self { by, action } = self.clone();
        env.authored.test_read(move |txn| {
            txn.query_row(
                "SELECT rowid FROM Action WHERE author = :author AND hash = :hash",
                named_params! {
                    ":author": by,
                    ":hash": action,
                },
                |row| row.get::<_, usize>(0),
            )
            .optional()
            .unwrap()
            .is_some()
        })
    }
}

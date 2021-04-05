use holo_hash::*;
use holochain_sqlite::rusqlite::named_params;
use holochain_zome_types::*;
use std::fmt::Debug;

use super::*;

#[derive(Debug, Clone)]
pub struct ChainHeadQuery(AgentPubKey);

impl ChainHeadQuery {
    pub fn new(agent: AgentPubKey) -> Self {
        Self(agent)
    }
}

impl Query for ChainHeadQuery {
    type Data = SignedHeader;
    type State = Option<SignedHeader>;
    type Output = Option<HeaderHash>;

    fn create_query(&self) -> &str {
        "
            SELECT H.blob FROM Header AS H
            JOIN (
                SELECT MAX(seq) FROM Header
                GROUP BY author
            ) AS H2
            ON H.author = H2.author
            WHERE H.author = :author
        "
    }

    fn create_params(&self) -> Vec<Params> {
        let params = named_params! {
            ":author": self.0,
        };
        params.to_vec()
    }

    fn init_fold(&self) -> StateQueryResult<Self::State> {
        Ok(None)
    }

    fn as_filter(&self) -> Box<dyn Fn(&Self::Data) -> bool> {
        let author = self.0.clone();
        let f = move |header: &SignedHeader| *header.header().author() == author;
        Box::new(f)
    }

    fn fold(&mut self, state: Self::State, sh: SignedHeader) -> StateQueryResult<Self::State> {
        // Simple maximum finding
        Ok(Some(match state {
            None => sh,
            Some(old) => {
                if sh.header().header_seq() > old.header().header_seq() {
                    sh
                } else {
                    old
                }
            }
        }))
    }

    fn render<S>(&mut self, state: Self::State, _stores: S) -> StateQueryResult<Self::Output>
    where
        S: Stores<Self>,
        S::O: StoresIter<Self::Data>,
    {
        Ok(state.map(|sh| HeaderHash::with_data_sync(sh.header())))
    }

    fn as_map(&self) -> Arc<dyn Fn(&Row) -> StateQueryResult<Self::Data>> {
        Arc::new(row_to_header("blob"))
    }
}

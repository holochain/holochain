use crate::prelude::StateMutationError;
use crate::prelude::StateMutationResult;
use crate::prelude::StateQueryResult;
use crate::query::StateQueryError;
use holochain_types::prelude::EntryDefBufferKey;
use holochain_zome_types::prelude::*;

pub async fn get(
    db: &holochain_data::DbRead<holochain_data::kind::Wasm>,
    key: EntryDefBufferKey,
) -> StateQueryResult<Option<EntryDef>> {
    use holochain_serialized_bytes::SerializedBytes;
    let serialized: SerializedBytes = key
        .try_into()
        .map_err(|e: holochain_serialized_bytes::SerializedBytesError| StateQueryError::from(e))?;
    let key_bytes = serialized.bytes().to_vec();
    match db.get_entry_def(&key_bytes).await {
        Ok(entry_def) => Ok(entry_def),
        Err(e) => Err(StateQueryError::from(e)),
    }
}

pub async fn get_all(
    db: &holochain_data::DbRead<holochain_data::kind::Wasm>,
) -> StateQueryResult<Vec<(EntryDefBufferKey, EntryDef)>> {
    let all_entry_defs = db
        .get_all_entry_defs()
        .await
        .map_err(StateQueryError::from)?;

    use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
    all_entry_defs
        .into_iter()
        .map(|(key_bytes, entry_def)| {
            let serialized = SerializedBytes::from(UnsafeBytes::from(key_bytes));
            let key: EntryDefBufferKey = serialized.try_into().map_err(
                |e: holochain_serialized_bytes::SerializedBytesError| StateQueryError::from(e),
            )?;
            Ok((key, entry_def))
        })
        .collect()
}

pub async fn contains(
    db: &holochain_data::DbRead<holochain_data::kind::Wasm>,
    key: EntryDefBufferKey,
) -> StateQueryResult<bool> {
    use holochain_serialized_bytes::SerializedBytes;
    let serialized: SerializedBytes = key
        .try_into()
        .map_err(|e: holochain_serialized_bytes::SerializedBytesError| StateQueryError::from(e))?;
    let key_bytes = serialized.bytes().to_vec();
    db.entry_def_exists(&key_bytes)
        .await
        .map_err(StateQueryError::from)
}

pub async fn put(
    db: &holochain_data::DbWrite<holochain_data::kind::Wasm>,
    key: EntryDefBufferKey,
    entry_def: &EntryDef,
) -> StateMutationResult<()> {
    use holochain_serialized_bytes::SerializedBytes;
    let serialized: SerializedBytes =
        key.try_into()
            .map_err(|e: holochain_serialized_bytes::SerializedBytesError| {
                StateMutationError::from(e)
            })?;
    let key_bytes = serialized.bytes().to_vec();
    db.put_entry_def(key_bytes, entry_def)
        .await
        .map_err(StateMutationError::from)
}

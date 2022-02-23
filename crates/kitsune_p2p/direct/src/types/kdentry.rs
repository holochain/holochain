//! kdirect kdentry type

use crate::*;
use futures::future::{BoxFuture, FutureExt};

pub use kitsune_p2p_direct_api::{KdEntryContent, KdEntrySigned};

/// Extension trait to augment the direct_api version of KdEntrySigned
pub trait KdEntrySignedExt: Sized {
    /// Build out a full, checked entry from wire encoding
    fn from_wire(wire: Box<[u8]>) -> BoxFuture<'static, KdResult<Self>>;

    /// Build out a full, checked entry from db encoding
    fn from_str(s: &str) -> BoxFuture<'static, KdResult<Self>>;

    /// Sign entry data into a full KdEntry instance
    fn from_content(
        persist: &KdPersist,
        content: KdEntryContent,
    ) -> BoxFuture<'static, KdResult<Self>>;

    /// Sign entry data into a full KdEntry instance with additional binary data
    fn from_content_with_binary(
        persist: &KdPersist,
        content: KdEntryContent,
        binary: &[u8],
    ) -> BoxFuture<'static, KdResult<Self>>;
}

async fn check_sig_and_hash(e: &KdEntrySigned) -> KdResult<()> {
    let data = sodoken::BufRead::new_no_lock(e.as_data_to_sign_ref());

    let sig = e.as_signature_ref();
    let sig = Arc::new(*sig);

    let author = e.author();

    if author.verify_signature(data, sig).await {
        Ok(())
    } else {
        Err("invalid signature".into())
    }
}

impl KdEntrySignedExt for KdEntrySigned {
    fn from_wire(wire: Box<[u8]>) -> BoxFuture<'static, KdResult<Self>> {
        async move {
            let out = Self::from_wire_unchecked(wire)?;
            check_sig_and_hash(&out).await?;
            Ok(out)
        }
        .boxed()
    }

    fn from_str(s: &str) -> BoxFuture<'static, KdResult<Self>> {
        let out = Self::from_str_unchecked(s);
        async move {
            let out = out?;
            check_sig_and_hash(&out).await?;
            Ok(out)
        }
        .boxed()
    }

    fn from_content(
        persist: &KdPersist,
        content: KdEntryContent,
    ) -> BoxFuture<'static, KdResult<Self>> {
        Self::from_content_with_binary(persist, content, &[])
    }

    /// Sign entry data into a full KdEntry instance with additional binary data
    fn from_content_with_binary(
        persist: &KdPersist,
        content: KdEntryContent,
        binary: &[u8],
    ) -> BoxFuture<'static, KdResult<Self>> {
        let persist = persist.clone();
        let binary_len = binary.len();
        let data_to_sign = content.to_data_to_sign(binary.to_vec());
        async move {
            let _ = &content;
            let data_to_sign = data_to_sign?;
            let hash = KdHash::from_data(&data_to_sign).await?;
            let signature = persist
                .sign(content.author.clone(), &data_to_sign)
                .await
                .map_err(KdError::other)?;
            let out = Self::from_components_unchecked(
                data_to_sign,
                binary_len,
                hash.as_ref(),
                &signature,
            )?;
            check_sig_and_hash(&out).await?;
            Ok(out)
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_kdentry_codec() {
        let persist = crate::persist_mem::new_persist_mem();
        let agent = persist.generate_signing_keypair().await.unwrap();
        let binary = [0, 1, 2, 3];

        let edata = KdEntryContent {
            kind: "s.root".to_string(),
            parent: [0; 39].into(),
            author: agent,
            verify: "".to_string(),
            data: serde_json::json!({
                "hello": "world",
            }),
        };
        let entry = KdEntrySigned::from_content_with_binary(&persist, edata, &binary[..])
            .await
            .unwrap();
        println!("{:?}", &entry);
        let wire = entry.as_wire_data_ref();
        println!("wire: {}", String::from_utf8_lossy(wire));
        let e2 = KdEntrySigned::from_wire(wire.to_vec().into_boxed_slice())
            .await
            .unwrap();
        assert_eq!(e2, entry);
        assert_eq!(&[0, 1, 2, 3][..], e2.as_binary_ref());
        let s = entry.to_string();
        println!("str: {}", s);
        let e3 = KdEntrySigned::from_str(&s).await.unwrap();
        assert_eq!(e3, entry);
        assert_eq!(&[0, 1, 2, 3][..], e3.as_binary_ref());
    }
}
